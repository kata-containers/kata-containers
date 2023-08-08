// Copyright 2020 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause

use std::io::Error as IOError;
use std::ops::Deref;
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::{mpsc, Arc, Mutex};

use dbs_utils::epoll_manager::{EventOps, EventSet, Events, MutEventSubscriber};
use dbs_utils::rate_limiter::{BucketUpdate, RateLimiter, TokenType};
use fuse_backend_rs::abi::virtio_fs::RemovemappingOne;
use fuse_backend_rs::api::server::Server;
use fuse_backend_rs::api::Vfs;
use fuse_backend_rs::transport::{FsCacheReqHandler, Reader, VirtioFsWriter, Writer};
use log::{debug, error, info, trace};
use threadpool::ThreadPool;
use virtio_queue::{QueueOwnedT, QueueT};
use vm_memory::{GuestAddressSpace, GuestMemoryRegion};
use vmm_sys_util::eventfd::EventFd;

use crate::{Error, Result, VirtioDeviceConfig};

use super::{Error as FsError, VIRTIO_FS_NAME};

// New descriptors are pending on the virtio queue.
const QUEUE_AVAIL_EVENT: u32 = 0;

// two rate limiter events
const RATE_LIMITER_EVENT_COUNT: u32 = 2;

/// CacheHandler handles DAX window mmap/unmap operations
#[derive(Clone)]
pub struct CacheHandler {
    /// the size of memory region allocated for virtiofs
    pub(crate) cache_size: u64,

    /// the address of mmap region corresponding to the memory region
    pub(crate) mmap_cache_addr: u64,

    /// the device ID
    pub(crate) id: String,
}

impl CacheHandler {
    /// Make sure request is within cache range
    fn is_req_valid(&self, offset: u64, len: u64) -> bool {
        // TODO: do we need to validate alignment here?
        match offset.checked_add(len) {
            Some(n) => n <= self.cache_size,
            None => false,
        }
    }
}

impl FsCacheReqHandler for CacheHandler {
    // Do not close fd in here. The fd is automatically closed in the setupmapping
    // of passthrough_fs when destructing.
    fn map(
        &mut self,
        foffset: u64,
        moffset: u64,
        len: u64,
        flags: u64,
        fd: RawFd,
    ) -> std::result::Result<(), IOError> {
        let addr = self.mmap_cache_addr + moffset;
        trace!(
            target: VIRTIO_FS_NAME,
            "{}: CacheHandler::map(): fd={}, foffset=0x{:x}, moffset=0x{:x}(host addr: 0x{:x}), len=0x{:x}, flags=0x{:x}",
            self.id,
            fd,
            foffset,
            moffset,
            addr,
            len,
            flags
        );

        if !self.is_req_valid(moffset, len) {
            error!(
                "{}: CacheHandler::map(): Wrong offset or length, offset=0x{:x} len=0x{:x} cache_size=0x{:x}",
                self.id, moffset, len, self.cache_size
            );
            return Err(IOError::from_raw_os_error(libc::EINVAL));
        }

        // TODO:
        // In terms of security, DAX does not easily handle all kinds of write
        // scenarios, especially append write. Therefore, to prevent guest users
        // from using the DAX to write files maliciously, we do not support guest
        // write permission configuration. If DAX needs to support write, we can
        // add write permissions by Control path.
        let ret = unsafe {
            libc::mmap(
                addr as *mut libc::c_void,
                len as usize,
                libc::PROT_READ,
                libc::MAP_SHARED | libc::MAP_FIXED,
                fd,
                foffset as libc::off_t,
            )
        };
        if ret == libc::MAP_FAILED {
            let e = IOError::last_os_error();
            error!("{}: CacheHandler::map() failed: {}", VIRTIO_FS_NAME, e);
            return Err(e);
        }

        Ok(())
    }

