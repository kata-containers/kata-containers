// Copyright 2019-2022 Alibaba Cloud. All rights reserved.
//
// Portions Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

//! Vritio Device Model.
//!
//! The Virtio specification defines a group of Virtio devices and transport layers.
//! The Virtio device model defines traits and structs for Virtio transport layers to
//! manage Virtio device backend drivers.

use std::any::Any;
use std::cmp;
use std::io::Write;
use std::ops::Deref;
use std::sync::Arc;

use dbs_address_space::AddressSpace;
use dbs_device::resources::{DeviceResources, ResourceConstraint};
use dbs_interrupt::{InterruptNotifier, NoopNotifier};
use dbs_utils::epoll_manager::{EpollManager, EpollSubscriber, SubscriberId};
use kvm_ioctls::VmFd;
use log::{error, warn};
use virtio_queue::{DescriptorChain, QueueOwnedT, QueueSync, QueueT};
use vm_memory::{
    Address, GuestAddress, GuestAddressSpace, GuestMemory, GuestMemoryRegion, GuestRegionMmap,
    GuestUsize,
};
use vmm_sys_util::eventfd::{EventFd, EFD_NONBLOCK};

use crate::{ActivateError, ActivateResult, ConfigError, ConfigResult, Error, Result};

/// Virtio queue configuration information.
///
/// The `VirtioQueueConfig` maintains configuration information for a Virtio queue.
/// It also provides methods to access the queue and associated interrupt/event notifiers.
pub struct VirtioQueueConfig<Q: QueueT = QueueSync> {
    /// Virtio queue object to access the associated queue.
    pub queue: Q,
    /// EventFd to receive queue notification from guest.
    pub eventfd: Arc<EventFd>,
    /// Notifier to inject interrupt to guest.
    notifier: Arc<dyn InterruptNotifier>,
    /// Queue index into the queue array.
    index: u16,
}

impl<Q: QueueT> VirtioQueueConfig<Q> {
    /// Create a `VirtioQueueConfig` object.
    pub fn new(
        queue: Q,
        eventfd: Arc<EventFd>,
        notifier: Arc<dyn InterruptNotifier>,
        index: u16,
    ) -> Self {
        VirtioQueueConfig {
            queue,
            eventfd,
            notifier,
            index,
        }
    }

    /// Create a `VirtioQueueConfig` object with the specified queue size and index.
    pub fn create(queue_size: u16, index: u16) -> Result<Self> {
        let eventfd = EventFd::new(EFD_NONBLOCK).map_err(Error::IOError)?;

        let queue = Q::new(queue_size)?;
        Ok(VirtioQueueConfig {
            queue,
            eventfd: Arc::new(eventfd),
            notifier: Arc::new(NoopNotifier::new()),
            index,
        })
    }

    /// Get queue index.
    #[inline]
    pub fn index(&self) -> u16 {
        self.index
    }

    /// Get immutable reference to the associated Virtio queue.
    pub fn queue(&self) -> &Q {
        &self.queue
    }

    /// Get mutable reference to the associated Virtio queue.
    pub fn queue_mut(&mut self) -> &mut Q {
        &mut self.queue
    }

    /// Get the maximum queue size.
    #[inline]
    pub fn max_size(&self) -> u16 {
        self.queue.max_size()
    }

    /// Get the next available descriptor.
    pub fn get_next_descriptor<M>(&mut self, mem: M) -> Result<Option<DescriptorChain<M>>>
    where
        M: Deref + Clone,
        M::Target: GuestMemory + Sized,
    {
        let mut guard = self.queue.lock();
        let mut iter = guard.iter(mem)?;
        Ok(iter.next())
    }

    /// Put a used descriptor into the used ring.
    #[inline]
    pub fn add_used<M: GuestMemory>(&mut self, mem: &M, desc_index: u16, len: u32) {
        self.queue
            .add_used(mem, desc_index, len)
            .unwrap_or_else(|_| panic!("Failed to add used. index: {}", desc_index))
    }

