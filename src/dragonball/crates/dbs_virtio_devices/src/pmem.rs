// Copyright 2026 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::fs::File;
use std::marker::PhantomData;
use std::os::fd::AsRawFd;
use std::sync::Arc;
use std::{any::Any, ops::Deref};

use dbs_device::resources::{DeviceResources, ResourceConstraint};
use dbs_utils::epoll_manager::{
    EpollManager, EventOps, EventSet, Events, MutEventSubscriber, SubscriberId,
};
use kvm_bindings::{kvm_userspace_memory_region, KVM_MEM_READONLY};
use kvm_ioctls::VmFd;
use log::{error, trace};
use nix::sys::statfs::{fstatfs, HUGETLBFS_MAGIC};
use virtio_bindings::bindings::virtio_config::{VIRTIO_F_ACCESS_PLATFORM, VIRTIO_F_VERSION_1};
use virtio_queue::{QueueOwnedT, QueueSync, QueueT};
use vm_memory::{
    Bytes, GuestAddress, GuestAddressSpace, GuestMemoryRegion, GuestRegionMmap, MmapRegion,
};

use crate::{
    ActivateError, ActivateResult, ConfigResult, DbsGuestAddressSpace, Error, Result, VirtioDevice,
    VirtioDeviceConfig, VirtioDeviceInfo, VirtioRegionHandler, VirtioSharedMemoryList, TYPE_PMEM,
};

const VIRTIO_PMEM_REQ_TYPE_FLUSH: u32 = 0;
const QUEUE_SIZE: u16 = 256;
const NUM_QUEUES: usize = 1;
const QUEUE_SIZES: &[u16] = &[QUEUE_SIZE];
/// Guest-visible granularity for the pmem region.
const PMEM_ALIGN: u64 = 2 * 1024 * 1024;

pub(crate) struct PmemEpollHandler<
    AS: GuestAddressSpace,
    Q: QueueT + Send = QueueSync,
    R: GuestMemoryRegion = GuestRegionMmap,
> {
    id: String,
    config: VirtioDeviceConfig<AS, Q, R>,
    region: Arc<GuestRegionMmap>,
    _mapping: Arc<MmapRegion>,
    size: u64,
}

impl<AS: DbsGuestAddressSpace, Q: QueueT + Send, R: GuestMemoryRegion> PmemEpollHandler<AS, Q, R> {
    fn do_flush(region: &GuestRegionMmap, size: u64, id: &str) -> i32 {
        // SAFETY: The caller keeps the file-backed mapping alive for `size` bytes.
        let ret = unsafe {
            libc::msync(
                region.as_ptr() as *mut libc::c_void,
                size as usize,
                libc::MS_SYNC,
            )
        };
        if ret < 0 {
            error!(
                target: "virtio-pmem",
                "{}: msync failed: {}",
                id,
                std::io::Error::last_os_error(),
            );
            -1
        } else {
            0
        }
    }

