// Copyright (C) 2019 Alibaba Cloud Computing. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause

use std::any::Any;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use byteorder::{ByteOrder, LittleEndian};
use dbs_address_space::AddressSpace;
use dbs_device::resources::{DeviceResources, Resource};
use dbs_device::{DeviceIo, IoAddress};
use dbs_interrupt::{InterruptStatusRegister32, KvmIrqManager};
use kvm_ioctls::VmFd;
use log::{debug, info, warn};
use virtio_queue::QueueT;
use vm_memory::{GuestAddressSpace, GuestMemoryRegion};

use crate::{
    mmio::*, Error, Result, VirtioDevice, DEVICE_ACKNOWLEDGE, DEVICE_DRIVER, DEVICE_DRIVER_OK,
    DEVICE_FAILED, DEVICE_FEATURES_OK, DEVICE_INIT, VIRTIO_INTR_VRING,
};

const DEVICE_STATUS_INIT: u32 = DEVICE_INIT;
const DEVICE_STATUS_ACKNOWLEDE: u32 = DEVICE_STATUS_INIT | DEVICE_ACKNOWLEDGE;
const DEVICE_STATUS_DRIVER: u32 = DEVICE_STATUS_ACKNOWLEDE | DEVICE_DRIVER;
const DEVICE_STATUS_FEATURE_OK: u32 = DEVICE_STATUS_DRIVER | DEVICE_FEATURES_OK;
const DEVICE_STATUS_DRIVER_OK: u32 = DEVICE_STATUS_FEATURE_OK | DEVICE_DRIVER_OK;

/// Implements the
/// [MMIO](http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-1090002)
/// transport for virtio devices.
///
/// This requires 3 points of installation to work with a VM:
///
/// 1. Mmio reads and writes must be sent to this device at what is referred to here as MMIO base.
/// 1. `Mmio::queue_evts` must be installed at `MMIO_NOTIFY_REG_OFFSET` offset from the MMIO
/// base. Each event in the array must be signaled if the index is written at that offset.
/// 1. `Mmio::interrupt_evt` must signal an interrupt that the guest driver is listening to when it
/// is written to.
///
/// Typically one page (4096 bytes) of MMIO address space is sufficient to handle this transport
/// and inner virtio device.
pub struct MmioV2Device<AS: GuestAddressSpace + Clone, Q: QueueT, R: GuestMemoryRegion> {
    state: Mutex<MmioV2DeviceState<AS, Q, R>>,
    assigned_resources: DeviceResources,
    mmio_cfg_res: Resource,
    device_vendor: u32,
    driver_status: AtomicU32,
    config_generation: AtomicU32,
    interrupt_status: Arc<InterruptStatusRegister32>,
}