    /// Consume a queue notification event.
    #[inline]
    pub fn consume_event(&self) -> Result<u64> {
        self.eventfd.read().map_err(Error::IOError)
    }

    /// Produce a queue notification event.
    #[inline]
    pub fn generate_event(&self) -> Result<()> {
        self.eventfd.write(1).map_err(Error::IOError)
    }

    /// Inject an interrupt to the guest for queue change events.
    #[inline]
    pub fn notify(&self) -> Result<()> {
        self.notifier.notify().map_err(Error::IOError)
    }

    /// Set interrupt notifier to inject interrupts to the guest.
    #[inline]
    pub fn set_interrupt_notifier(&mut self, notifier: Arc<dyn InterruptNotifier>) {
        self.notifier = notifier;
    }

    /// Return the actual size of the queue, as the driver may not set up a
    /// queue as big as the device allows.
    #[inline]
    pub fn actual_size(&self) -> u16 {
        // TODO: rework once https://github.com/rust-vmm/vm-virtio/pull/153 get merged.
        //self.queue.size()
        std::cmp::min(self.queue.size(), self.queue.max_size())
    }
}

impl<Q: QueueT + Clone> Clone for VirtioQueueConfig<Q> {
    fn clone(&self) -> Self {
        VirtioQueueConfig {
            queue: self.queue.clone(),
            eventfd: self.eventfd.clone(),
            notifier: self.notifier.clone(),
            index: self.index,
        }
    }
}

/// Virtio device configuration information.
///
/// This structure maintains all configuration information for a Virtio device. It will be passed
/// to VirtioDevice::activate() and the Virtio device will take ownership of the configuration
/// object. On VirtioDevice::reset(), the configuration object should be returned to the caller.
pub struct VirtioDeviceConfig<
    AS: GuestAddressSpace,
    Q: QueueT = QueueSync,
    R: GuestMemoryRegion = GuestRegionMmap,
> {
    /// `GustMemoryAddress` object to access the guest memory.
    pub vm_as: AS,
    /// Guest address space
    pub address_space: AddressSpace,
    /// `VmFd` object for the device to access the hypervisor, such as KVM/HyperV etc.
    pub vm_fd: Arc<VmFd>,
    /// Resources assigned to the Virtio device.
    pub resources: DeviceResources,
    /// Virtio queues for normal data stream.
    pub queues: Vec<VirtioQueueConfig<Q>>,
    /// Virtio queue for device control requests.
    pub ctrl_queue: Option<VirtioQueueConfig<Q>>,
    /// Interrupt notifier to inject Virtio device change interrupt to the guest.
    pub device_change_notifier: Arc<dyn InterruptNotifier>,
    /// Shared memory region for Virtio-fs etc.
    pub shm_regions: Option<VirtioSharedMemoryList<R>>,
}

impl<AS, Q, R> VirtioDeviceConfig<AS, Q, R>
where
    AS: GuestAddressSpace,
    Q: QueueT,
    R: GuestMemoryRegion,
{
    /// Creates a new `VirtioDeviceConfig` object.
    pub fn new(
        vm_as: AS,
        address_space: AddressSpace,
        vm_fd: Arc<VmFd>,
        resources: DeviceResources,
        queues: Vec<VirtioQueueConfig<Q>>,
        ctrl_queue: Option<VirtioQueueConfig<Q>>,
        device_change_notifier: Arc<dyn InterruptNotifier>,
    ) -> Self {
        VirtioDeviceConfig {
            vm_as,
            address_space,
            vm_fd,
            resources,
            queues,
            ctrl_queue,
            device_change_notifier,
            shm_regions: None,
        }
    }

    /// Inject a Virtio device change notification to the guest.
    pub fn notify_device_changes(&self) -> Result<()> {
        self.device_change_notifier.notify().map_err(Error::IOError)
    }

    /// Get interrupt eventfds for normal Vritio queues.
    pub fn get_queue_interrupt_eventfds(&self) -> Vec<&EventFd> {
        self.queues
            .iter()
            .map(|x| x.notifier.notifier().unwrap())
            .collect()
    }

    /// Set shared memory region for Virtio-fs.
    pub fn set_shm_regions(&mut self, shm_regions: VirtioSharedMemoryList<R>) {
        self.shm_regions = Some(shm_regions);
    }

    /// Get host address and guest address of the shared memory region.
    pub fn get_shm_region_addr(&self) -> Option<(u64, u64)> {
        self.shm_regions
            .as_ref()
            .map(|shms| (shms.host_addr, shms.guest_addr.raw_value()))
    }

    /// Gets a shared reference to the guest memory object.
    pub fn lock_guest_memory(&self) -> AS::T {
        self.vm_as.memory()
    }
}

