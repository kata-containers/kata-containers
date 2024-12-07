// Copyright (C) 2019 Alibaba Cloud Computing. All rights reserved.
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause

///////////////////////////////////////////////////////////////
// TODO: we really need better support of device reset, error recovery, exceptions handling.
///////////////////////////////////////////////////////////////

use std::ops::Deref;
use std::sync::Arc;

use dbs_address_space::AddressSpace;
use dbs_device::resources::DeviceResources;
use dbs_interrupt::{DeviceInterruptManager, DeviceInterruptMode, InterruptIndex, KvmIrqManager};
use kvm_bindings::kvm_userspace_memory_region;
use kvm_ioctls::{IoEventAddress, NoDatamatch, VmFd};
use log::{debug, error, info, warn};
use virtio_queue::QueueT;
use vm_memory::{GuestAddressSpace, GuestMemoryRegion};

use crate::{
    mmio::*, warn_or_panic, ActivateError, Error, Result, VirtioDevice, VirtioDeviceConfig,
    VirtioQueueConfig, VirtioSharedMemory, VirtioSharedMemoryList, DEVICE_DRIVER_OK, DEVICE_FAILED,
};

/// The state of Virtio Mmio device.
pub struct MmioV2DeviceState<AS: GuestAddressSpace + Clone, Q: QueueT, R: GuestMemoryRegion> {
    device: Box<dyn VirtioDevice<AS, Q, R>>,
    vm_fd: Arc<VmFd>,
    vm_as: AS,
    address_space: AddressSpace,
    intr_mgr: DeviceInterruptManager<Arc<KvmIrqManager>>,
    device_resources: DeviceResources,
    queues: Vec<VirtioQueueConfig<Q>>,

    mmio_base: u64,
    has_ctrl_queue: bool,
    device_activated: bool,
    ioevent_registered: bool,

    features_select: u32,
    acked_features_select: u32,
    queue_select: u32,

    msi: Option<Msi>,
    doorbell: Option<DoorBell>,

    shm_region_id: u32,
    shm_regions: Option<VirtioSharedMemoryList<R>>,
}

