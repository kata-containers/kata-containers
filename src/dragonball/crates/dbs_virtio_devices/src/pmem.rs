// Copyright 2026 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::any::Any;
use std::fs::File;
use std::marker::PhantomData;
use std::ops::{Deref, Range};
use std::os::fd::AsRawFd;
use std::sync::Arc;

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

const QUEUE_SIZE: u16 = 256;
const NUM_QUEUES: usize = 1;
const QUEUE_SIZES: &[u16] = &[QUEUE_SIZE];
const VIRTIO_PMEM_REQ_TYPE_FLUSH: u32 = 0;
/// Guest-visible granularity: a pmem region must start at a 2MiB boundary and
/// span a 2MiB multiple, so misaligned sizes are padded up.
const PMEM_ALIGN: u64 = 2 * 1024 * 1024;

pub(crate) struct PmemEpollHandler<
    AS: GuestAddressSpace,
    Q: QueueT + Send = QueueSync,
    R: GuestMemoryRegion = GuestRegionMmap,
> {
    id: String,
    config: VirtioDeviceConfig<AS, Q, R>,
    region: Arc<GuestRegionMmap>,
    // The aligned region is a non-owning view into this mapping.
    _mapping: Arc<MmapRegion>,
    /// Length of the file-backed part of the mapping, i.e. the msync length.
    size: u64,
}

impl<AS: DbsGuestAddressSpace, Q: QueueT + Send, R: GuestMemoryRegion> PmemEpollHandler<AS, Q, R> {
    /// Flush the backing file: one synchronous msync over the mapped range.
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
        // Hoisted out of the closures below: edition 2018 closures capture the
        // whole `self`, which conflicts with the mutable queue borrow.
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

        // A single msync covers all flush requests in one queue notification.
        let mut flush_result: Option<i32> = None;
        // A malformed queue can fail on every chain in a row, so only report
        // the first failure for each queue notification to keep a hostile
        // guest from flooding the host log.
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
                        // A single msync per queue notification covers every
                        // request in the batch.
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

        // The queue state lock is held for the whole batch: the locked view
        // (`Queue`) offers an unlocked `add_used`, so the used-ring updates
        // stay inside the same critical section without re-locking.
        if let Some(e) = first_error {
            error!(
                target: "virtio-pmem",
                "{}: failed to process pmem queue: {}",
                self.id, e
            );
        }

        let mut nr_used = 0;
        for &(desc_index, len) in &used_desc_heads {
            // Do not use `VirtioQueueConfig::add_used()` here: it panics on
            // failure, which would take down the event thread shared by every
            // virtio device of the VM.
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
    pub(crate) file_range: Option<Range<u64>>,
    pub(crate) len: u64,
    pub(crate) read_only: bool,
    pub(crate) region: Option<Arc<GuestRegionMmap>>,
    pub(crate) mapping: Option<Arc<MmapRegion>>,
    pub(crate) handler: Box<dyn VirtioRegionHandler>,
    pub(crate) region_registered: bool,
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
    ///
    /// If `file_range` is None, the entire backing file is used.
    pub fn new(
        id: String,
        file: File,
        file_range: Option<Range<u64>>,
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
            None => 0..file_size,
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

        // Config space: { le64 start; le64 size; }. Back-filled by set_resource().
        // `size` is relaxed: the guest-visible length is padded up to PMEM_ALIGN,
        // matching the firecracker device; the tail past `size` maps anonymous
        // pages that are not synchronized with the backing file.
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
            region_registered: false,
            kvm_slot: None,
            subscriber_id: None,
            phantom: PhantomData,
        })
    }

    fn map_file(
        file: &File,
        range: &Range<u64>,
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

    fn unregister_memory(&mut self) {
        let mut leak_mapping = false;
        if let Some((vm_fd, slot)) = self.kvm_slot.take() {
            let region = kvm_userspace_memory_region {
                slot,
                memory_size: 0,
                ..Default::default()
            };
            // SAFETY: A zero-sized region unregisters the slot.
            if let Err(e) = unsafe { vm_fd.set_user_memory_region(region) } {
                error!(target: "virtio-pmem", "{}: failed to unregister KVM slot: {}", self.id, e);
                leak_mapping = true;
            }
        }
        if self.region_registered {
            if let Some(region) = self.region.as_ref() {
                if let Err(e) = self.handler.remove_regions(std::slice::from_ref(region)) {
                    error!(target: "virtio-pmem", "{}: failed to remove guest region: {}", self.id, e);
                    leak_mapping = true;
                } else {
                    self.region_registered = false;
                }
            } else {
                leak_mapping = true;
            }
        }
        if leak_mapping {
            if let Some(mapping) = self.mapping.take() {
                std::mem::forget(mapping);
            }
        }
    }
}