    fn process_queue(&mut self, queue_index: usize) -> bool {
        let guard = self.config.lock_guest_memory();
        let mem = guard.deref().memory();
        let region = Arc::clone(&self.region);
        let size = self.size;
        let id = self.id.as_str();

        let queue = &mut self.config.queues[queue_index];
        let mut queue_guard = queue.queue_mut().lock();
        let queue_size = queue_guard.size();
        let mut used_desc_heads = Vec::with_capacity(queue_size as usize);

        let mut iter = match queue_guard.iter(mem) {
            Ok(iter) => iter,
            Err(e) => {
                error!(target: "virtio-pmem", "{}: failed to process queue: {}", self.id, e);
                return false;
            }
        };

        let mut flush_result: Option<i32> = None;
        let mut first_error: Option<String> = None;

        for mut desc_chain in &mut iter {
            let head_index = desc_chain.head_index();
            if head_index >= queue_size {
                first_error.get_or_insert_with(|| {
                    format!(
                        "invalid descriptor chain head index {head_index}, queue size {queue_size}"
                    )
                });
                continue;
            }

            let mut len = 0;
            if let (Some(req), Some(resp)) = (desc_chain.next(), desc_chain.next()) {
                let malformed = req.is_write_only()
                    || req.len() != 4
                    || !resp.is_write_only()
                    || resp.len() != 4;
                let req_type = if malformed {
                    None
                } else {
                    mem.read_obj::<[u8; 4]>(req.addr())
                        .ok()
                        .map(u32::from_le_bytes)
                };

                match req_type {
                    Some(VIRTIO_PMEM_REQ_TYPE_FLUSH) => {
                        let status =
                            flush_result.get_or_insert_with(|| Self::do_flush(&region, size, id));
                        match mem.write_obj(status.to_le_bytes(), resp.addr()) {
                            Ok(_) => len = 4,
                            Err(e) => {
                                first_error
                                    .get_or_insert_with(|| format!("failed to write status: {e}"));
                            }
                        }
                    }
                    Some(t) => {
                        first_error
                            .get_or_insert_with(|| format!("unsupported request type {t:#x}"));
                    }
                    None => {
                        first_error.get_or_insert_with(|| "malformed flush request".into());
                    }
                }
            } else {
                first_error.get_or_insert_with(|| "descriptor chain too short".into());
            }
            used_desc_heads.push((head_index, len));
        }

        if let Some(e) = first_error {
            error!(
                target: "virtio-pmem",
                "{}: failed to process pmem queue: {}",
                self.id, e
            );
        }

        let mut nr_used = 0;
        for &(desc_index, len) in &used_desc_heads {
            match queue_guard.add_used(mem, desc_index, len) {
                Ok(()) => nr_used += 1,
                Err(e) => error!(
                    target: "virtio-pmem",
                    "{}: failed to add used descriptor {}: {}",
                    self.id,
                    desc_index,
                    e
                ),
            }
        }
        nr_used > 0
    }
}

impl<AS, Q, R> MutEventSubscriber for PmemEpollHandler<AS, Q, R>
where
    AS: 'static + GuestAddressSpace + Send + Sync,
    Q: QueueT + Send,
    R: GuestMemoryRegion,
{
    fn init(&mut self, ops: &mut EventOps) {
        for (idx, queue) in self.config.queues.iter().enumerate() {
            let event = Events::with_data(queue.eventfd.as_ref(), idx as u32, EventSet::IN);
            if let Err(e) = ops.add(event) {
                error!(target: "virtio-pmem", "{}: failed to register queue event: {:?}", self.id, e);
            }
        }
    }

    fn process(&mut self, events: Events, _ops: &mut EventOps) {
        let idx = events.data() as usize;
        if idx >= self.config.queues.len() {
            error!(target: "virtio-pmem", "{}: invalid queue index {}", self.id, idx);
            return;
        }
        if let Err(e) = self.config.queues[idx].consume_event() {
            error!(target: "virtio-pmem", "{}: failed to get queue event: {:?}", self.id, e);
        } else if self.process_queue(idx) {
            if let Err(e) = self.config.queues[idx].notify() {
                error!(target: "virtio-pmem", "{}: failed to signal used queue: {}", self.id, e);
            }
        }
    }
}

pub struct Pmem<AS: GuestAddressSpace> {
    pub(crate) id: String,
    pub(crate) device_info: VirtioDeviceInfo,
    pub(crate) file: Option<File>,
    /// Backing-file byte range; `len` is the padded guest-visible length.
    pub(crate) file_range: Option<std::range::Range<u64>>,
    pub(crate) len: u64,
    pub(crate) read_only: bool,
    pub(crate) region: Option<Arc<GuestRegionMmap>>,
    pub(crate) mapping: Option<Arc<MmapRegion>>,
    pub(crate) handler: Box<dyn VirtioRegionHandler>,
    pub(crate) kvm_slot: Option<(Arc<VmFd>, u32)>,
    pub(crate) subscriber_id: Option<SubscriberId>,
    pub(crate) phantom: PhantomData<AS>,
}

impl<AS: GuestAddressSpace> Pmem<AS> {
    fn file_page_size(file: &File) -> Result<u64> {
        let stat = fstatfs(file).map_err(std::io::Error::from)?;
        if stat.filesystem_type() == HUGETLBFS_MAGIC {
            Ok(stat.block_size() as u64)
        } else {
            // SAFETY: sysconf only queries the host page size.
            Ok(unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64)
        }
    }