    fn unmap(&mut self, requests: Vec<RemovemappingOne>) -> std::result::Result<(), IOError> {
        trace!(target: VIRTIO_FS_NAME, "{}: CacheHandler::unmap()", self.id,);

        for req in requests {
            let mut offset = req.moffset;
            let mut len = req.len;

            // Ignore if the length is 0.
            if len == 0 {
                continue;
            }

            debug!(
                "{}: do unmap(): offset=0x{:x} len=0x{:x} cache_size=0x{:x}",
                self.id, offset, len, self.cache_size
            );

            // Need to handle a special case where the slave ask for the unmapping
            // of the entire mapping.
            if len == 0xffff_ffff_ffff_ffff {
                len = self.cache_size;
                offset = 0;
            }

            if !self.is_req_valid(offset, len) {
                error!(
                    "{}: CacheHandler::unmap(): Wrong offset or length, offset=0x{:x} len=0x{:x} cache_size=0x{:x}",
                    self.id, offset, len, self.cache_size
                );
                return Err(IOError::from_raw_os_error(libc::EINVAL));
            }

            let addr = self.mmap_cache_addr + offset;
            // Use mmap + PROT_NONE can reserve host userspace address while unmap memory.
            // In this way, guest will not be able to access the memory, and dragonball
            // also can reserve the HVA.
            let ret = unsafe {
                libc::mmap(
                    addr as *mut libc::c_void,
                    len as usize,
                    libc::PROT_NONE,
                    libc::MAP_ANONYMOUS | libc::MAP_PRIVATE | libc::MAP_FIXED,
                    -1,
                    0_i64,
                )
            };
            if ret == libc::MAP_FAILED {
                let e = IOError::last_os_error();
                error!("{}: CacheHandler::unmap() failed, {}", self.id, e);
                return Err(e);
            }
        }

        Ok(())
    }
}

pub(crate) struct VirtioFsEpollHandler<
    AS: 'static + GuestAddressSpace,
    Q: QueueT,
    R: GuestMemoryRegion,
> {
    pub(crate) config: Arc<Mutex<VirtioDeviceConfig<AS, Q, R>>>,
    server: Arc<Server<Arc<Vfs>>>,
    cache_handler: Option<CacheHandler>,
    thread_pool: Option<ThreadPool>,
    id: String,
    rate_limiter: RateLimiter,
    patch_rate_limiter_fd: EventFd,
    receiver: Option<mpsc::Receiver<(BucketUpdate, BucketUpdate)>>,
}