impl<AS, Q, R> VirtioDevice<AS, Q, R> for Pmem<AS>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Sync + Send + 'static,
{
    fn device_type(&self) -> u32 {
        TYPE_PMEM
    }

    fn queue_max_sizes(&self) -> &[u16] {
        QUEUE_SIZES
    }

    fn get_avail_features(&self, page: u32) -> u32 {
        self.device_info.get_avail_features(page)
    }

    fn set_acked_features(&mut self, page: u32, value: u32) {
        trace!(
            target: "virtio-pmem",
            "{}: VirtioDevice::set_acked_features({}, 0x{:x})",
            self.id,
            page,
            value
        );
        self.device_info.set_acked_features(page, value)
    }

    fn read_config(&mut self, offset: u64, data: &mut [u8]) -> ConfigResult {
        trace!(target: "virtio-pmem", "{}: VirtioDevice::read_config(0x{:x}, {:?})",
               self.id, offset, data);
        self.device_info.read_config(offset, data)
    }

    fn write_config(&mut self, offset: u64, data: &[u8]) -> ConfigResult {
        trace!(target: "virtio-pmem", "{}: ignore VirtioDevice::write_config(0x{:x}, {:?})",
               self.id, offset, data);
        Ok(())
    }

    fn activate(&mut self, config: VirtioDeviceConfig<AS, Q, R>) -> ActivateResult {
        trace!(target: "virtio-pmem", "{}: VirtioDevice::activate()", self.id);

        if self.subscriber_id.is_some() {
            return Err(ActivateError::InternalError);
        }
        self.device_info.check_queue_sizes(&config.queues)?;

        let region = self.region.clone().ok_or(ActivateError::InternalError)?;
        let mapping = self.mapping.clone().ok_or(ActivateError::InternalError)?;
        let file_range = self
            .file_range
            .as_ref()
            .ok_or(ActivateError::InternalError)?;
        let handler = Box::new(PmemEpollHandler {
            id: self.id.clone(),
            config,
            region,
            _mapping: mapping,
            size: file_range.end - file_range.start,
        });
        self.subscriber_id = Some(self.device_info.register_event_handler(handler));

        Ok(())
    }

    fn reset(&mut self) -> ActivateResult {
        if let Some(id) = self.subscriber_id.take() {
            self.device_info.remove_event_handler(id)?;
        }
        Ok(())
    }

    fn get_resource_requirements(
        &self,
        requests: &mut Vec<ResourceConstraint>,
        use_generic_irq: bool,
    ) {
        requests.push(ResourceConstraint::LegacyIrq { irq: None });
        if use_generic_irq {
            requests.push(ResourceConstraint::GenericIrq {
                size: (self.device_info.queue_sizes.len() + 1) as u32,
            });
        }

        // Keep device memory above 4GiB, outside normal guest RAM and legacy MMIO.
        requests.push(ResourceConstraint::MmioAddress {
            range: Some((0x1_0000_0000, u64::MAX)),
            align: PMEM_ALIGN,
            size: self.len,
        });
        requests.push(ResourceConstraint::KvmMemSlot {
            slot: None,
            size: 1,
        });
    }