    /// Create a new virtio-pmem device backed by `file`.
    pub fn new(
        id: String,
        file: File,
        file_range: Option<std::range::Range<u64>>,
        read_only: bool,
        handler: Box<dyn VirtioRegionHandler>,
        epoll_mgr: EpollManager,
        f_access_platform: bool,
    ) -> Result<Self> {
        // SAFETY: fcntl only queries access flags on the live backing fd.
        let access = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
        if access < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let mode = access & libc::O_ACCMODE;
        if mode != libc::O_RDWR && !(read_only && mode == libc::O_RDONLY) {
            return Err(Error::InvalidInput);
        }
        let file_size = file.metadata()?.len();
        let file_range = match file_range {
            Some(range) => range,
            None => std::range::Range {
                start: 0,
                end: file_size,
            },
        };
        let file_page_size = Self::file_page_size(&file)?;
        // SAFETY: sysconf only queries the host page size.
        let host_page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
        if file_range.end > file_size
            || file_range.start % file_page_size != 0
            || (file_page_size > host_page_size && file_range.end % file_page_size != 0)
            || file_range.start > libc::off_t::MAX as u64
        {
            return Err(Error::InvalidInput);
        }
        let size = file_range
            .end
            .checked_sub(file_range.start)
            .filter(|&s| s > 0)
            .ok_or(Error::InvalidInput)?;

        let mut avail_features = 1u64 << VIRTIO_F_VERSION_1;
        if f_access_platform {
            avail_features |= 1u64 << VIRTIO_F_ACCESS_PLATFORM;
        }
        let len = size
            .checked_next_multiple_of(PMEM_ALIGN.max(file_page_size))
            .ok_or(Error::InvalidInput)?;
        let mut config = Vec::with_capacity(16);
        config.extend_from_slice(&0u64.to_le_bytes());
        config.extend_from_slice(&len.to_le_bytes());

        let device_info = VirtioDeviceInfo::new(
            "virtio-pmem".into(),
            avail_features,
            Arc::new(vec![QUEUE_SIZE; NUM_QUEUES]),
            config,
            epoll_mgr,
        );

        Ok(Self {
            id,
            device_info,
            file: Some(file),
            file_range: Some(file_range),
            len,
            read_only,
            region: None,
            mapping: None,
            handler,
            kvm_slot: None,
            subscriber_id: None,
            phantom: PhantomData,
        })
    }

    fn map_file(
        file: &File,
        range: &std::range::Range<u64>,
        len: u64,
        read_only: bool,
    ) -> Result<(MmapRegion, MmapRegion, u32)> {
        let page_size = Self::file_page_size(file)?;
        // SAFETY: sysconf only queries the host page size.
        let host_page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
        let size = range
            .end
            .checked_sub(range.start)
            .filter(|&size| size > 0)
            .ok_or(Error::InvalidInput)?;
        if range.end > file.metadata()?.len()
            || range.start & (page_size - 1) != 0
            || (page_size > host_page_size && size & (page_size - 1) != 0)
            || range.start > libc::off_t::MAX as u64
            || size > len
            || len & (page_size - 1) != 0
        {
            return Err(Error::InvalidInput);
        }
        let reserved_len = len.checked_add(page_size).ok_or(Error::InvalidInput)?;
        let prot = libc::PROT_READ | if read_only { 0 } else { libc::PROT_WRITE };
        let flags = libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NORESERVE;
        let mapping = MmapRegion::build(None, reserved_len as usize, prot, flags)
            .map_err(Error::NewMmapRegion)?;
        let base = mapping.as_ptr() as usize;
        let offset = (page_size as usize - base % page_size as usize) % page_size as usize;
        // SAFETY: The aligned view and file mapping fit inside the owned reservation.
        let mmap = unsafe {
            let ptr = mapping.as_ptr().add(offset);
            let mapped = libc::mmap(
                ptr.cast(),
                size as usize,
                prot,
                libc::MAP_SHARED | libc::MAP_FIXED | libc::MAP_NORESERVE,
                file.as_raw_fd(),
                range.start as libc::off_t,
            );
            if mapped == libc::MAP_FAILED {
                return Err(std::io::Error::last_os_error().into());
            }
            MmapRegion::build_raw(ptr, len as usize, prot, flags).map_err(Error::NewMmapRegion)?
        };
        Ok((mapping, mmap, if read_only { KVM_MEM_READONLY } else { 0 }))
    }
}