impl<AS, Q, R> VirtioFsEpollHandler<AS, Q, R>
where
    AS: GuestAddressSpace + Clone + Send,
    AS::T: Send,
    AS::M: Sync + Send,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Send + Sync + 'static,
{
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        config: VirtioDeviceConfig<AS, Q, R>,
        fs: Arc<Vfs>,
        cache_handler: Option<CacheHandler>,
        thread_pool_size: u16,
        id: String,
        rate_limiter: RateLimiter,
        patch_rate_limiter_fd: EventFd,
        receiver: Option<mpsc::Receiver<(BucketUpdate, BucketUpdate)>>,
    ) -> Self {
        let thread_pool = if thread_pool_size > 0 {
            Some(ThreadPool::with_name(
                "virtiofs-thread".to_string(),
                thread_pool_size as usize,
            ))
        } else {
            None
        };
        Self {
            config: Arc::new(Mutex::new(config)),
            server: Arc::new(Server::new(fs)),
            cache_handler,
            thread_pool,
            id,
            rate_limiter,
            patch_rate_limiter_fd,
            receiver,
        }
    }

    fn process_queue(&mut self, queue_index: usize) -> Result<()> {
        let mut config_guard = self.config.lock().unwrap();
        let mem = config_guard.lock_guest_memory();
        let vm_as = config_guard.vm_as.clone();
        let queue = &mut config_guard.queues[queue_index];
        let (tx, rx) = mpsc::channel::<(u16, u32)>();
        let mut used_count = 0;
        let mut rate_limited = false;
        // TODO: use multiqueue to process new entries.

        let mut queue_guard = queue.queue_mut().lock();
        let mut iter = queue_guard
            .iter(mem.clone())
            .map_err(Error::VirtioQueueError)?;

        for desc_chain in &mut iter {
            // Prepare a set of objects that can be moved to the worker thread.
            if !self.rate_limiter.consume(1, TokenType::Ops) {
                rate_limited = true;
                break;
            }

            let head_index = desc_chain.head_index();
            let server = self.server.clone();
            let vm_as = vm_as.clone();
            let config = self.config.clone();
            let pooled = self.is_multi_thread();
            let tx = tx.clone();
            used_count += 1;
            let mut cache_handler = self.cache_handler.clone();

            let work_func = move || {
                let guard = vm_as.memory();
                let mem = guard.deref();
                let reader = Reader::from_descriptor_chain(mem, desc_chain.clone())
                    .map_err(FsError::InvalidDescriptorChain)
                    .unwrap();
                let writer = Writer::VirtioFs(
                    VirtioFsWriter::new(mem, desc_chain)
                        .map_err(FsError::InvalidDescriptorChain)
                        .unwrap(),
                );
                let total = server
                    .handle_message(
                        reader,
                        writer,
                        cache_handler
                            .as_mut()
                            .map(|x| x as &mut dyn FsCacheReqHandler),
                        None,
                    )
                    .map_err(FsError::ProcessQueue)
                    .unwrap();

                if pooled {
                    let queue = &mut config.lock().unwrap().queues[queue_index];
                    queue.add_used(mem, head_index, total as u32);
                    if let Err(e) = queue.notify() {
                        error!("failed to signal used queue: {:?}", e);
                    }
                } else {
                    tx.send((head_index, total as u32))
                        .expect("virtiofs: failed to send fuse result");
                }
            };

            if let Some(pool) = &self.thread_pool {
                trace!("{}: poping new fuse req to thread pool.", VIRTIO_FS_NAME,);
                pool.execute(work_func);
            } else {
                work_func();
            }
        }
        if rate_limited {
            iter.go_to_previous_position();
        }

        let notify = !self.is_multi_thread() && used_count > 0;
        // unlock QueueT
        drop(queue_guard);
        while !self.is_multi_thread() && used_count > 0 {
            used_count -= 1;
            let (idx, ret) = rx
                .recv()
                .expect("virtiofs: failed to recv result from thread pool");
            queue.add_used(mem.deref(), idx, ret);
        }

        if notify {
            if let Err(e) = queue.notify() {
                error!("failed to signal used queue: {:?}", e);
            }
        }

        Ok(())
    }

    pub fn get_patch_rate_limiters(&mut self, bytes: BucketUpdate, ops: BucketUpdate) {
        info!("{}: Update rate limiter for fs device", VIRTIO_FS_NAME);
        match &bytes {
            BucketUpdate::Update(tb) => {
                info!(
                    "{}: update bandwidth, \"size\": {}, \"one_time_burst\": {}, \"refill_time\": {}",
                    VIRTIO_FS_NAME,
                    tb.capacity(),
                    tb.one_time_burst(),
                    tb.refill_time_ms()
                );
            }
            BucketUpdate::None => {
                info!("{}: no update for bandwidth", VIRTIO_FS_NAME);
            }
            _ => {
                info!("{}: bandwidth limiting is disabled", VIRTIO_FS_NAME);
            }
        }
        match &ops {
            BucketUpdate::Update(tb) => {
                info!(
                    "{}: update ops, \"size\": {}, \"one_time_burst\": {}, \"refill_time\": {}",
                    VIRTIO_FS_NAME,
                    tb.capacity(),
                    tb.one_time_burst(),
                    tb.refill_time_ms()
                );
            }
            BucketUpdate::None => {
                info!("{}: no update for ops", VIRTIO_FS_NAME);
            }
            _ => {
                info!("{}: ops limiting is disabled", VIRTIO_FS_NAME);
            }
        }
        self.rate_limiter.update_buckets(bytes, ops);
    }

    // True if thread pool is enabled.
    fn is_multi_thread(&self) -> bool {
        self.thread_pool.is_some()
    }
}