    fn set_resource(
        &mut self,
        vm_fd: Arc<VmFd>,
        resource: DeviceResources,
    ) -> Result<Option<VirtioSharedMemoryList<R>>> {
        let ranges = resource.get_mmio_address_ranges();
        let slots = resource.get_kvm_mem_slots();
        if ranges.len() != 1 || slots.len() != 1 || self.kvm_slot.is_some() {
            return Err(Error::InvalidResource);
        }
        let (guest_addr, len) = ranges[0];
        if guest_addr % PMEM_ALIGN != 0 || len != self.len || guest_addr.checked_add(len).is_none()
        {
            return Err(Error::InvalidResource);
        }

        let file = self.file.as_ref().ok_or(Error::InternalError)?;
        let range = self.file_range.as_ref().ok_or(Error::InternalError)?;
        let (mapping, mmap, flags) = Self::map_file(file, range, len, self.read_only)?;

        let region = Arc::new(
            GuestRegionMmap::new(mmap, GuestAddress(guest_addr)).ok_or(Error::InsertMmap)?,
        );

        let kvm_region = kvm_userspace_memory_region {
            slot: slots[0],
            flags,
            guest_phys_addr: guest_addr,
            memory_size: len,
            userspace_addr: region.as_ptr() as u64,
        };

        // SAFETY: The allocated slot uses our live mapping, retained until slot removal.
        unsafe { vm_fd.set_user_memory_region(kvm_region) }.map_err(Error::SetUserMemoryRegion)?;

        if let Err(e) = self.handler.insert_regions(vec![region.clone()]) {
            let unregister = kvm_userspace_memory_region {
                slot: slots[0],
                memory_size: 0,
                ..Default::default()
            };
            // SAFETY: Undo the slot before releasing its mapping.
            if let Err(unregister_error) = unsafe { vm_fd.set_user_memory_region(unregister) } {
                error!(target: "virtio-pmem", "{}: failed to roll back KVM slot: {}", self.id, unregister_error);
                std::mem::forget(mapping);
            }
            return Err(e);
        }

        self.kvm_slot = Some((vm_fd, slots[0]));
        self.region = Some(region);
        self.mapping = Some(Arc::new(mapping));
        self.region_registered = true;
        self.device_info.config_space[..8].copy_from_slice(&guest_addr.to_le_bytes());

        Ok(None)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl<AS: GuestAddressSpace> Drop for Pmem<AS> {
    fn drop(&mut self) {
        if let Some(id) = self.subscriber_id.take() {
            if let Err(e) = self.device_info.remove_event_handler(id) {
                error!(target: "virtio-pmem", "{}: failed to remove event handler: {}", self.id, e);
            }
        }
        self.unregister_memory();
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::os::fd::FromRawFd;
    use std::sync::atomic::{AtomicBool, Ordering};

    use dbs_device::resources::Resource;
    use dbs_interrupt::NoopNotifier;
    use kvm_ioctls::Kvm;
    use test_utils::skip_if_kvm_unaccessable;
    use vm_memory::GuestMemoryMmap;
    use vmm_sys_util::eventfd::EventFd;
    use vmm_sys_util::tempfile::TempFile;

    use super::*;
    use crate::device::VirtioQueueConfig;
    use crate::tests::{create_address_space, VirtQueue, VIRTQ_DESC_F_NEXT, VIRTQ_DESC_F_WRITE};

    struct TestRegionHandler {
        reject: Arc<AtomicBool>,
        removed: Arc<AtomicBool>,
    }

    impl VirtioRegionHandler for TestRegionHandler {
        fn insert_regions(&mut self, _regions: Vec<Arc<GuestRegionMmap>>) -> Result<()> {
            if self.reject.load(Ordering::SeqCst) {
                Err(Error::InsertMmap)
            } else {
                Ok(())
            }
        }

        fn remove_regions(&mut self, _regions: &[Arc<GuestRegionMmap>]) -> Result<()> {
            self.removed.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    fn backing_file(size: u64) -> File {
        let file = TempFile::new().unwrap().into_file();
        file.set_len(size).unwrap();
        file
    }

    fn test_handler(
        reject: Arc<AtomicBool>,
        removed: Arc<AtomicBool>,
    ) -> Box<dyn VirtioRegionHandler> {
        Box::new(TestRegionHandler { reject, removed })
    }

    fn resources(len: u64) -> DeviceResources {
        let mut resources = DeviceResources::new();
        resources.append(Resource::MmioAddressRange {
            base: 0x1_0000_0000,
            size: len,
        });
        resources.append(Resource::KvmMemSlot(0));
        resources
    }

    fn device_config(vm_fd: Arc<VmFd>) -> VirtioDeviceConfig<Arc<GuestMemoryMmap>> {
        let memory = Arc::new(GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap());
        VirtioDeviceConfig::new(
            memory,
            create_address_space(),
            vm_fd,
            DeviceResources::new(),
            vec![VirtioQueueConfig::create(QUEUE_SIZE, 0).unwrap()],
            None,
            Arc::new(NoopNotifier::new()),
        )
    }

    fn queue_handler() -> PmemEpollHandler<Arc<GuestMemoryMmap>> {
        let vm_fd = Arc::new(Kvm::new().unwrap().create_vm().unwrap());
        let config = device_config(vm_fd);
        let region = Arc::new(
            GuestRegionMmap::new(MmapRegion::new(0x1000).unwrap(), GuestAddress(0x8000)).unwrap(),
        );
        PmemEpollHandler {
            id: "pmem-test".into(),
            config,
            region,
            _mapping: Arc::new(MmapRegion::new(0x1000).unwrap()),
            size: 0x1000,
        }
    }

    #[test]
    fn test_file_range_and_read_only_config() {
        let reject = Arc::new(AtomicBool::new(false));
        let removed = Arc::new(AtomicBool::new(false));
        let mut dev = Pmem::<Arc<GuestMemoryMmap>>::new(
            "pmem0".into(),
            backing_file(PMEM_ALIGN + 1),
            None,
            true,
            test_handler(reject, removed),
            EpollManager::default(),
            false,
        )
        .unwrap();
        assert_eq!(dev.len, PMEM_ALIGN * 2);
        let config = dev.device_info.config_space.clone();
        VirtioDevice::<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap>::write_config(
            &mut dev,
            0,
            &[0xff; 16],
        )
        .unwrap();
        assert_eq!(dev.device_info.config_space, config);

        let range = 0..1;
        let (_owner, mapping, flags) = Pmem::<Arc<GuestMemoryMmap>>::map_file(
            dev.file.as_ref().unwrap(),
            &range,
            PMEM_ALIGN,
            false,
        )
        .unwrap();
        assert_eq!(mapping.size(), PMEM_ALIGN as usize);
        assert_eq!(flags, 0);
        // SAFETY: The padded byte is inside the anonymous tail of the live reservation.
        unsafe {
            mapping.as_ptr().add(0x1000).write_volatile(0xa5);
            assert_eq!(mapping.as_ptr().add(0x1000).read_volatile(), 0xa5);
        }
        let (_read_only_owner, _, flags) = Pmem::<Arc<GuestMemoryMmap>>::map_file(
            dev.file.as_ref().unwrap(),
            &range,
            PMEM_ALIGN,
            true,
        )
        .unwrap();
        assert_eq!(flags, KVM_MEM_READONLY);

        let invalid_range = 1..2;
        assert!(Pmem::<Arc<GuestMemoryMmap>>::new(
            "invalid".into(),
            backing_file(PMEM_ALIGN),
            Some(invalid_range),
            false,
            test_handler(
                Arc::new(AtomicBool::new(false)),
                Arc::new(AtomicBool::new(false)),
            ),
            EpollManager::default(),
            false,
        )
        .is_err());
    }

    #[test]
    fn test_flush_and_malformed_queue() {
        skip_if_kvm_unaccessable!();
        for (request_len, used_len) in [(4, 4), (8, 0)] {
            let mut handler = queue_handler();
            let memory = handler.config.vm_as.clone();
            let vq = VirtQueue::new(GuestAddress(0), &memory, 16);
            vq.avail.ring(0).store(0);
            vq.avail.idx().store(1);
            vq.dtable(0).set(0x1000, request_len, VIRTQ_DESC_F_NEXT, 1);
            vq.dtable(1).set(0x2000, 4, VIRTQ_DESC_F_WRITE, 0);
            memory
                .write_obj(VIRTIO_PMEM_REQ_TYPE_FLUSH, GuestAddress(0x1000))
                .unwrap();
            handler.config.queues = vec![VirtioQueueConfig::new(
                vq.create_queue(),
                Arc::new(EventFd::new(0).unwrap()),
                Arc::new(NoopNotifier::new()),
                0,
            )];
            assert!(handler.process_queue(0));
            assert_eq!(vq.used.ring(0).load().len, used_len);
        }
    }

    #[test]
    fn test_resource_rollback_and_reset() {
        skip_if_kvm_unaccessable!();
        let epoll = EpollManager::default();
        let reject = Arc::new(AtomicBool::new(true));
        let removed = Arc::new(AtomicBool::new(false));
        let mut dev = Pmem::<Arc<GuestMemoryMmap>>::new(
            "pmem0".into(),
            backing_file(PMEM_ALIGN),
            None,
            false,
            test_handler(reject.clone(), removed.clone()),
            epoll,
            false,
        )
        .unwrap();
        let kvm = Kvm::new().unwrap();
        let vm_fd = Arc::new(kvm.create_vm().unwrap());
        let len = dev.len;
        assert!(
            VirtioDevice::<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap>::set_resource(
                &mut dev,
                vm_fd.clone(),
                resources(len),
            )
            .is_err()
        );
        reject.store(false, Ordering::SeqCst);
        VirtioDevice::<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap>::set_resource(
            &mut dev,
            vm_fd.clone(),
            resources(len),
        )
        .unwrap();
        VirtioDevice::<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap>::activate(
            &mut dev,
            device_config(vm_fd.clone()),
        )
        .unwrap();
        VirtioDevice::<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap>::reset(&mut dev).unwrap();
        VirtioDevice::<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap>::activate(
            &mut dev,
            device_config(vm_fd),
        )
        .unwrap();
        drop(dev);
        assert!(removed.load(Ordering::SeqCst));
    }

    #[test]
    fn test_hugetlbfs_page_size() {
        let name = CString::new("pmem-test").unwrap();
        // SAFETY: memfd_create receives a valid name.
        let fd = unsafe {
            libc::memfd_create(
                name.as_ptr(),
                libc::MFD_CLOEXEC | libc::MFD_HUGETLB | libc::MFD_HUGE_2MB,
            )
        };
        if fd < 0 {
            return;
        }
        // SAFETY: fd is uniquely owned after successful memfd_create.
        let file = unsafe { File::from_raw_fd(fd) };
        assert_eq!(
            Pmem::<Arc<GuestMemoryMmap>>::file_page_size(&file).unwrap(),
            PMEM_ALIGN
        );
    }
}