impl<AS, Q, R> MmioV2Device<AS, Q, R>
where
    AS: GuestAddressSpace + Clone,
    Q: QueueT + Clone,
    R: GuestMemoryRegion,
{
    /// Constructs a new MMIO transport for the given virtio device.
    pub fn new(
        vm_fd: Arc<VmFd>,
        vm_as: AS,
        address_space: AddressSpace,
        irq_manager: Arc<KvmIrqManager>,
        device: Box<dyn VirtioDevice<AS, Q, R>>,
        resources: DeviceResources,
        mut features: Option<u32>,
    ) -> Result<Self> {
        let mut device_resources = DeviceResources::new();
        let mut mmio_cfg_resource = None;
        let mut mmio_base = 0;
        let mut doorbell_enabled = false;

        for res in resources.iter() {
            if let Resource::MmioAddressRange { base, size } = res {
                if mmio_cfg_resource.is_none()
                    && *size == MMIO_DEFAULT_CFG_SIZE + DRAGONBALL_MMIO_DOORBELL_SIZE
                {
                    mmio_base = *base;
                    mmio_cfg_resource = Some(res.clone());
                    continue;
                }
            }
            device_resources.append(res.clone());
        }
        let mmio_cfg_res = match mmio_cfg_resource {
            Some(v) => v,
            None => return Err(Error::InvalidInput),
        };

        let msi_feature = if resources.get_generic_msi_irqs().is_some() {
            DRAGONBALL_FEATURE_MSI_INTR
        } else {
            0
        };

        if let Some(ref mut ft) = features {
            if (*ft & DRAGONBALL_FEATURE_PER_QUEUE_NOTIFY != 0)
                && vm_fd.check_extension(kvm_ioctls::Cap::IoeventfdNoLength)
            {
                doorbell_enabled = true;
            } else {
                *ft &= !DRAGONBALL_FEATURE_PER_QUEUE_NOTIFY;
            }
        }

        debug!("mmiov2: fast-mmio enabled: {}", doorbell_enabled);

        let state = MmioV2DeviceState::new(
            device,
            vm_fd,
            vm_as,
            address_space,
            irq_manager,
            device_resources,
            mmio_base,
            doorbell_enabled,
        )?;

        let mut device_vendor = MMIO_VENDOR_ID_DRAGONBALL | msi_feature;
        if let Some(ft) = features {
            debug!("mmiov2: feature bit is 0x{:0X}", ft);
            device_vendor |= ft & DRAGONBALL_FEATURE_MASK;
        }

        Ok(MmioV2Device {
            state: Mutex::new(state),
            assigned_resources: resources,
            mmio_cfg_res,
            device_vendor,
            driver_status: AtomicU32::new(DEVICE_INIT),
            config_generation: AtomicU32::new(0),
            interrupt_status: Arc::new(InterruptStatusRegister32::new()),
        })
    }

    /// Acquires the state while holding the lock.
    pub fn state(&self) -> MutexGuard<MmioV2DeviceState<AS, Q, R>> {
        // Safe to unwrap() because we don't expect poisoned lock here.
        self.state.lock().unwrap()
    }

    /// Removes device.
    pub fn remove(&self) {
        self.state().get_inner_device_mut().remove();
    }

    /// Returns the Resource.
    pub fn get_mmio_cfg_res(&self) -> Resource {
        self.mmio_cfg_res.clone()
    }

    /// Returns the type of device.
    pub fn get_device_type(&self) -> u32 {
        self.state().get_inner_device().device_type()
    }

    pub(crate) fn interrupt_status(&self) -> Arc<InterruptStatusRegister32> {
        self.interrupt_status.clone()
    }

    #[inline]
    /// Atomic sets the drive state to fail.
    pub(crate) fn set_driver_failed(&self) {
        self.driver_status.fetch_or(DEVICE_FAILED, Ordering::SeqCst);
    }

    #[inline]
    pub(crate) fn driver_status(&self) -> u32 {
        self.driver_status.load(Ordering::SeqCst)
    }

    #[inline]
    fn check_driver_status(&self, set: u32, clr: u32) -> bool {
        self.driver_status() & (set | clr) == set
    }

    #[inline]
    fn exchange_driver_status(&self, old: u32, new: u32) -> std::result::Result<u32, u32> {
        self.driver_status
            .compare_exchange(old, new, Ordering::SeqCst, Ordering::SeqCst)
    }

    /// Update driver status according to the state machine defined by VirtIO Spec 1.0.
    /// Please refer to VirtIO Spec 1.0, section 2.1.1 and 3.1.1.
    ///
    /// The driver MUST update device status, setting bits to indicate the completed steps
    /// of the driver initialization sequence specified in 3.1. The driver MUST NOT clear
    /// a device status bit. If the driver sets the FAILED bit, the driver MUST later reset
    /// the device before attempting to re-initialize.
    fn update_driver_status(&self, v: u32) {
        // Serialize to update device state.
        let mut state = self.state();
        let mut result = Err(DEVICE_FAILED);
        if v == DEVICE_STATUS_ACKNOWLEDE {
            result = self.exchange_driver_status(DEVICE_STATUS_INIT, DEVICE_STATUS_ACKNOWLEDE);
        } else if v == DEVICE_STATUS_DRIVER {
            result = self.exchange_driver_status(DEVICE_STATUS_ACKNOWLEDE, DEVICE_STATUS_DRIVER);
        } else if v == DEVICE_STATUS_FEATURE_OK {
            result = self.exchange_driver_status(DEVICE_STATUS_DRIVER, DEVICE_STATUS_FEATURE_OK);
        } else if v == DEVICE_STATUS_DRIVER_OK {
            result = self.exchange_driver_status(DEVICE_STATUS_FEATURE_OK, DEVICE_STATUS_DRIVER_OK);
            if result.is_ok() {
                if let Err(e) = state.activate(self) {
                    // Reset internal status to initial state on failure.
                    // Error is ignored since the device will go to DEVICE_FAILED status.
                    let _ = state.reset();
                    warn!("failed to activate MMIO Virtio device: {:?}", e);
                    result = Err(DEVICE_FAILED);
                }
            }
        } else if v == 0 {
            if self.driver_status() == DEVICE_INIT {
                result = Ok(0);
            } else if state.device_activated() {
                let ret = state.get_inner_device_mut().reset();
                if ret.is_err() {
                    warn!("failed to reset MMIO Virtio device: {:?}.", ret);
                } else {
                    state.deactivate();
                    // it should reset the device's status to init, otherwise, the guest would
                    // get the wrong device's status.
                    if let Err(e) = state.reset() {
                        warn!("failed to reset device state due to {:?}", e);
                        result = Err(DEVICE_FAILED);
                    } else {
                        result = self
                            .exchange_driver_status(DEVICE_STATUS_DRIVER_OK, DEVICE_STATUS_INIT);
                    }
                }
            }
        } else if v == self.driver_status() {
            // No real state change, nothing to do.
            result = Ok(0);
        } else if v & DEVICE_FAILED != 0 {
            // Guest driver marks device as failed.
            self.set_driver_failed();
            result = Ok(0);
        }

        if result.is_err() {
            warn!(
                "invalid virtio driver status transition: 0x{:x} -> 0x{:x}",
                self.driver_status(),
                v
            );
            // TODO: notify backend driver to stop the device
            self.set_driver_failed();
        }
    }

    fn update_queue_field<F: FnOnce(&mut Q)>(&self, f: F) {
        // Use mutex for state to protect device.write_config()
        let mut state = self.state();
        if self.check_driver_status(DEVICE_FEATURES_OK, DEVICE_DRIVER_OK | DEVICE_FAILED) {
            state.with_queue_mut(f);
        } else {
            info!(
                "update virtio queue in invalid state 0x{:x}",
                self.driver_status()
            );
        }
    }

    fn tweak_intr_flags(&self, flags: u32) -> u32 {
        // The MMIO virtio transport layer only supports legacy IRQs. And the typical way to
        // inject interrupt into the guest is:
        // 1) the vhost-user-net slave sends notifcaticaiton to dragonball by writing to eventfd.
        // 2) dragonball consumes the notification by read the eventfd.
        // 3) dragonball updates interrupt status register.
        // 4) dragonball injects interrupt to the guest by writing to an irqfd.
        //
        // We play a trick here to always report "descriptor ready in the used virtque".
        // This trick doesn't break the virtio spec because it allow virtio devices to inject
        // supurous interrupts. By applying this trick, the way to inject interrupts gets
        // simplified as:
        // 1) the vhost-user-net slave sends interrupt to the guest by writing to the irqfd.
        if self.device_vendor & DRAGONBALL_FEATURE_INTR_USED != 0 {
            flags | VIRTIO_INTR_VRING
        } else {
            flags
        }
    }

    fn device_features(&self) -> u32 {
        let state = self.state();
        let features_select = state.features_select();
        let mut features = state.get_inner_device().get_avail_features(features_select);
        if features_select == 1 {
            features |= 0x1; // enable support of VirtIO Version 1
        }
        features
    }

    fn set_acked_features(&self, v: u32) {
        // Use mutex for state to protect device.ack_features()
        let mut state = self.state();
        if self.check_driver_status(DEVICE_DRIVER, DEVICE_FEATURES_OK | DEVICE_FAILED) {
            state.set_acked_features(v);
        } else {
            info!(
                "ack virtio features in invalid state 0x{:x}",
                self.driver_status()
            );
        }
    }

    fn get_device_config(&self, offset: u64, data: &mut [u8]) {
        // Use mutex for state to protect device.write_config()
        let mut state = self.state();
        if self.check_driver_status(DEVICE_DRIVER, DEVICE_FAILED) {
            if let Err(e) = state.get_inner_device_mut().read_config(offset, data) {
                warn!("device read config err: {}", e);
            }
        } else {
            info!("can not read from device config data area before driver is ready");
        }
    }

    fn set_device_config(&self, offset: u64, data: &[u8]) {
        // Use mutex for state to protect device.write_config()
        let mut state = self.state();
        if self.check_driver_status(DEVICE_DRIVER, DEVICE_FAILED) {
            if let Err(e) = state.get_inner_device_mut().write_config(offset, data) {
                warn!("device write config err: {}", e);
            }
        } else {
            info!("can not write to device config data area before driver is ready");
        }
    }

    fn get_shm_base_low(&self) -> u32 {
        let mut state = self.state();
        let guest_addr: u64 = match state.shm_regions() {
            Some(regions) => regions.guest_addr.0,
            None => 0,
        };
        state.get_shm_field(0xffff_ffff, |s| (s.offset + guest_addr) as u32)
    }

    fn get_shm_base_high(&self) -> u32 {
        let mut state = self.state();
        let guest_addr: u64 = match state.shm_regions() {
            Some(regions) => regions.guest_addr.0,
            None => 0,
        };
        state.get_shm_field(0xffff_ffff, |s| ((s.offset + guest_addr) >> 32) as u32)
    }
}