/// Device memory shared between guest and the device backend driver, defined by the Virtio
/// specification for Virtio-fs devices.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct VirtioSharedMemory {
    /// offset from the bar base
    pub offset: u64,
    /// len of this shared memory region
    pub len: u64,
}

/// A list of Shared Memory regions
#[derive(Debug)]
pub struct VirtioSharedMemoryList<R: GuestMemoryRegion> {
    /// Host address
    pub host_addr: u64,
    /// Guest address
    pub guest_addr: GuestAddress,
    /// Length
    pub len: GuestUsize,
    /// kvm_userspace_memory_region flags
    pub kvm_userspace_memory_region_flags: u32,
    /// kvm_userspace_memory_region slot
    pub kvm_userspace_memory_region_slot: u32,
    /// List of shared regions.
    pub region_list: Vec<VirtioSharedMemory>,

    /// List of mmap()ed regions managed through GuestRegionMmap instances. Using
    /// GuestRegionMmap will perform the unmapping automatically when the instance
    /// is dropped, which happens when the VirtioDevice gets dropped.
    ///
    /// GuestRegionMmap is used instead of MmapRegion. Because We need to insert
    /// this region into vm_as，but vm_as uses GuestRegionMmap to manage regions.
    /// If MmapRegion is used in here, the MmapRegion needs to be clone() to create
    /// new GuestRegionMmap for vm_as. MmapRegion clone() will cause the problem of
    /// duplicate unmap during automatic drop, so we should try to avoid the clone
    /// of MmapRegion. This problem does not exist with GuestRegionMmap because
    /// vm_as and VirtioSharedMemoryList can share GuestRegionMmap through Arc.
    pub mmap_region: Arc<R>,
}

impl<R: GuestMemoryRegion> Clone for VirtioSharedMemoryList<R> {
    fn clone(&self) -> Self {
        Self {
            host_addr: self.host_addr,
            guest_addr: self.guest_addr,
            len: self.len,
            kvm_userspace_memory_region_slot: self.kvm_userspace_memory_region_slot,
            kvm_userspace_memory_region_flags: self.kvm_userspace_memory_region_flags,
            region_list: self.region_list.clone(),
            mmap_region: self.mmap_region.clone(),
        }
    }
}

/// A callback for the VMM to insert memory region for virtio devices that
/// has device memory, such as DAX of virtiofs, pmem.
///
/// insert_region function is used to solve the problem that the virtio device cannot
/// find the host address corresponding to the guest address when reading the
/// guest device memory.
///
/// For example, the guest application executes the following code:
/// {
///     // "dax_fd" is virtio-fs file that support dax
///     // "no_dax_fd" is virtio-fs file that do not support dax
///     void *dax_ptr = (void*)mmap(NUMM, 4096, PORT, MAP_SHARED, dax_fd, 0);
///     write(no_dax_fd, dax_ptr, 4096);
/// }
/// dragonball will coredump.
///
/// This is because the virtiofs device cannot resolve the dax_ptr address
/// when calling vm_as.get_slice(). There is no DAX region in vm_as. This
/// trait inserts the virtio device memory region, such as DAX region, into
/// vm_as. This trait should be implemented in VMM when creating virtio
/// devices with device memory, because the virtio device does not have
/// permission to change vm_as.
pub trait VirtioRegionHandler: Send {
    /// Insert GuestRegionMmap to vm_as & address_space.
    fn insert_region(&mut self, region: Arc<GuestRegionMmap>) -> Result<()>;
}