impl<AS, Q, R> MutEventSubscriber for VirtioFsEpollHandler<AS, Q, R>
where
    AS: GuestAddressSpace + Send + Sync + 'static + Clone,
    AS::T: Send,
    AS::M: Sync + Send,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Send + Sync + 'static,
{
    fn process(&mut self, events: Events, _ops: &mut EventOps) {
        trace!(
            target: VIRTIO_FS_NAME,
            "{}: VirtioFsHandler::process({})",
            self.id,
            events.data()
        );

        let slot = events.data();
        let config = &self.config.clone();
        let guard = config.lock().unwrap();
        let queues = &guard.queues;

        let queues_len = queues.len() as u32;
        // Rate limiter budget is now available.
        let rate_limiter_event = QUEUE_AVAIL_EVENT + queues_len;
        // patch request of rate limiter has arrived
        let patch_rate_limiter_event = rate_limiter_event + 1;

        match slot {
            s if s >= RATE_LIMITER_EVENT_COUNT + QUEUE_AVAIL_EVENT + queues_len => {
                error!("{}: unknown epoll event slot {}", VIRTIO_FS_NAME, slot);
            }

            s if s == rate_limiter_event => match self.rate_limiter.event_handler() {
                Ok(()) => {
                    drop(guard);
                    for idx in QUEUE_AVAIL_EVENT as usize..(QUEUE_AVAIL_EVENT + queues_len) as usize
                    {
                        if let Err(e) = self.process_queue(idx) {
                            error!("{}: error in queue {}, {:?}", VIRTIO_FS_NAME, idx, e);
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "{}: the rate limiter is disabled or is not blocked, {:?}",
                        VIRTIO_FS_NAME, e
                    );
                }
            },

            s if s == patch_rate_limiter_event => {
                if let Err(e) = self.patch_rate_limiter_fd.read() {
                    error!("{}: failed to get patch event, {:?}", VIRTIO_FS_NAME, e);
                }
                if let Some(receiver) = &self.receiver {
                    if let Ok((bytes, ops)) = receiver.try_recv() {
                        self.get_patch_rate_limiters(bytes, ops);
                    }
                }
            }

            // QUEUE_AVAIL_EVENT
            _ => {
                let idx = (slot - QUEUE_AVAIL_EVENT) as usize;
                if let Err(e) = queues[idx].consume_event() {
                    error!("{}: failed to read queue event, {:?}", VIRTIO_FS_NAME, e);
                    return;
                }
                drop(guard);

                if let Err(e) = self.process_queue(idx) {
                    error!(
                        "{}: process_queue failed due to error {:?}",
                        VIRTIO_FS_NAME, e
                    );
                }
            }
        }
    }

    fn init(&mut self, ops: &mut EventOps) {
        trace!(
            target: VIRTIO_FS_NAME,
            "{}: VirtioFsHandler::init()",
            self.id
        );

        let queues = &self.config.lock().unwrap().queues;

        for (idx, queue) in queues.iter().enumerate() {
            let events = Events::with_data(
                queue.eventfd.as_ref(),
                QUEUE_AVAIL_EVENT + idx as u32,
                EventSet::IN,
            );
            if let Err(e) = ops.add(events) {
                error!(
                    "{}: failed to register epoll event for event queue {}, {:?}",
                    VIRTIO_FS_NAME, idx, e
                );
            }
        }

        let rate_limiter_fd = self.rate_limiter.as_raw_fd();
        if rate_limiter_fd != -1 {
            if let Err(e) = ops.add(Events::with_data_raw(
                rate_limiter_fd,
                QUEUE_AVAIL_EVENT + queues.len() as u32,
                EventSet::IN,
            )) {
                error!(
                    "{}: failed to register rate limiter event, {:?}",
                    VIRTIO_FS_NAME, e
                );
            }
        }

        if let Err(e) = ops.add(Events::with_data(
            &self.patch_rate_limiter_fd,
            1 + QUEUE_AVAIL_EVENT + queues.len() as u32,
            EventSet::IN,
        )) {
            error!(
                "{}: failed to register rate limiter patch event {:?}",
                VIRTIO_FS_NAME, e
            );
        }
    }
}

#[cfg(test)]
pub mod tests {
    use std::io::Seek;
    use std::io::Write;
    use std::sync::Arc;

    use dbs_interrupt::NoopNotifier;
    use dbs_utils::epoll_manager::EpollManager;
    use dbs_utils::epoll_manager::SubscriberOps;
    use dbs_utils::rate_limiter::TokenBucket;
    use vm_memory::{GuestAddress, GuestMemoryMmap};
    use vmm_sys_util::tempfile::TempFile;

    use super::*;
    use crate::fs::device::tests::*;
    use crate::fs::*;
    use crate::tests::{VirtQueue, VIRTQ_DESC_F_NEXT, VIRTQ_DESC_F_WRITE};
    use crate::VirtioQueueConfig;

    #[test]
    fn test_is_req_valid() {
        let handler = CacheHandler {
            cache_size: 0x1000,
            mmap_cache_addr: 0x1000,
            id: "test".to_string(),
        };

        // Normal case.
        assert!(handler.is_req_valid(0x0, 0x500));

        // Invalid case.
        assert!(!handler.is_req_valid(0x500, 0x1000));
    }

    #[test]
    fn test_map() {
        let mmap_addr = 0x10000;
        let moffset = 0x5000;
        let mut handler = CacheHandler {
            cache_size: 0x10000,
            mmap_cache_addr: mmap_addr,
            id: "test".to_string(),
        };

        // Normal case.
        let original_content = b"hello world";
        let mut file = TempFile::new().unwrap().into_file();
        file.set_len(0x1000).unwrap();
        file.write_all(original_content).unwrap();
        file.rewind().unwrap();
        let fd = file.as_raw_fd();
        handler.map(0x0, moffset, 0x5000, 0, fd).unwrap();
        let mapped_addr = (mmap_addr + moffset) as *const [u8; 11];
        unsafe {
            let content = mapped_addr.read();
            assert_eq!(&content, original_content);
        }

        // Invalid argument case.
        assert!(matches!(
            handler
                .map(0x0, 0x5000, 0xc000, 0, fd)
                .err()
                .unwrap()
                .kind(),
            std::io::ErrorKind::InvalidInput
        ));

        // Bad file descriptor case.
        let fd = TempFile::new().unwrap().as_file().as_raw_fd();
        assert!(format!(
            "{:?}",
            handler.map(0x0, 0x5000, 0x5000, 0, fd).err().unwrap()
        )
        .contains("Bad file descriptor"));
    }

    #[test]
    fn test_unmap() {
        let mmap_addr = 0x10000;
        let moffset = 0x5000;
        let mut handler = CacheHandler {
            cache_size: 0x10000,
            mmap_cache_addr: mmap_addr,
            id: "test".to_string(),
        };

        // Normal case after map.
        let original_content = b"hello world";
        let mut file = TempFile::new().unwrap().into_file();
        file.set_len(0x1000).unwrap();
        file.write_all(original_content).unwrap();
        file.rewind().unwrap();
        let fd = file.as_raw_fd();
        handler.map(0x0, moffset, 0x5000, 0, fd).unwrap();
        let mapped_addr = (mmap_addr + moffset) as *const [u8; 11];
        unsafe {
            let content = mapped_addr.read();
            assert_eq!(&content, original_content);
        }
        let requests = vec![
            RemovemappingOne {
                moffset: 0x5000,
                len: 0x1000,
            },
            RemovemappingOne {
                moffset: 0x6000,
                len: 0x2500,
            },
        ];
        assert!(handler.unmap(requests).is_ok());

        // Normal case.
        let mut handler = CacheHandler {
            cache_size: 0x10000,
            mmap_cache_addr: mmap_addr,
            id: "test".to_string(),
        };
        let requests = vec![
            RemovemappingOne {
                moffset: 0x5000,
                len: 0x1000,
            },
            RemovemappingOne {
                moffset: 0x6000,
                len: 0x2500,
            },
        ];
        assert!(handler.unmap(requests).is_ok());

        // Invalid argument case.
        let requests = vec![RemovemappingOne {
            moffset: 0x5000,
            len: 0x10000,
        }];
        assert!(matches!(
            handler.unmap(requests).err().unwrap().kind(),
            std::io::ErrorKind::InvalidInput
        ));
    }

    #[test]
    fn test_fs_get_patch_rate_limiters() {
        let mut handler = create_fs_epoll_handler(String::from("1"));
        let tokenbucket = TokenBucket::new(1, 1, 4);

        handler.get_patch_rate_limiters(
            BucketUpdate::None,
            BucketUpdate::Update(tokenbucket.clone()),
        );
        assert_eq!(handler.rate_limiter.ops().unwrap(), &tokenbucket);

        handler.get_patch_rate_limiters(
            BucketUpdate::Update(tokenbucket.clone()),
            BucketUpdate::None,
        );
        assert_eq!(handler.rate_limiter.bandwidth().unwrap(), &tokenbucket);

        handler.get_patch_rate_limiters(BucketUpdate::None, BucketUpdate::None);
        assert_eq!(handler.rate_limiter.ops().unwrap(), &tokenbucket);

        handler.get_patch_rate_limiters(BucketUpdate::None, BucketUpdate::Disabled);
        assert_eq!(handler.rate_limiter.ops(), None);

        handler.get_patch_rate_limiters(BucketUpdate::Disabled, BucketUpdate::None);
        assert_eq!(handler.rate_limiter.bandwidth(), None);
    }

    #[test]
    fn test_fs_set_patch_rate_limiters() {
        let epoll_manager = EpollManager::default();
        let rate_limiter = RateLimiter::new(100, 0, 300, 10, 0, 300).unwrap();
        let mut fs: VirtioFs<Arc<GuestMemoryMmap>> = VirtioFs::new(
            TAG,
            NUM_QUEUES,
            QUEUE_SIZE,
            CACHE_SIZE,
            CACHE_POLICY,
            THREAD_NUM,
            WB_CACHE,
            NO_OPEN,
            KILLPRIV_V2,
            XATTR,
            DROP_SYS_RSC,
            NO_READDIR,
            new_dummy_handler_helper(),
            epoll_manager,
            Some(rate_limiter),
        )
        .unwrap();

        // No sender
        assert!(fs
            .set_patch_rate_limiters(BucketUpdate::None, BucketUpdate::None)
            .is_err());

        // Success
        let (sender, receiver) = mpsc::channel();
        fs.sender = Some(sender);
        assert!(fs
            .set_patch_rate_limiters(BucketUpdate::None, BucketUpdate::None)
            .is_ok());

        // Send error
        drop(receiver);
        assert!(fs
            .set_patch_rate_limiters(BucketUpdate::None, BucketUpdate::None)
            .is_err());
    }

    #[test]
    fn test_fs_epoll_handler_handle_event() {
        let handler = create_fs_epoll_handler("test_1".to_string());
        let event_fd = EventFd::new(0).unwrap();
        let mgr = EpollManager::default();
        let id = mgr.add_subscriber(Box::new(handler));
        let mut inner_mgr = mgr.mgr.lock().unwrap();
        let mut event_op = inner_mgr.event_ops(id).unwrap();
        let event_set = EventSet::EDGE_TRIGGERED;
        let mut handler = create_fs_epoll_handler("test_2".to_string());

        // test for QUEUE_AVAIL_EVENT
        let events = Events::with_data(&event_fd, QUEUE_AVAIL_EVENT, event_set);
        handler.process(events, &mut event_op);
        handler.config.lock().unwrap().queues[0]
            .generate_event()
            .unwrap();
        handler.process(events, &mut event_op);

        // test for RATE_LIMITER_EVENT
        let queues_len = handler.config.lock().unwrap().queues.len() as u32;
        let events = Events::with_data(&event_fd, QUEUE_AVAIL_EVENT + queues_len, event_set);
        handler.process(events, &mut event_op);

        // test for PATCH_RATE_LIMITER_EVENT
        if let Err(e) = handler.patch_rate_limiter_fd.write(1) {
            error!(
                "{} test: failed to write patch_rate_limiter_fd, {:?}",
                VIRTIO_FS_NAME, e
            );
        }
        let events = Events::with_data(&event_fd, 1 + QUEUE_AVAIL_EVENT + queues_len, event_set);
        handler.process(events, &mut event_op);
    }

    #[test]
    fn test_fs_epoll_handler_handle_unknown_event() {
        let handler = create_fs_epoll_handler("test_1".to_string());
        let event_fd = EventFd::new(0).unwrap();
        let mgr = EpollManager::default();
        let id = mgr.add_subscriber(Box::new(handler));
        let mut inner_mgr = mgr.mgr.lock().unwrap();
        let mut event_op = inner_mgr.event_ops(id).unwrap();
        let event_set = EventSet::EDGE_TRIGGERED;
        let mut handler = create_fs_epoll_handler("test_2".to_string());

        // test for unknown event
        let events = Events::with_data(&event_fd, FS_EVENTS_COUNT + 10, event_set);
        handler.process(events, &mut event_op);
    }

    #[test]
    fn test_fs_epoll_handler_process_queue() {
        {
            let mut handler = create_fs_epoll_handler("test_1".to_string());

            let m = &handler.config.lock().unwrap().vm_as.clone();
            let vq = VirtQueue::new(GuestAddress(0), m, 16);
            vq.avail.ring(0).store(0);
            vq.avail.idx().store(1);
            let q = vq.create_queue();
            vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
            vq.dtable(1)
                .set(0x2000, 0x1000, VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE, 2);
            vq.dtable(2).set(0x3000, 1, VIRTQ_DESC_F_WRITE, 1);

            handler.config.lock().unwrap().queues = vec![VirtioQueueConfig::new(
                q,
                Arc::new(EventFd::new(0).unwrap()),
                Arc::new(NoopNotifier::new()),
                0,
            )];
            assert!(handler.process_queue(0).is_ok());
        }
    }
}