impl<AS, Q, R> DeviceIo for MmioV2Device<AS, Q, R>
where
    AS: 'static + GuestAddressSpace + Send + Sync + Clone,
    Q: 'static + QueueT + Send + Clone,
    R: 'static + GuestMemoryRegion + Send + Sync,
{
    fn read(&self, _base: IoAddress, offset: IoAddress, data: &mut [u8]) {
        let offset = offset.raw_value();

        if offset >= MMIO_CFG_SPACE_OFF {
            self.get_device_config(offset - MMIO_CFG_SPACE_OFF, data);
        } else if data.len() == 4 {
            let v = match offset {
                REG_MMIO_MAGIC_VALUE => MMIO_MAGIC_VALUE,
                REG_MMIO_VERSION => MMIO_VERSION_2,
                REG_MMIO_DEVICE_ID => self.state().get_inner_device().device_type(),
                REG_MMIO_VENDOR_ID => self.device_vendor,
                REG_MMIO_DEVICE_FEATURE => self.device_features(),
                REG_MMIO_QUEUE_NUM_MA => self.state().with_queue(0, |q| q.max_size() as u32),
                REG_MMIO_QUEUE_READY => self.state().with_queue(0, |q| q.ready() as u32),
                REG_MMIO_QUEUE_NOTIF if self.state().doorbell().is_some() => {
                    // Safe to unwrap() because we have determined the option is a Some value.
                    self.state()
                        .doorbell()
                        .map(|doorbell| doorbell.register_data())
                        .unwrap()
                }
                REG_MMIO_INTERRUPT_STAT => self.tweak_intr_flags(self.interrupt_status.read()),
                REG_MMIO_STATUS => self.driver_status(),
                REG_MMIO_SHM_LEN_LOW => self.state().get_shm_field(0xffff_ffff, |s| s.len as u32),
                REG_MMIO_SHM_LEN_HIGH => self
                    .state()
                    .get_shm_field(0xffff_ffff, |s| (s.len >> 32) as u32),
                REG_MMIO_SHM_BASE_LOW => self.get_shm_base_low(),
                REG_MMIO_SHM_BASE_HIGH => self.get_shm_base_high(),
                REG_MMIO_CONFIG_GENERATI => self.config_generation.load(Ordering::SeqCst),
                _ => {
                    info!("unknown virtio mmio readl at 0x{:x}", offset);
                    return;
                }
            };
            LittleEndian::write_u32(data, v);
        } else if data.len() == 2 {
            let v = match offset {
                REG_MMIO_MSI_CSR => {
                    if (self.device_vendor & DRAGONBALL_FEATURE_MSI_INTR) != 0 {
                        MMIO_MSI_CSR_SUPPORTED
                    } else {
                        0
                    }
                }
                _ => {
                    info!("unknown virtio mmio readw from 0x{:x}", offset);
                    return;
                }
            };
            LittleEndian::write_u16(data, v);
        } else {
            info!(
                "unknown virtio mmio register read: 0x{:x}/0x{:x}",
                offset,
                data.len()
            );
        }
    }

    fn write(&self, _base: IoAddress, offset: IoAddress, data: &[u8]) {
        let offset = offset.raw_value();
        // Write to the device configuration area.
        if (MMIO_CFG_SPACE_OFF..DRAGONBALL_MMIO_DOORBELL_OFFSET).contains(&offset) {
            self.set_device_config(offset - MMIO_CFG_SPACE_OFF, data);
        } else if data.len() == 4 {
            let v = LittleEndian::read_u32(data);
            match offset {
                REG_MMIO_DEVICE_FEATURES_S => self.state().set_features_select(v),
                REG_MMIO_DRIVER_FEATURE => self.set_acked_features(v),
                REG_MMIO_DRIVER_FEATURES_S => self.state().set_acked_features_select(v),
                REG_MMIO_QUEUE_SEL => self.state().set_queue_select(v),
                REG_MMIO_QUEUE_NUM => self.update_queue_field(|q| q.set_size(v as u16)),
                REG_MMIO_QUEUE_READY => self.update_queue_field(|q| q.set_ready(v == 1)),
                REG_MMIO_INTERRUPT_AC => self.interrupt_status.clear_bits(v),
                REG_MMIO_STATUS => self.update_driver_status(v),
                REG_MMIO_QUEUE_DESC_LOW => {
                    self.update_queue_field(|q| q.set_desc_table_address(Some(v), None))
                }
                REG_MMIO_QUEUE_DESC_HIGH => {
                    self.update_queue_field(|q| q.set_desc_table_address(None, Some(v)))
                }
                REG_MMIO_QUEUE_AVAIL_LOW => {
                    self.update_queue_field(|q| q.set_avail_ring_address(Some(v), None))
                }
                REG_MMIO_QUEUE_AVAIL_HIGH => {
                    self.update_queue_field(|q| q.set_avail_ring_address(None, Some(v)))
                }
                REG_MMIO_QUEUE_USED_LOW => {
                    self.update_queue_field(|q| q.set_used_ring_address(Some(v), None))
                }
                REG_MMIO_QUEUE_USED_HIGH => {
                    self.update_queue_field(|q| q.set_used_ring_address(None, Some(v)))
                }
                REG_MMIO_SHM_SEL => self.state().set_shm_region_id(v),
                REG_MMIO_MSI_ADDRESS_L => self.state().set_msi_address_low(v),
                REG_MMIO_MSI_ADDRESS_H => self.state().set_msi_address_high(v),
                REG_MMIO_MSI_DATA => self.state().set_msi_data(v),
                _ => info!("unknown virtio mmio writel to 0x{:x}", offset),
            }
        } else if data.len() == 2 {
            let v = LittleEndian::read_u16(data);
            match offset {
                REG_MMIO_MSI_CSR => self.state().update_msi_enable(v, self),
                REG_MMIO_MSI_COMMAND => self.state().handle_msi_cmd(v, self),
                _ => {
                    info!("unknown virtio mmio writew to 0x{:x}", offset);
                }
            }
        } else {
            info!(
                "unknown virtio mmio register write: 0x{:x}/0x{:x}",
                offset,
                data.len()
            );
        }
    }

    fn get_assigned_resources(&self) -> DeviceResources {
        self.assigned_resources.clone()
    }

    fn get_trapped_io_resources(&self) -> DeviceResources {
        let mut resources = DeviceResources::new();

        resources.append(self.mmio_cfg_res.clone());

        resources
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::any::Any;
    use std::sync::Mutex;

    use byteorder::{ByteOrder, LittleEndian};
    use dbs_device::resources::{MsiIrqType, Resource, ResourceConstraint};
    use dbs_device::{DeviceIo, IoAddress};
    use dbs_utils::epoll_manager::EpollManager;
    use kvm_bindings::kvm_userspace_memory_region;
    use kvm_ioctls::Kvm;
    use virtio_queue::QueueSync;
    use vm_memory::{
        GuestAddress, GuestMemoryMmap, GuestMemoryRegion, GuestRegionMmap, MemoryRegionAddress,
        MmapRegion,
    };

    use super::*;
    use crate::tests::create_address_space;
    use crate::{
        ActivateResult, ConfigResult, Error, VirtioDeviceConfig, VirtioDeviceInfo,
        VirtioSharedMemory, VirtioSharedMemoryList, DEVICE_ACKNOWLEDGE, DEVICE_DRIVER,
        DEVICE_FEATURES_OK,
    };

    pub struct MmioDevice {
        state: Mutex<VirtioDeviceInfo>,
        config: Mutex<Option<VirtioDeviceConfig<Arc<GuestMemoryMmap>>>>,
        ctrl_queue_size: u16,
    }

    impl MmioDevice {
        pub fn new(ctrl_queue_size: u16) -> Self {
            let epoll_mgr = EpollManager::default();
            let state = VirtioDeviceInfo::new(
                "dummy".to_string(),
                0xf,
                Arc::new(vec![16u16, 32u16]),
                vec![0xffu8; 256],
                epoll_mgr,
            );
            MmioDevice {
                state: Mutex::new(state),
                config: Mutex::new(None),
                ctrl_queue_size,
            }
        }
    }

    impl VirtioDevice<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap> for MmioDevice {
        fn device_type(&self) -> u32 {
            123
        }

        fn queue_max_sizes(&self) -> &[u16] {
            &[16, 32]
        }

        fn ctrl_queue_max_sizes(&self) -> u16 {
            self.ctrl_queue_size
        }

        fn set_acked_features(&mut self, page: u32, value: u32) {
            self.state.lock().unwrap().set_acked_features(page, value);
        }

        fn read_config(&mut self, offset: u64, data: &mut [u8]) -> ConfigResult {
            self.state.lock().unwrap().read_config(offset, data)
        }

        fn write_config(&mut self, offset: u64, data: &[u8]) -> ConfigResult {
            self.state.lock().unwrap().write_config(offset, data)
        }

        fn activate(&mut self, config: VirtioDeviceConfig<Arc<GuestMemoryMmap>>) -> ActivateResult {
            self.config.lock().unwrap().replace(config);
            Ok(())
        }

        fn reset(&mut self) -> ActivateResult {
            Ok(())
        }

        fn set_resource(
            &mut self,
            vm_fd: Arc<VmFd>,
            resource: DeviceResources,
        ) -> Result<Option<VirtioSharedMemoryList<GuestRegionMmap>>> {
            let mmio_res = resource.get_mmio_address_ranges();
            let slot_res = resource.get_kvm_mem_slots();

            if mmio_res.is_empty() || slot_res.is_empty() {
                return Ok(None);
            }

            let guest_addr = mmio_res[0].0;
            let len = mmio_res[0].1;

            let mmap_region = GuestRegionMmap::new(
                MmapRegion::new(len as usize).unwrap(),
                GuestAddress(guest_addr),
            )
            .unwrap();
            let host_addr: u64 = mmap_region
                .get_host_address(MemoryRegionAddress(0))
                .unwrap() as u64;
            let kvm_mem_region = kvm_userspace_memory_region {
                slot: slot_res[0],
                flags: 0,
                guest_phys_addr: guest_addr,
                memory_size: len,
                userspace_addr: host_addr,
            };
            unsafe { vm_fd.set_user_memory_region(kvm_mem_region).unwrap() };
            Ok(Some(VirtioSharedMemoryList {
                host_addr,
                guest_addr: GuestAddress(guest_addr),
                len,
                kvm_userspace_memory_region_flags: 0,
                kvm_userspace_memory_region_slot: slot_res[0],
                region_list: vec![VirtioSharedMemory {
                    offset: 0x40_0000,
                    len,
                }],
                mmap_region: Arc::new(mmap_region),
            }))
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

    pub fn set_driver_status(
        d: &mut MmioV2Device<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap>,
        status: u32,
    ) {
        let mut buf = vec![0; 4];
        LittleEndian::write_u32(&mut buf[..], status);
        d.write(IoAddress(0), IoAddress(REG_MMIO_STATUS), &buf[..]);
    }

    pub fn get_device_resource(have_msi_feature: bool, shared_memory: bool) -> DeviceResources {
        let mut resources = DeviceResources::new();
        resources.append(Resource::MmioAddressRange {
            base: 0,
            size: MMIO_DEFAULT_CFG_SIZE + DRAGONBALL_MMIO_DOORBELL_SIZE,
        });
        resources.append(Resource::LegacyIrq(5));
        if have_msi_feature {
            resources.append(Resource::MsiIrq {
                ty: MsiIrqType::GenericMsi,
                base: 24,
                size: 1,
            });
        }
        if shared_memory {
            resources.append(Resource::MmioAddressRange {
                base: 0x1_0000_0000,
                size: 0x1000,
            });

            resources.append(Resource::KvmMemSlot(1));
        }
        resources
    }

    pub fn get_mmio_device_inner(
        doorbell: bool,
        ctrl_queue_size: u16,
        resources: DeviceResources,
    ) -> MmioV2Device<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap> {
        let device = MmioDevice::new(ctrl_queue_size);
        let mem = Arc::new(GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x1000)]).unwrap());
        let kvm = Kvm::new().unwrap();
        let vm_fd = Arc::new(kvm.create_vm().unwrap());
        vm_fd.create_irq_chip().unwrap();
        let irq_manager = Arc::new(KvmIrqManager::new(vm_fd.clone()));
        irq_manager.initialize().unwrap();

        let features = if doorbell {
            Some(DRAGONBALL_FEATURE_PER_QUEUE_NOTIFY)
        } else {
            None
        };

        let address_space = create_address_space();

        MmioV2Device::new(
            vm_fd,
            mem,
            address_space,
            irq_manager,
            Box::new(device),
            resources,
            features,
        )
        .unwrap()
    }

    pub fn get_mmio_device() -> MmioV2Device<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap> {
        let resources = get_device_resource(false, false);
        get_mmio_device_inner(false, 0, resources)
    }

    #[test]
    fn test_virtio_mmio_v2_device_new() {
        // test create error.
        let resources = DeviceResources::new();
        let mem = Arc::new(GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x1000)]).unwrap());
        let device = MmioDevice::new(0);
        let kvm = Kvm::new().unwrap();
        let vm_fd = Arc::new(kvm.create_vm().unwrap());
        vm_fd.create_irq_chip().unwrap();
        let irq_manager = Arc::new(KvmIrqManager::new(vm_fd.clone()));
        irq_manager.initialize().unwrap();
        let address_space = create_address_space();
        let ret = MmioV2Device::new(
            vm_fd,
            mem,
            address_space,
            irq_manager,
            Box::new(device),
            resources,
            None,
        );
        assert!(matches!(ret, Err(Error::InvalidInput)));

        // test create without msi
        let mut d = get_mmio_device();

        set_driver_status(&mut d, DEVICE_ACKNOWLEDGE);
        assert_eq!(d.driver_status(), DEVICE_STATUS_ACKNOWLEDE);
        set_driver_status(&mut d, DEVICE_ACKNOWLEDGE | DEVICE_DRIVER);
        assert_eq!(d.driver_status(), DEVICE_STATUS_DRIVER);
        set_driver_status(
            &mut d,
            DEVICE_ACKNOWLEDGE | DEVICE_DRIVER | DEVICE_FEATURES_OK,
        );
        assert_eq!(d.driver_status(), DEVICE_STATUS_FEATURE_OK);

        set_driver_status(
            &mut d,
            DEVICE_ACKNOWLEDGE | DEVICE_DRIVER | DEVICE_FEATURES_OK | DEVICE_STATUS_DRIVER_OK,
        );
        assert_ne!(d.driver_status() & DEVICE_FAILED, 0);

        // test create with msi
        let d_mmio_feature = get_mmio_device_inner(false, 0, get_device_resource(true, false));
        assert_ne!(
            d_mmio_feature.device_vendor & DRAGONBALL_FEATURE_MSI_INTR,
            0
        );

        // test create with doorbell features
        let d_doorbell = get_mmio_device_inner(true, 0, get_device_resource(false, false));
        assert_ne!(
            d_doorbell.device_vendor & DRAGONBALL_FEATURE_PER_QUEUE_NOTIFY,
            0
        );

        // test ctrl queue
        let d_ctrl = get_mmio_device_inner(true, 1, get_device_resource(false, false));
        assert_eq!(d_ctrl.state().queues().len(), 3);
    }

    #[test]
    fn test_bus_device_read() {
        let mut d = get_mmio_device();

        let mut buf = vec![0xff, 0, 0xfe, 0];
        let buf_copy = buf.to_vec();

        // The following read shouldn't be valid, because the length of the buf is not 4.
        buf.push(0);
        d.read(IoAddress(0), IoAddress(0), &mut buf[..]);
        assert_eq!(buf[..4], buf_copy[..]);

        // the length is ok again
        buf.pop();

        let mut dev_cfg = vec![0; 4];
        d.read(
            IoAddress(0),
            IoAddress(MMIO_CFG_SPACE_OFF),
            &mut dev_cfg[..],
        );
        assert_eq!(LittleEndian::read_u32(&dev_cfg[..]), 0x0);

        // Now we test that reading at various predefined offsets works as intended.
        d.read(IoAddress(0), IoAddress(REG_MMIO_MAGIC_VALUE), &mut buf[..]);
        assert_eq!(LittleEndian::read_u32(&buf[..]), MMIO_MAGIC_VALUE);

        d.read(IoAddress(0), IoAddress(REG_MMIO_VERSION), &mut buf[..]);
        assert_eq!(LittleEndian::read_u32(&buf[..]), MMIO_VERSION_2);

        d.read(IoAddress(0), IoAddress(REG_MMIO_DEVICE_ID), &mut buf[..]);
        assert_eq!(
            LittleEndian::read_u32(&buf[..]),
            d.state().get_inner_device().device_type()
        );

        d.read(IoAddress(0), IoAddress(REG_MMIO_VENDOR_ID), &mut buf[..]);
        assert_eq!(LittleEndian::read_u32(&buf[..]), MMIO_VENDOR_ID_DRAGONBALL);

        d.state().set_features_select(0);
        d.read(
            IoAddress(0),
            IoAddress(REG_MMIO_DEVICE_FEATURE),
            &mut buf[..],
        );
        assert_eq!(
            LittleEndian::read_u32(&buf[..]),
            d.state().get_inner_device().get_avail_features(0)
        );

        d.state().set_features_select(1);
        d.read(
            IoAddress(0),
            IoAddress(REG_MMIO_DEVICE_FEATURE),
            &mut buf[..],
        );
        assert_eq!(
            LittleEndian::read_u32(&buf[..]),
            d.state().get_inner_device().get_avail_features(0) | 0x1
        );

        d.read(IoAddress(0), IoAddress(REG_MMIO_QUEUE_NUM_MA), &mut buf[..]);
        assert_eq!(LittleEndian::read_u32(&buf[..]), 16);

        d.read(IoAddress(0), IoAddress(REG_MMIO_QUEUE_READY), &mut buf[..]);
        assert_eq!(LittleEndian::read_u32(&buf[..]), false as u32);

        d.read(
            IoAddress(0),
            IoAddress(REG_MMIO_INTERRUPT_STAT),
            &mut buf[..],
        );
        assert_eq!(LittleEndian::read_u32(&buf[..]), 0);

        d.read(IoAddress(0), IoAddress(REG_MMIO_STATUS), &mut buf[..]);
        assert_eq!(LittleEndian::read_u32(&buf[..]), 0);

        d.config_generation.store(5, Ordering::SeqCst);
        d.read(
            IoAddress(0),
            IoAddress(REG_MMIO_CONFIG_GENERATI),
            &mut buf[..],
        );
        assert_eq!(LittleEndian::read_u32(&buf[..]), 5);

        // This read shouldn't do anything, as it's past the readable generic registers, and
        // before the device specific configuration space. Btw, reads from the device specific
        // conf space are going to be tested a bit later, alongside writes.
        buf = buf_copy.to_vec();
        d.read(IoAddress(0), IoAddress(0xfd), &mut buf[..]);
        assert_eq!(buf[..], buf_copy[..]);

        // Read from an invalid address in generic register range.
        d.read(IoAddress(0), IoAddress(0xfb), &mut buf[..]);
        assert_eq!(buf[..], buf_copy[..]);

        // Read from an invalid length in generic register range.
        d.read(IoAddress(0), IoAddress(0xfc), &mut buf[..3]);
        assert_eq!(buf[..], buf_copy[..]);

        // test for no msi_feature
        let mut buf = vec![0; 2];
        d.read(IoAddress(0), IoAddress(REG_MMIO_MSI_CSR), &mut buf[..]);
        assert_eq!(LittleEndian::read_u16(&buf[..]), 0);

        // test for msi_feature
        d.device_vendor |= DRAGONBALL_FEATURE_MSI_INTR;
        let mut buf = vec![0; 2];
        d.read(IoAddress(0), IoAddress(REG_MMIO_MSI_CSR), &mut buf[..]);
        assert_eq!(LittleEndian::read_u16(&buf[..]), MMIO_MSI_CSR_SUPPORTED);

        let mut dev_cfg = vec![0; 4];
        assert_eq!(
            d.exchange_driver_status(0, DEVICE_DRIVER | DEVICE_INIT)
                .unwrap(),
            0
        );
        d.read(
            IoAddress(0),
            IoAddress(MMIO_CFG_SPACE_OFF),
            &mut dev_cfg[..],
        );
        assert_eq!(LittleEndian::read_u32(&dev_cfg[..]), 0xffffffff);
    }

    #[test]
    fn test_bus_device_write() {
        let mut d = get_mmio_device();

        let mut buf = vec![0; 5];
        LittleEndian::write_u32(&mut buf[..4], 1);

        // Nothing should happen, because the slice len > 4.
        d.state().set_features_select(0);
        d.write(
            IoAddress(0),
            IoAddress(REG_MMIO_DEVICE_FEATURES_S),
            &buf[..],
        );
        assert_eq!(d.state().features_select(), 0);

        set_driver_status(&mut d, DEVICE_ACKNOWLEDGE);
        assert_eq!(d.driver_status(), DEVICE_STATUS_ACKNOWLEDE);
        set_driver_status(&mut d, DEVICE_STATUS_DRIVER);
        assert_eq!(d.driver_status(), DEVICE_STATUS_DRIVER);

        let mut buf = vec![0; 4];
        buf[0] = 0xa5;
        d.write(IoAddress(0), IoAddress(MMIO_CFG_SPACE_OFF), &buf[..]);
        buf[0] = 0;
        d.read(IoAddress(0), IoAddress(MMIO_CFG_SPACE_OFF), &mut buf[..]);
        assert_eq!(buf[0], 0xa5);
        assert_eq!(buf[1], 0);

        // Acking features in invalid state shouldn't take effect.
        d.state().set_acked_features_select(0x0);
        LittleEndian::write_u32(&mut buf[..], 1);
        d.write(IoAddress(0), IoAddress(REG_MMIO_DRIVER_FEATURE), &buf[..]);
        // TODO: find a way to check acked features

        // now writes should work
        d.state().set_features_select(0);
        LittleEndian::write_u32(&mut buf[..], 1);
        d.write(
            IoAddress(0),
            IoAddress(REG_MMIO_DEVICE_FEATURES_S),
            &buf[..],
        );
        assert_eq!(d.state().features_select(), 1);

        d.state().set_acked_features_select(0x123);
        LittleEndian::write_u32(&mut buf[..], 1);
        d.write(IoAddress(0), IoAddress(REG_MMIO_DRIVER_FEATURE), &buf[..]);
        // TODO: find a way to check acked features

        d.state().set_acked_features_select(0);
        LittleEndian::write_u32(&mut buf[..], 2);
        d.write(
            IoAddress(0),
            IoAddress(REG_MMIO_DRIVER_FEATURES_S),
            &buf[..],
        );
        assert_eq!(d.state().acked_features_select(), 2);

        set_driver_status(&mut d, DEVICE_STATUS_FEATURE_OK);
        assert_eq!(d.driver_status(), DEVICE_STATUS_FEATURE_OK);

        // Setup queues
        d.state().set_queue_select(0);
        LittleEndian::write_u32(&mut buf[..], 3);
        d.write(IoAddress(0), IoAddress(REG_MMIO_QUEUE_SEL), &buf[..]);
        assert_eq!(d.state().queue_select(), 3);

        d.state().set_queue_select(0);
        assert_eq!(d.state().queues()[0].queue.size(), 16);
        LittleEndian::write_u32(&mut buf[..], 8);
        d.write(IoAddress(0), IoAddress(REG_MMIO_QUEUE_NUM), &buf[..]);
        assert_eq!(d.state().queues()[0].queue.size(), 8);

        assert!(!d.state().queues()[0].queue.ready());
        LittleEndian::write_u32(&mut buf[..], 1);
        d.write(IoAddress(0), IoAddress(REG_MMIO_QUEUE_READY), &buf[..]);
        assert!(d.state().queues()[0].queue.ready());

        LittleEndian::write_u32(&mut buf[..], 0b111);
        d.write(IoAddress(0), IoAddress(REG_MMIO_INTERRUPT_AC), &buf[..]);

        assert_eq!(d.state().queues_mut()[0].queue.lock().desc_table(), 0);

        // When write descriptor, descriptor table will judge like this:
        // if desc_table.mask(0xf) != 0 {
        //     virtio queue descriptor table breaks alignment constraints
        // return
        // desc_table is the data that will be written.
        LittleEndian::write_u32(&mut buf[..], 0x120);
        d.write(IoAddress(0), IoAddress(REG_MMIO_QUEUE_DESC_LOW), &buf[..]);
        assert_eq!(d.state().queues_mut()[0].queue.lock().desc_table(), 0x120);
        d.write(IoAddress(0), IoAddress(REG_MMIO_QUEUE_DESC_HIGH), &buf[..]);
        assert_eq!(
            d.state().queues_mut()[0].queue.lock().desc_table(),
            0x120 + (0x120 << 32)
        );

        assert_eq!(d.state().queues_mut()[0].queue.lock().avail_ring(), 0);
        LittleEndian::write_u32(&mut buf[..], 124);
        d.write(IoAddress(0), IoAddress(REG_MMIO_QUEUE_AVAIL_LOW), &buf[..]);
        assert_eq!(d.state().queues_mut()[0].queue.lock().avail_ring(), 124);
        d.write(IoAddress(0), IoAddress(REG_MMIO_QUEUE_AVAIL_HIGH), &buf[..]);
        assert_eq!(
            d.state().queues_mut()[0].queue.lock().avail_ring(),
            124 + (124 << 32)
        );

        assert_eq!(d.state().queues_mut()[0].queue.lock().used_ring(), 0);
        LittleEndian::write_u32(&mut buf[..], 128);
        d.write(IoAddress(0), IoAddress(REG_MMIO_QUEUE_USED_LOW), &buf[..]);
        assert_eq!(d.state().queues_mut()[0].queue.lock().used_ring(), 128);
        d.write(IoAddress(0), IoAddress(REG_MMIO_QUEUE_USED_HIGH), &buf[..]);
        assert_eq!(
            d.state().queues_mut()[0].queue.lock().used_ring(),
            128 + (128 << 32)
        );

        // Write to an invalid address in generic register range.
        LittleEndian::write_u32(&mut buf[..], 0xf);
        d.config_generation.store(0, Ordering::SeqCst);
        d.write(IoAddress(0), IoAddress(0xfb), &buf[..]);
        assert_eq!(d.config_generation.load(Ordering::SeqCst), 0);

        // Write to an invalid length in generic register range.
        d.write(IoAddress(0), IoAddress(REG_MMIO_CONFIG_GENERATI), &buf[..2]);
        assert_eq!(d.config_generation.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_bus_device_activate() {
        // invalid state transition should failed
        let mut d = get_mmio_device();

        assert!(!d.state().check_queues_valid());
        assert!(!d.state().device_activated());
        assert_eq!(d.driver_status(), DEVICE_INIT);
        set_driver_status(&mut d, DEVICE_ACKNOWLEDGE);
        assert_eq!(d.driver_status(), DEVICE_ACKNOWLEDGE);
        set_driver_status(&mut d, DEVICE_ACKNOWLEDGE | DEVICE_DRIVER);
        assert_eq!(d.driver_status(), DEVICE_ACKNOWLEDGE | DEVICE_DRIVER);
        // Invalid state set
        set_driver_status(
            &mut d,
            DEVICE_ACKNOWLEDGE | DEVICE_DRIVER | DEVICE_DRIVER_OK,
        );
        assert_eq!(
            d.driver_status(),
            DEVICE_ACKNOWLEDGE | DEVICE_DRIVER | DEVICE_FAILED
        );

        // valid state transition
        let mut d = get_mmio_device();

        assert!(!d.state().check_queues_valid());
        assert!(!d.state().device_activated());
        assert_eq!(d.driver_status(), DEVICE_INIT);

        set_driver_status(&mut d, DEVICE_ACKNOWLEDGE);
        assert_eq!(d.driver_status(), DEVICE_ACKNOWLEDGE);
        set_driver_status(&mut d, DEVICE_ACKNOWLEDGE | DEVICE_DRIVER);
        assert_eq!(d.driver_status(), DEVICE_ACKNOWLEDGE | DEVICE_DRIVER);

        set_driver_status(
            &mut d,
            DEVICE_ACKNOWLEDGE | DEVICE_DRIVER | DEVICE_FEATURES_OK,
        );
        assert_eq!(
            d.driver_status(),
            DEVICE_ACKNOWLEDGE | DEVICE_DRIVER | DEVICE_FEATURES_OK
        );

        let mut buf = vec![0; 4];
        let size = d.state().queues().len();
        for q in 0..size {
            d.state().set_queue_select(q as u32);
            LittleEndian::write_u32(&mut buf[..], 16);
            d.write(IoAddress(0), IoAddress(REG_MMIO_QUEUE_NUM), &buf[..]);
            LittleEndian::write_u32(&mut buf[..], 1);
            d.write(IoAddress(0), IoAddress(REG_MMIO_QUEUE_READY), &buf[..]);
        }
        assert!(d.state().check_queues_valid());
        assert!(!d.state().device_activated());

        // Device should be ready for activation now.

        // A couple of invalid writes; will trigger warnings; shouldn't activate the device.
        d.write(IoAddress(0), IoAddress(0xa8), &buf[..]);
        assert!(!d.state().device_activated());

        set_driver_status(
            &mut d,
            DEVICE_ACKNOWLEDGE | DEVICE_DRIVER | DEVICE_FEATURES_OK | DEVICE_DRIVER_OK,
        );
        assert_eq!(
            d.driver_status(),
            DEVICE_ACKNOWLEDGE | DEVICE_DRIVER | DEVICE_FEATURES_OK | DEVICE_DRIVER_OK
        );
        assert!(d.state().device_activated());

        // activate again
        set_driver_status(
            &mut d,
            DEVICE_ACKNOWLEDGE | DEVICE_DRIVER | DEVICE_FEATURES_OK | DEVICE_DRIVER_OK,
        );
        assert!(d.state().device_activated());

        // A write which changes the size of a queue after activation; currently only triggers
        // a warning path and have no effect on queue state.
        LittleEndian::write_u32(&mut buf[..], 0);
        d.state().set_queue_select(0);
        d.write(IoAddress(0), IoAddress(REG_MMIO_QUEUE_READY), &buf[..]);
        d.read(IoAddress(0), IoAddress(REG_MMIO_QUEUE_READY), &mut buf[..]);
        assert_eq!(LittleEndian::read_u32(&buf[..]), 1);
    }

    fn activate_device(d: &mut MmioV2Device<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap>) {
        set_driver_status(d, DEVICE_ACKNOWLEDGE);
        set_driver_status(d, DEVICE_ACKNOWLEDGE | DEVICE_DRIVER);
        set_driver_status(d, DEVICE_ACKNOWLEDGE | DEVICE_DRIVER | DEVICE_FEATURES_OK);

        // Setup queue data structures
        let mut buf = vec![0; 4];
        let size = d.state().queues().len();
        for q in 0..size {
            d.state().set_queue_select(q as u32);
            LittleEndian::write_u32(&mut buf[..], 16);
            d.write(IoAddress(0), IoAddress(REG_MMIO_QUEUE_NUM), &buf[..]);
            LittleEndian::write_u32(&mut buf[..], 1);
            d.write(IoAddress(0), IoAddress(REG_MMIO_QUEUE_READY), &buf[..]);
        }
        assert!(d.state().check_queues_valid());
        assert!(!d.state().device_activated());

        // Device should be ready for activation now.
        set_driver_status(
            d,
            DEVICE_ACKNOWLEDGE | DEVICE_DRIVER | DEVICE_FEATURES_OK | DEVICE_DRIVER_OK,
        );
        assert_eq!(
            d.driver_status(),
            DEVICE_ACKNOWLEDGE | DEVICE_DRIVER | DEVICE_FEATURES_OK | DEVICE_DRIVER_OK
        );
        assert!(d.state().device_activated());
    }

    #[test]
    fn test_bus_device_reset() {
        let resources = get_device_resource(false, false);
        let mut d = get_mmio_device_inner(true, 0, resources);
        let mut buf = vec![0; 4];

        assert!(!d.state().check_queues_valid());
        assert!(!d.state().device_activated());
        assert_eq!(d.driver_status(), 0);
        activate_device(&mut d);

        // Marking device as FAILED should not affect device_activated state
        LittleEndian::write_u32(&mut buf[..], 0x8f);
        d.write(IoAddress(0), IoAddress(REG_MMIO_STATUS), &buf[..]);
        assert_eq!(d.driver_status(), 0x8f);
        assert!(d.state().device_activated());

        // Nothing happens when backend driver doesn't support reset
        LittleEndian::write_u32(&mut buf[..], 0x0);
        d.write(IoAddress(0), IoAddress(REG_MMIO_STATUS), &buf[..]);
        assert_eq!(d.driver_status(), 0x8f);
        assert!(!d.state().device_activated());

        // test for reactivate device
        // but device don't support reactivate now
        d.state().deactivate();
        assert!(!d.state().device_activated());
    }

    #[test]
    fn test_mmiov2_device_resources() {
        let d = get_mmio_device();

        let resources = d.get_assigned_resources();
        assert_eq!(resources.len(), 2);
        let resources = d.get_trapped_io_resources();
        assert_eq!(resources.len(), 1);
        let mmio_cfg_res = resources.get_mmio_address_ranges();
        assert_eq!(mmio_cfg_res.len(), 1);
        assert_eq!(
            mmio_cfg_res[0].1,
            MMIO_DEFAULT_CFG_SIZE + DRAGONBALL_MMIO_DOORBELL_SIZE
        );
    }

    #[test]
    fn test_mmio_v2_device_msi() {
        let resources = get_device_resource(true, false);
        let mut d = get_mmio_device_inner(true, 0, resources);

        let mut buf = vec![0; 4];
        LittleEndian::write_u32(&mut buf[..], 0x1234);
        d.write(IoAddress(0), IoAddress(REG_MMIO_MSI_ADDRESS_L), &buf[..]);
        LittleEndian::write_u32(&mut buf[..], 0x5678);
        d.write(IoAddress(0), IoAddress(REG_MMIO_MSI_ADDRESS_H), &buf[..]);
        LittleEndian::write_u32(&mut buf[..], 0x11111111);
        d.write(IoAddress(0), IoAddress(REG_MMIO_MSI_DATA), &buf[..]);

        // Enable msi
        LittleEndian::write_u16(&mut buf[..], MMIO_MSI_CSR_ENABLED);
        d.write(IoAddress(0), IoAddress(REG_MMIO_MSI_CSR), &buf[..2]);

        // Activate the device, it will enable interrupts.
        activate_device(&mut d);

        // update msi index
        LittleEndian::write_u16(&mut buf[..], MMIO_MSI_CMD_CODE_UPDATE);
        d.write(IoAddress(0), IoAddress(REG_MMIO_MSI_COMMAND), &buf[..2]);

        // update msi int mask
        LittleEndian::write_u16(&mut buf[..], MMIO_MSI_CMD_CODE_INT_MASK);
        d.write(IoAddress(0), IoAddress(REG_MMIO_MSI_COMMAND), &buf[..2]);

        // update msi int unmask
        LittleEndian::write_u16(&mut buf[..], MMIO_MSI_CMD_CODE_INT_UNMASK);
        d.write(IoAddress(0), IoAddress(REG_MMIO_MSI_COMMAND), &buf[..2]);

        // unknown msi command
        LittleEndian::write_u16(&mut buf[..], 0x4000);
        d.write(IoAddress(0), IoAddress(REG_MMIO_MSI_COMMAND), &buf[..2]);
        assert_ne!(d.driver_status() & DEVICE_FAILED, 0);

        // Disable msi
        LittleEndian::write_u16(&mut buf[..], 0);
        d.write(IoAddress(0), IoAddress(REG_MMIO_MSI_CSR), &buf[..2]);
    }

    #[test]
    fn test_mmio_shared_memory() {
        let resources = get_device_resource(true, true);
        let d = get_mmio_device_inner(true, 0, resources);

        let mut buf = vec![0; 4];

        // shm select 0
        d.write(IoAddress(0), IoAddress(REG_MMIO_SHM_SEL), &buf[..]);

        d.read(IoAddress(0), IoAddress(REG_MMIO_SHM_LEN_LOW), &mut buf[..]);
        assert_eq!(LittleEndian::read_u32(&buf[..]), 0x1000);

        d.read(IoAddress(0), IoAddress(REG_MMIO_SHM_LEN_HIGH), &mut buf[..]);
        assert_eq!(LittleEndian::read_u32(&buf[..]), 0x0);

        d.read(IoAddress(0), IoAddress(REG_MMIO_SHM_BASE_LOW), &mut buf[..]);
        assert_eq!(LittleEndian::read_u32(&buf[..]), 0x40_0000);

        d.read(
            IoAddress(0),
            IoAddress(REG_MMIO_SHM_BASE_HIGH),
            &mut buf[..],
        );
        assert_eq!(LittleEndian::read_u32(&buf[..]), 0x1);
    }
}