/// Trait for Virtio transport layer to manage virtio devices.
///
/// The virtio transport driver takes the responsibility to manage lifecycle of virtio devices.
/// The device manager registers virtio devices to the transport driver, which will then manage
/// the device by:
/// - query device's resource requirement and allocate resources for it.
/// - handle guest register access by forwarding requests to the device.
/// - call activate()/reset() when the device is activated/reset by the guest.
/// The lifecycle of a virtio device is to be moved to a virtio transport, which will then query the
/// device. Once the guest driver has configured the device, `VirtioDevice::activate` will be called
/// and all the events, memory, and queues for device operation will be moved into the device.
/// Optionally, a virtio device can implement device reset in which it returns said resources and
/// resets its internal.
pub trait VirtioDevice<AS: GuestAddressSpace, Q: QueueT, R: GuestMemoryRegion>: Send {
    /// The virtio device type.
    fn device_type(&self) -> u32;

    /// The maximum size of each queue that this device supports.
    fn queue_max_sizes(&self) -> &[u16];

    /// The maxinum size of control queue
    fn ctrl_queue_max_sizes(&self) -> u16 {
        0
    }

    /// The set of feature bits shifted by `page * 32`.
    fn get_avail_features(&self, page: u32) -> u32 {
        let _ = page;
        0
    }

    /// Acknowledges that this set of features should be enabled.
    fn set_acked_features(&mut self, page: u32, value: u32);

    /// Reads this device configuration space at `offset`.
    fn read_config(&mut self, offset: u64, data: &mut [u8]) -> ConfigResult;

    /// Writes to this device configuration space at `offset`.
    fn write_config(&mut self, offset: u64, data: &[u8]) -> ConfigResult;

    /// Activates this device for real usage.
    fn activate(&mut self, config: VirtioDeviceConfig<AS, Q, R>) -> ActivateResult;

    /// Deactivates this device.
    fn reset(&mut self) -> ActivateResult {
        Err(ActivateError::InternalError)
    }

    /// Removes this devices.
    fn remove(&mut self) {}

    /// every new device object has its resource requirements
    fn get_resource_requirements(
        &self,
        requests: &mut Vec<ResourceConstraint>,
        use_generic_irq: bool,
    );

    /// Assigns requested resources back to virtio device
    fn set_resource(
        &mut self,
        _vm_fd: Arc<VmFd>,
        _resource: DeviceResources,
    ) -> Result<Option<VirtioSharedMemoryList<R>>> {
        Ok(None)
    }

    /// Used to downcast to the specific type.
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// A helper struct to support basic operations for emulated VirtioDevice backend devices.
pub struct VirtioDeviceInfo {
    /// Name of the virtio backend device.
    pub driver_name: String,
    /// Available features of the virtio backend device.
    pub avail_features: u64,
    /// Acknowledged features of the virtio backend device.
    pub acked_features: u64,
    /// Array of queue sizes.
    pub queue_sizes: Arc<Vec<u16>>,
    /// Space to store device specific configuration data.
    pub config_space: Vec<u8>,
    /// EventManager SubscriberOps to register/unregister epoll events.
    pub epoll_manager: EpollManager,
}

/// A helper struct to support basic operations for emulated VirtioDevice backend devices.
impl VirtioDeviceInfo {
    /// Creates a VirtioDeviceInfo instance.
    pub fn new(
        driver_name: String,
        avail_features: u64,
        queue_sizes: Arc<Vec<u16>>,
        config_space: Vec<u8>,
        epoll_manager: EpollManager,
    ) -> Self {
        VirtioDeviceInfo {
            driver_name,
            avail_features,
            acked_features: 0u64,
            queue_sizes,
            config_space,
            epoll_manager,
        }
    }

    /// Gets available features of virtio backend device.
    #[inline]
    pub fn avail_features(&self) -> u64 {
        self.avail_features
    }