impl<AS, Q, R> MmioV2DeviceState<AS, Q, R>
where
    AS: GuestAddressSpace + Clone,
    Q: QueueT + Clone,
    R: GuestMemoryRegion,
{
    /// Returns a reference to the internal device object.
    pub fn get_inner_device(&self) -> &dyn VirtioDevice<AS, Q, R> {
        self.device.as_ref()
    }

    /// Returns a mutable reference to the internal device object.
    pub fn get_inner_device_mut(&mut self) -> &mut dyn VirtioDevice<AS, Q, R> {
        self.device.as_mut()
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        mut device: Box<dyn VirtioDevice<AS, Q, R>>,
        vm_fd: Arc<VmFd>,
        vm_as: AS,
        address_space: AddressSpace,
        irq_manager: Arc<KvmIrqManager>,
        device_resources: DeviceResources,
        mmio_base: u64,
        doorbell_enabled: bool,
    ) -> Result<Self> {
        let intr_mgr =
            DeviceInterruptManager::new(irq_manager, &device_resources).map_err(Error::IOError)?;

        let (queues, has_ctrl_queue) = Self::create_queues(device.as_ref())?;

        // Assign requested device resources back to virtio device and let it do necessary setups,
        // as only virtio device knows how to use such resources. And if there's
        // VirtioSharedMemoryList returned, assigned it to MmioV2DeviceState
        let shm_regions = device
            .set_resource(vm_fd.clone(), device_resources.clone())
            .map_err(|e| {
                error!("Failed to assign device resource to virtio device: {}", e);
                e
            })?;

        let doorbell = if doorbell_enabled {
            Some(DoorBell::new(
                DRAGONBALL_MMIO_DOORBELL_OFFSET as u32,
                DRAGONBALL_MMIO_DOORBELL_SCALE as u32,
            ))
        } else {
            None
        };

        Ok(MmioV2DeviceState {
            device,
            vm_fd,
            vm_as,
            address_space,
            intr_mgr,
            device_resources,
            queues,
            mmio_base,
            has_ctrl_queue,
            ioevent_registered: false,
            device_activated: false,
            features_select: 0,
            acked_features_select: 0,
            queue_select: 0,
            doorbell,
            msi: None,
            shm_region_id: 0,
            shm_regions,
        })
    }

    pub(crate) fn activate(&mut self, device: &MmioV2Device<AS, Q, R>) -> Result<()> {
        if self.device_activated {
            return Ok(());
        }

        // If the driver incorrectly sets up the queues, the following check will fail and take
        // the device into an unusable state.
        if !self.check_queues_valid() {
            return Err(Error::ActivateError(Box::new(
                ActivateError::InvalidQueueConfig,
            )));
        }

        self.register_ioevent()?;

        self.intr_mgr.enable()?;

        let config = self.create_device_config(device)?;

        self.device
            .activate(config)
            .map(|_| self.device_activated = true)
            .map_err(|e| {
                error!("device activate error: {:?}", e);
                Error::ActivateError(Box::new(e))
            })
    }

    fn create_queues(
        device: &dyn VirtioDevice<AS, Q, R>,
    ) -> Result<(Vec<VirtioQueueConfig<Q>>, bool)> {
        let mut queues = Vec::new();
        for (idx, size) in device.queue_max_sizes().iter().enumerate() {
            queues.push(VirtioQueueConfig::create(*size, idx as u16)?);
        }

        // The ctrl queue must be append to Queue Vec, because the guest will
        // configure it which is same with other queues.
        let has_ctrl_queue = device.ctrl_queue_max_sizes() > 0;
        if has_ctrl_queue {
            queues.push(VirtioQueueConfig::create(
                device.ctrl_queue_max_sizes(),
                queues.len() as u16,
            )?);
        }

        Ok((queues, has_ctrl_queue))
    }

    fn create_queue_config(
        &mut self,
        device: &MmioV2Device<AS, Q, R>,
    ) -> Result<Vec<VirtioQueueConfig<Q>>> {
        // Safe because we have just called self.intr_mgr.enable().
        let group = self.intr_mgr.get_group().unwrap();
        let mut queues = Vec::new();
        for queue in self.queues.iter() {
            //The first interrupt index is device config change.
            let queue_notifier = crate::notifier::create_queue_notifier(
                group.clone(),
                device.interrupt_status(),
                queue.index() as InterruptIndex + 1,
            );
            queues.push(VirtioQueueConfig::new(
                queue.queue.clone(),
                queue.eventfd.clone(),
                queue_notifier,
                queue.index(),
            ));
        }
        Ok(queues)
    }

    fn create_device_config(
        &mut self,
        device: &MmioV2Device<AS, Q, R>,
    ) -> Result<VirtioDeviceConfig<AS, Q, R>> {
        let mut queues = self.create_queue_config(device)?;
        let ctrl_queue = if self.has_ctrl_queue {
            queues.pop()
        } else {
            None
        };

        // Safe because we have just called self.intr_mgr.enable().
        let group = self.intr_mgr.get_group().unwrap();
        //The first interrupt index is device config change.
        let notifier = crate::notifier::create_device_notifier(group, device.interrupt_status(), 0);

        let mut config = VirtioDeviceConfig::new(
            self.vm_as.clone(),
            self.address_space.clone(),
            self.vm_fd.clone(),
            self.device_resources.clone(),
            queues,
            ctrl_queue,
            notifier,
        );
        if let Some(shm_regions) = self.shm_regions.as_ref() {
            config.set_shm_regions((*shm_regions).clone());
        }
        Ok(config)
    }

    fn register_ioevent(&mut self) -> Result<()> {
        for (i, queue) in self.queues.iter().enumerate() {
            if let Some(doorbell) = self.doorbell.as_ref() {
                let io_addr = IoEventAddress::Mmio(self.mmio_base + doorbell.queue_offset(i));
                if let Err(e) = self
                    .vm_fd
                    .register_ioevent(&queue.eventfd, &io_addr, NoDatamatch)
                {
                    self.revert_ioevent(i, &io_addr, true);
                    return Err(Error::IOError(std::io::Error::from_raw_os_error(e.errno())));
                }
            }
            // always register ioeventfd in MMIO_NOTIFY_REG_OFFSET to avoid guest kernel which not support doorbell
            let io_addr = IoEventAddress::Mmio(self.mmio_base + MMIO_NOTIFY_REG_OFFSET as u64);
            if let Err(e) = self
                .vm_fd
                .register_ioevent(&queue.eventfd, &io_addr, i as u32)
            {
                self.unregister_ioevent_doorbell();
                self.revert_ioevent(i, &io_addr, false);
                return Err(Error::IOError(std::io::Error::from_raw_os_error(e.errno())));
            }
        }
        self.ioevent_registered = true;

        Ok(())
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn queues(&self) -> &Vec<VirtioQueueConfig<Q>> {
        &self.queues
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn queues_mut(&mut self) -> &mut Vec<VirtioQueueConfig<Q>> {
        &mut self.queues
    }

    #[inline]
    pub(crate) fn features_select(&self) -> u32 {
        self.features_select
    }

    #[inline]
    pub(crate) fn set_features_select(&mut self, v: u32) {
        self.features_select = v;
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn acked_features_select(&mut self) -> u32 {
        self.acked_features_select
    }

    #[inline]
    pub(crate) fn set_acked_features_select(&mut self, v: u32) {
        self.acked_features_select = v;
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn queue_select(&mut self) -> u32 {
        self.queue_select
    }

    #[inline]
    pub(crate) fn set_queue_select(&mut self, v: u32) {
        self.queue_select = v;
    }

    #[inline]
    pub(crate) fn set_acked_features(&mut self, v: u32) {
        self.device
            .set_acked_features(self.acked_features_select, v)
    }

    #[inline]
    pub(crate) fn set_shm_region_id(&mut self, v: u32) {
        self.shm_region_id = v;
    }

    #[inline]
    pub(crate) fn set_msi_address_low(&mut self, v: u32) {
        if let Some(m) = self.msi.as_mut() {
            m.set_address_low(v)
        }
    }

    #[inline]
    pub(crate) fn set_msi_address_high(&mut self, v: u32) {
        if let Some(m) = self.msi.as_mut() {
            m.set_address_high(v)
        }
    }

    #[inline]
    pub(crate) fn set_msi_data(&mut self, v: u32) {
        if let Some(m) = self.msi.as_mut() {
            m.set_data(v)
        }
    }

    #[inline]
    pub(crate) fn shm_regions(&self) -> Option<&VirtioSharedMemoryList<R>> {
        self.shm_regions.as_ref()
    }

    #[inline]
    pub(crate) fn device_activated(&self) -> bool {
        self.device_activated
    }

    #[inline]
    pub(crate) fn doorbell(&self) -> Option<&DoorBell> {
        self.doorbell.as_ref()
    }

    pub(crate) fn deactivate(&mut self) {
        if self.device_activated {
            self.device_activated = false;
        }
    }

    pub(crate) fn reset(&mut self) -> Result<()> {
        if self.device_activated {
            warn!("reset device while it's still in active state");
            Ok(())
        } else {
            // . Keep interrupt_evt and queue_evts as is. There may be pending
            //   notifications in those eventfds, but nothing will happen other
            //   than supurious wakeups.
            // . Do not reset config_generation and keep it monotonically increasing
            for queue in self.queues.iter_mut() {
                let new_queue = Q::new(queue.queue.max_size());
                if let Err(e) = new_queue {
                    warn!("reset device failed because new virtio-queue could not be created due to {:?}", e);
                    return Err(Error::VirtioQueueError(e));
                } else {
                    // unwrap is safe here since we have checked new_queue result above.
                    queue.queue = new_queue.unwrap();
                }
            }

            let _ = self.intr_mgr.reset();
            self.unregister_ioevent();
            self.features_select = 0;
            self.acked_features_select = 0;
            self.queue_select = 0;
            self.msi = None;
            self.doorbell = None;
            Ok(())
        }
    }

    fn unregister_ioevent(&mut self) {
        if self.ioevent_registered {
            let io_addr = IoEventAddress::Mmio(self.mmio_base + MMIO_NOTIFY_REG_OFFSET as u64);
            for (i, queue) in self.queues.iter().enumerate() {
                let _ = self
                    .vm_fd
                    .unregister_ioevent(&queue.eventfd, &io_addr, i as u32);
                self.ioevent_registered = false;
            }
        }
    }

    fn revert_ioevent(&mut self, num: usize, io_addr: &IoEventAddress, wildcard: bool) {
        assert!(num < self.queues.len());
        let mut idx = num;
        while idx > 0 {
            let datamatch = if wildcard {
                NoDatamatch.into()
            } else {
                idx as u64
            };
            idx -= 1;
            let _ = self
                .vm_fd
                .unregister_ioevent(&self.queues[idx].eventfd, io_addr, datamatch);
        }
    }

    fn unregister_ioevent_doorbell(&mut self) {
        if let Some(doorbell) = self.doorbell.as_ref() {
            for (i, queue) in self.queues.iter().enumerate() {
                let io_addr = IoEventAddress::Mmio(self.mmio_base + doorbell.queue_offset(i));
                let _ = self
                    .vm_fd
                    .unregister_ioevent(&queue.eventfd, &io_addr, NoDatamatch);
            }
        }
    }

    pub(crate) fn check_queues_valid(&self) -> bool {
        let mem = self.vm_as.memory();
        // All queues must have been enabled, we doesn't allow disabled queues.
        self.queues.iter().all(|c| c.queue.is_valid(mem.deref()))
    }

    pub(crate) fn with_queue<U, F>(&self, d: U, f: F) -> U
    where
        F: FnOnce(&Q) -> U,
    {
        match self.queues.get(self.queue_select as usize) {
            Some(config) => f(&config.queue),
            None => d,
        }
    }

    pub(crate) fn with_queue_mut<F: FnOnce(&mut Q)>(&mut self, f: F) -> bool {
        if let Some(config) = self.queues.get_mut(self.queue_select as usize) {
            f(&mut config.queue);
            true
        } else {
            false
        }
    }

    pub(crate) fn get_shm_field<U, F>(&mut self, d: U, f: F) -> U
    where
        F: FnOnce(&VirtioSharedMemory) -> U,
    {
        if let Some(regions) = self.shm_regions.as_ref() {
            match regions.region_list.get(self.shm_region_id as usize) {
                Some(region) => f(region),
                None => d,
            }
        } else {
            d
        }
    }

    pub(crate) fn update_msi_enable(&mut self, v: u16, device: &MmioV2Device<AS, Q, R>) {
        // Can't switch interrupt mode once the device has been activated.
        if device.driver_status() & DEVICE_DRIVER_OK != 0 {
            if device.driver_status() & DEVICE_FAILED == 0 {
                debug!("mmio_v2: can not switch interrupt mode for active device");
                device.set_driver_failed();
            }
            return;
        }

        if v & MMIO_MSI_CSR_ENABLED != 0 {
            // Guest enable msi interrupt
            if self.msi.is_none() {
                debug!("mmio_v2: switch to MSI interrupt mode");
                match self
                    .intr_mgr
                    .set_working_mode(DeviceInterruptMode::GenericMsiIrq)
                {
                    Ok(_) => self.msi = Some(Msi::default()),
                    Err(e) => {
                        warn!("mmio_v2: failed to switch to MSI interrupt mode: {:?}", e);
                        device.set_driver_failed();
                    }
                }
            }
        } else if self.msi.is_some() {
            // Guest disable msi interrupt
            match self
                .intr_mgr
                .set_working_mode(DeviceInterruptMode::LegacyIrq)
            {
                Ok(_) => self.msi = None,
                Err(e) => {
                    warn!(
                        "mmio_v2: failed to switch to legacy interrupt mode: {:?}",
                        e
                    );
                    device.set_driver_failed();
                }
            }
        }
    }

    fn update_msi_cfg(&mut self, v: u16) -> Result<()> {
        if let Some(msi) = self.msi.as_mut() {
            msi.index_select = v as u32;
            self.intr_mgr
                .set_msi_low_address(msi.index_select, msi.address_low)
                .map_err(Error::InterruptError)?;
            self.intr_mgr
                .set_msi_high_address(msi.index_select, msi.address_high)
                .map_err(Error::InterruptError)?;
            self.intr_mgr
                .set_msi_data(msi.index_select, msi.data)
                .map_err(Error::InterruptError)?;
            if self.intr_mgr.is_enabled() {
                self.intr_mgr
                    .update(msi.index_select)
                    .map_err(Error::InterruptError)?;
            }
        }

        Ok(())
    }

    fn mask_msi_int(&mut self, index: u32, mask: bool) -> Result<()> {
        if self.intr_mgr.is_enabled() {
            if let Some(group) = self.intr_mgr.get_group() {
                let old_mask = self
                    .intr_mgr
                    .get_msi_mask(index)
                    .map_err(Error::InterruptError)?;
                debug!("mmio_v2 old mask {}, mask {}", old_mask, mask);

                if !old_mask && mask {
                    group.mask(index)?;
                    self.intr_mgr
                        .set_msi_mask(index, true)
                        .map_err(Error::InterruptError)?;
                } else if old_mask && !mask {
                    group.unmask(index)?;
                    self.intr_mgr
                        .set_msi_mask(index, false)
                        .map_err(Error::InterruptError)?;
                }
            }
        }

        Ok(())
    }

    pub(crate) fn handle_msi_cmd(&mut self, v: u16, device: &MmioV2Device<AS, Q, R>) {
        let arg = v & MMIO_MSI_CMD_ARG_MASK;
        match v & MMIO_MSI_CMD_CODE_MASK {
            MMIO_MSI_CMD_CODE_UPDATE => {
                if arg > self.device.queue_max_sizes().len() as u16 {
                    info!("mmio_v2: configure interrupt for invalid vector {}", v,);
                } else if let Err(e) = self.update_msi_cfg(arg) {
                    warn_or_panic!("mmio_v2: failed to configure vector {}, {:?}", v, e);
                }
            }
            MMIO_MSI_CMD_CODE_INT_MASK => {
                if let Err(e) = self.mask_msi_int(arg as u32, true) {
                    warn_or_panic!("mmio_v2: failed to mask {}, {:?}", v, e);
                }
            }
            MMIO_MSI_CMD_CODE_INT_UNMASK => {
                if let Err(e) = self.mask_msi_int(arg as u32, false) {
                    warn_or_panic!("mmio_v2: failed to unmask {}, {:?}", v, e);
                }
            }
            _ => {
                warn!("mmio_v2: unknown msi command: 0x{:x}", v);
                device.set_driver_failed();
            }
        }
    }
}

impl<AS, Q, R> Drop for MmioV2DeviceState<AS, Q, R>
where
    AS: GuestAddressSpace + Clone,
    Q: QueueT,
    R: GuestMemoryRegion,
{
    fn drop(&mut self) {
        if let Some(memlist) = &self.shm_regions {
            let mmio_res = self.device_resources.get_mmio_address_ranges();
            let slots_res = self.device_resources.get_kvm_mem_slots();
            let shm_regions_num = mmio_res.len();
            let slots_num = slots_res.len();
            assert_eq!((shm_regions_num, slots_num), (1, 1));
            let kvm_mem_region = kvm_userspace_memory_region {
                slot: slots_res[0],
                flags: 0,
                guest_phys_addr: memlist.guest_addr.0,
                memory_size: 0,
                userspace_addr: memlist.host_addr,
            };
            unsafe {
                self.vm_fd.set_user_memory_region(kvm_mem_region).unwrap();
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use kvm_ioctls::Kvm;
    use virtio_queue::QueueSync;
    use vm_memory::{GuestAddress, GuestMemoryMmap, GuestRegionMmap};

    use super::*;
    use crate::mmio::mmio_v2::tests::*;
    use crate::tests::create_address_space;

    pub fn get_mmio_state(
        have_msi: bool,
        doorbell: bool,
        ctrl_queue_size: u16,
    ) -> MmioV2DeviceState<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap> {
        let mem = Arc::new(GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x1000)]).unwrap());

        let mmio_base = 0;
        let device_resources = get_device_resource(have_msi, false);

        let kvm = Kvm::new().unwrap();
        let vm_fd = Arc::new(kvm.create_vm().unwrap());
        vm_fd.create_irq_chip().unwrap();

        let irq_manager = Arc::new(KvmIrqManager::new(vm_fd.clone()));
        irq_manager.initialize().unwrap();

        let device = MmioDevice::new(ctrl_queue_size);

        let address_space = create_address_space();

        MmioV2DeviceState::new(
            Box::new(device),
            vm_fd,
            mem,
            address_space,
            irq_manager,
            device_resources,
            mmio_base,
            doorbell,
        )
        .unwrap()
    }

    #[test]
    fn test_virtio_mmio_state_new() {
        let mut state = get_mmio_state(false, false, 1);

        assert_eq!(state.queues.len(), 3);
        assert!(!state.check_queues_valid());

        state.queue_select = 0;
        assert_eq!(state.with_queue(0, |q| q.max_size()), 16);
        assert!(state.with_queue_mut(|q| q.set_size(16)));
        assert_eq!(state.queues[state.queue_select as usize].queue.size(), 16);

        state.queue_select = 1;
        assert_eq!(state.with_queue(0, |q| q.max_size()), 32);
        assert!(state.with_queue_mut(|q| q.set_size(8)));
        assert_eq!(state.queues[state.queue_select as usize].queue.size(), 8);

        state.queue_select = 3;
        assert_eq!(state.with_queue(0xff, |q| q.max_size()), 0xff);
        assert!(!state.with_queue_mut(|q| q.set_size(16)));

        assert!(!state.check_queues_valid());

        drop(state);
    }
}