    /// Gets available features of virtio backend device.
    pub fn get_avail_features(&self, page: u32) -> u32 {
        match page {
            // Get the lower 32-bits of the features bitfield.
            0 => self.avail_features as u32,
            // Get the upper 32-bits of the features bitfield.
            1 => (self.avail_features >> 32) as u32,
            _ => {
                warn!("{}: query features page: {}", self.driver_name, page);
                0u32
            }
        }
    }

    /// Gets acknowledged features of virtio backend device.
    #[inline]
    pub fn acked_features(&self) -> u64 {
        self.acked_features
    }

    /// Sets acknowledged features of virtio backend device.
    pub fn set_acked_features(&mut self, page: u32, value: u32) {
        let mut v = match page {
            0 => value as u64,
            1 => (value as u64) << 32,
            _ => {
                warn!("{}: ack unknown feature page: {}", self.driver_name, page);
                0u64
            }
        };

        // Check if the guest is ACK'ing a feature that we didn't claim to have.
        let unrequested_features = v & !self.avail_features;
        if unrequested_features != 0 {
            warn!("{}: ackknowlege unknown feature: {:x}", self.driver_name, v);
            // Don't count these features as acked.
            v &= !unrequested_features;
        }
        self.acked_features |= v;
    }

    /// Reads device specific configuration data of virtio backend device.
    ///
    /// The `offset` is based of 0x100 from the MMIO configuration address space.
    pub fn read_config(&self, offset: u64, mut data: &mut [u8]) -> ConfigResult {
        let config_len = self.config_space.len() as u64;
        if offset >= config_len {
            error!(
                "{}: config space read request out of range, offset {}",
                self.driver_name, offset
            );
            return Err(ConfigError::InvalidOffset(offset));
        }
        if let Some(end) = offset.checked_add(data.len() as u64) {
            // This write can't fail, offset and end are checked against config_len.
            data.write_all(&self.config_space[offset as usize..cmp::min(end, config_len) as usize])
                .unwrap();
        }
        Ok(())
    }

    /// Writes device specific configuration data of virtio backend device.
    ///
    /// The `offset` is based of 0x100 from the MMIO configuration address space.
    pub fn write_config(&mut self, offset: u64, data: &[u8]) -> ConfigResult {
        let data_len = data.len() as u64;
        let config_len = self.config_space.len() as u64;
        if offset >= config_len {
            error!(
                "{}: config space write request out of range, offset {}",
                self.driver_name, offset
            );
            return Err(ConfigError::InvalidOffset(offset));
        }
        if offset.checked_add(data_len).is_none() {
            error!(
                "{}: config space write request out of range, offset {}, data length {}",
                self.driver_name, offset, data_len
            );
            return Err(ConfigError::PlusOverflow(offset, data_len));
        }
        if offset + data_len > config_len {
            error!(
                "{}: config space write request out of range, offset {}, data length {}",
                self.driver_name, offset, data_len
            );
            return Err(ConfigError::InvalidOffsetPlusDataLen(offset + data_len));
        }

        let dst = &mut self.config_space[offset as usize..(offset + data_len) as usize];
        dst.copy_from_slice(data);
        Ok(())
    }

    /// Validate size of queues and queue eventfds.
    pub fn check_queue_sizes<Q: QueueT>(&self, queues: &[VirtioQueueConfig<Q>]) -> ActivateResult {
        if queues.is_empty() || queues.len() != self.queue_sizes.len() {
            error!(
                "{}: invalid configuration: maximum {} queue(s), got {} queues",
                self.driver_name,
                self.queue_sizes.len(),
                queues.len(),
            );
            return Err(ActivateError::InvalidParam);
        }
        Ok(())
    }

    /// Register event handler for the device.
    pub fn register_event_handler(&self, handler: EpollSubscriber) -> SubscriberId {
        self.epoll_manager.add_subscriber(handler)
    }

    /// Unregister event handler for the device.
    pub fn remove_event_handler(&mut self, id: SubscriberId) -> Result<EpollSubscriber> {
        self.epoll_manager.remove_subscriber(id).map_err(|e| {
            Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("remove_event_handler failed: {e:?}"),
            ))
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use dbs_interrupt::{
        InterruptManager, InterruptSourceType, InterruptStatusRegister32, LegacyNotifier,
    };
    use dbs_utils::epoll_manager::{EventOps, Events, MutEventSubscriber};
    use kvm_ioctls::Kvm;
    use virtio_queue::QueueSync;
    use vm_memory::{GuestMemoryAtomic, GuestMemoryMmap, GuestMemoryRegion, MmapRegion};

    use super::*;
    use crate::tests::{create_address_space, VirtQueue};
    use crate::{VIRTIO_INTR_CONFIG, VIRTIO_INTR_VRING};

    pub fn create_virtio_device_config() -> VirtioDeviceConfig<Arc<GuestMemoryMmap>> {
        let (vmfd, irq_manager) = crate::tests::create_vm_and_irq_manager();
        let group = irq_manager
            .create_group(InterruptSourceType::LegacyIrq, 0, 1)
            .unwrap();
        let status = Arc::new(InterruptStatusRegister32::new());
        let device_change_notifier = Arc::new(LegacyNotifier::new(
            group.clone(),
            status.clone(),
            VIRTIO_INTR_CONFIG,
        ));

        let mem = Arc::new(GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap());

        let mut queues = Vec::new();
        for idx in 0..8 {
            queues.push(VirtioQueueConfig::new(
                QueueSync::new(512).unwrap(),
                Arc::new(EventFd::new(0).unwrap()),
                Arc::new(LegacyNotifier::new(
                    group.clone(),
                    status.clone(),
                    VIRTIO_INTR_VRING,
                )),
                idx,
            ));
        }

        let address_space = create_address_space();

        VirtioDeviceConfig::new(
            mem,
            address_space,
            vmfd,
            DeviceResources::new(),
            queues,
            None,
            device_change_notifier,
        )
    }

    #[test]
    fn test_create_virtio_queue_config() {
        let (_vmfd, irq_manager) = crate::tests::create_vm_and_irq_manager();
        let group = irq_manager
            .create_group(InterruptSourceType::LegacyIrq, 0, 1)
            .unwrap();
        let status = Arc::new(InterruptStatusRegister32::new());
        let notifier = Arc::new(LegacyNotifier::new(group, status, VIRTIO_INTR_VRING));

        let mem =
            Arc::new(GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap());
        let vq = VirtQueue::new(GuestAddress(0), &mem, 1024);
        let q = vq.create_queue();
        let mut cfg = VirtioQueueConfig::new(
            q,
            Arc::new(EventFd::new(EFD_NONBLOCK).unwrap()),
            notifier,
            1,
        );

        let desc = cfg.get_next_descriptor(mem.memory()).unwrap();
        assert!(matches!(desc, None));

        cfg.notify().unwrap();
        assert_eq!(cfg.index(), 1);
        assert_eq!(cfg.max_size(), 1024);
        assert_eq!(cfg.actual_size(), 1024);
        cfg.generate_event().unwrap();
        assert_eq!(cfg.consume_event().unwrap(), 1);
    }

    #[test]
    fn test_clone_virtio_queue_config() {
        let (_vmfd, irq_manager) = crate::tests::create_vm_and_irq_manager();
        let group = irq_manager
            .create_group(InterruptSourceType::LegacyIrq, 0, 1)
            .unwrap();
        let status = Arc::new(InterruptStatusRegister32::new());
        let notifier = Arc::new(LegacyNotifier::new(group, status, VIRTIO_INTR_VRING));

        let mem =
            Arc::new(GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap());
        let vq = VirtQueue::new(GuestAddress(0), &mem, 1024);
        let q = vq.create_queue();
        let mut cfg = VirtioQueueConfig::new(
            q,
            Arc::new(EventFd::new(EFD_NONBLOCK).unwrap()),
            notifier,
            1,
        );

        let desc = cfg.get_next_descriptor(mem.memory()).unwrap();
        assert!(matches!(desc, None));

        {
            let mut guard = cfg.queue_mut().lock();
            let mut iter = guard.iter(mem.memory()).unwrap();
            assert!(matches!(iter.next(), None));
        }

        cfg.notify().unwrap();
        assert_eq!(cfg.index(), 1);
        assert_eq!(cfg.max_size(), 1024);
        assert_eq!(cfg.actual_size(), 1024);
        assert_eq!(cfg.queue.max_size(), 1024);
        cfg.generate_event().unwrap();
        assert_eq!(cfg.consume_event().unwrap(), 1);
    }

    #[test]
    fn test_create_virtio_device_config() {
        let mut device_config = create_virtio_device_config();

        device_config.notify_device_changes().unwrap();
        assert_eq!(device_config.get_queue_interrupt_eventfds().len(), 8);

        let shared_mem =
            GuestRegionMmap::new(MmapRegion::new(4096).unwrap(), GuestAddress(0)).unwrap();

        let list = VirtioSharedMemoryList {
            host_addr: 0x1234,
            guest_addr: GuestAddress(0x5678),
            len: shared_mem.len(),
            kvm_userspace_memory_region_flags: 0,
            kvm_userspace_memory_region_slot: 1,
            region_list: vec![VirtioSharedMemory {
                offset: 0,
                len: 4096,
            }],
            mmap_region: Arc::new(shared_mem),
        };

        device_config.set_shm_regions(list);
        let (host_addr, guest_addr) = device_config.get_shm_region_addr().unwrap();
        assert_eq!(host_addr, 0x1234);
        assert_eq!(guest_addr, 0x5678);
        let list = device_config.shm_regions.unwrap();
        assert_eq!(list.kvm_userspace_memory_region_slot, 1);
        assert_eq!(list.kvm_userspace_memory_region_flags, 0);
        assert_eq!(list.region_list.len(), 1);
    }

    struct DummyDevice {
        queue_size: Arc<Vec<u16>>,
        device_info: VirtioDeviceInfo,
    }

    impl VirtioDevice<GuestMemoryAtomic<GuestMemoryMmap>, QueueSync, GuestRegionMmap> for DummyDevice {
        fn device_type(&self) -> u32 {
            0xffff
        }
        fn queue_max_sizes(&self) -> &[u16] {
            &self.queue_size
        }

        fn get_avail_features(&self, page: u32) -> u32 {
            self.device_info.get_avail_features(page)
        }
        fn set_acked_features(&mut self, page: u32, value: u32) {
            self.device_info.set_acked_features(page, value)
        }

        fn read_config(&mut self, offset: u64, data: &mut [u8]) -> ConfigResult {
            self.device_info.read_config(offset, data)
        }
        fn write_config(&mut self, offset: u64, data: &[u8]) -> ConfigResult {
            self.device_info.write_config(offset, data)
        }
        fn activate(
            &mut self,
            _config: VirtioDeviceConfig<GuestMemoryAtomic<GuestMemoryMmap>>,
        ) -> ActivateResult {
            Ok(())
        }
        fn get_resource_requirements(
            &self,
            _requests: &mut Vec<ResourceConstraint>,
            _use_generic_irq: bool,
        ) {
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    struct DummyHandler;
    impl MutEventSubscriber for DummyHandler {
        fn process(&mut self, _events: Events, _ops: &mut EventOps) {}
        fn init(&mut self, _ops: &mut EventOps) {}
    }

    #[test]
    fn test_virtio_device() {
        let epoll_mgr = EpollManager::default();

        let avail_features = 0x1234 << 32 | 0x4567;
        let config_space = vec![1; 16];
        let queue_size = Arc::new(vec![256; 1]);
        let device_info = VirtioDeviceInfo::new(
            String::from("dummy-device"),
            avail_features,
            queue_size.clone(),
            config_space,
            epoll_mgr,
        );

        let mut device = DummyDevice {
            queue_size,
            device_info,
        };
        assert_eq!(device.device_type(), 0xffff);
        assert_eq!(device.queue_max_sizes(), &[256]);
        assert_eq!(device.ctrl_queue_max_sizes(), 0);

        device.get_resource_requirements(&mut Vec::new(), true);

        // tests avail features
        assert_eq!(device.get_avail_features(0), 0x4567);
        assert_eq!(
            device.get_avail_features(1),
            (device.device_info.avail_features() >> 32) as u32
        );
        assert_eq!(device.get_avail_features(2), 0);

        // tests acked features
        assert_eq!(device.device_info.acked_features(), 0);
        device.set_acked_features(2, 0x0004 | 0x0002);
        assert_eq!(device.device_info.acked_features(), 0);
        device.set_acked_features(1, 0x0004 | 0x0002);
        assert_eq!(device.device_info.acked_features(), 0x0004 << 32);
        device.set_acked_features(0, 0x4567 | 0x0008);
        assert_eq!(device.device_info.acked_features(), 0x4567 | 0x0004 << 32);

        // test config space invalid read
        let mut data = vec![0u8; 16];
        assert_eq!(
            device.read_config(16, data.as_mut_slice()).unwrap_err(),
            ConfigError::InvalidOffset(16)
        );
        assert_eq!(data, vec![0; 16]);
        // test read config
        device.read_config(4, &mut data[..14]).unwrap();
        assert_eq!(data, vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0]);
        device.read_config(0, data.as_mut_slice()).unwrap();
        assert_eq!(data, vec![1; 16]);

        // test config space invalid write
        let write_data = vec![0xffu8; 16];
        let mut read_data = vec![0x0; 16];
        assert_eq!(
            device.write_config(4, &write_data[..13]).unwrap_err(),
            ConfigError::InvalidOffsetPlusDataLen(17)
        );
        assert_eq!(
            device.write_config(16, &write_data[..4]).unwrap_err(),
            ConfigError::InvalidOffset(16)
        );
        device.read_config(0, read_data.as_mut_slice()).unwrap();
        assert_eq!(read_data, vec![0x1; 16]);

        // test config space write
        device.write_config(4, &write_data[6..10]).unwrap();
        assert_eq!(
            device.device_info.config_space,
            vec![1, 1, 1, 1, 0xff, 0xff, 0xff, 0xff, 1, 1, 1, 1, 1, 1, 1, 1]
        );

        // test device info check_queue_sizes
        let queue_size = Vec::new();
        assert!(matches!(
            device
                .device_info
                .check_queue_sizes::<QueueSync>(&queue_size),
            Err(ActivateError::InvalidParam)
        ));

        assert!(matches!(device.reset(), Err(ActivateError::InternalError)));

        // test event handler
        let handler = DummyHandler;
        let id = device.device_info.register_event_handler(Box::new(handler));
        device.device_info.remove_event_handler(id).unwrap();
        assert!(matches!(
            device.device_info.remove_event_handler(id),
            Err(Error::IOError(_))
        ));

        // test device activate
        let region_size = 0x400;
        let regions = vec![
            (GuestAddress(0x0), region_size),
            (GuestAddress(0x1000), region_size),
        ];
        let gmm = GuestMemoryMmap::from_ranges(&regions).unwrap();
        let gm = GuestMemoryAtomic::<GuestMemoryMmap>::new(gmm);

        let queues = vec![
            VirtioQueueConfig::create(2, 0).unwrap(),
            VirtioQueueConfig::create(2, 0).unwrap(),
        ];
        let kvm = Kvm::new().unwrap();
        let vm_fd = Arc::new(kvm.create_vm().unwrap());
        let resources = DeviceResources::new();
        let address_space = create_address_space();
        let device_config = VirtioDeviceConfig::new(
            gm,
            address_space,
            vm_fd,
            resources,
            queues,
            None,
            Arc::new(NoopNotifier::new()),
        );
        device.activate(device_config).unwrap();
    }
}
