// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.
//
// Copyright © 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause

// Copyright (C) 2024 Alibaba Cloud. All rights reserved.
//
// Copyright (C) 2025 AntGroup. All rights reserved.

use log::{debug, error, info, trace, warn};
use std::any::Any;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use crate::{
    MsixCap, MsixState, PciBarConfiguration, PciBarPrefetchable, PciBarRegionType, PciBus,
    PciCapability, PciCapabilityId, PciClassCode, PciConfiguration, PciDevice, PciHeaderType,
    PciMassStorageSubclass, PciNetworkControllerSubclass, PciSubclass,
};
use byteorder::{ByteOrder, LittleEndian};
use dbs_address_space::AddressSpace;
use dbs_device::resources::{DeviceResources, Resource, ResourceConstraint};
use dbs_device::{DeviceIo, IoAddress};
use dbs_interrupt::{DeviceInterruptManager, DeviceInterruptMode, KvmIrqManager};
use kvm_ioctls::{IoEventAddress, NoDatamatch, VmFd};
use virtio_queue::QueueT;
use vm_memory::{Address, ByteValued, GuestAddress, GuestAddressSpace, GuestMemoryRegion, Le32};

use super::pci_common_config::{VirtioPciCommonConfig, VirtioPciCommonConfigState};
use dbs_virtio_devices::{TYPE_BLOCK, TYPE_NET};

use dbs_interrupt::{
    InterruptNotifier, VirtioInterruptType, VirtioNotifierMsix, VIRTQ_MSI_NO_VECTOR,
};

use crate::ArcMutexBoxDynVirtioDevice;

use dbs_virtio_devices::{
    ActivateError, ActivateResult, VirtioDevice, VirtioDeviceConfig, VirtioQueueConfig,
    VirtioSharedMemoryList, DEVICE_ACKNOWLEDGE, DEVICE_DRIVER, DEVICE_DRIVER_OK, DEVICE_FAILED,
    DEVICE_FEATURES_OK, DEVICE_INIT,
};

#[allow(dead_code)]
enum PciCapabilityType {
    Common = 1,
    Notify = 2,
    Isr = 3,
    Device = 4,
    // Each structure can be mapped by a Base Address register (BAR) belonging to the function,
    // or accessed via the special VIRTIO_PCI_CAP_PCI_CFG field in the PCI configuration space.
    // The VIRTIO_PCI_CAP_PCI_CFG capability creates an alternative (and likely suboptimal) access method
    // to the common configuration, notification, ISR and device-specific configuration regions.
    // This type is defined in the virtio spec, but is not currently implemented.
    Pci = 5,
    // 6~7、9~19 are currently reserved for upstream
    // the auxiliary notification feature is just a proposal and has not been formally integrated
    // virito spce draft: https://uarif1.github.io/vvu/virtio-v1.1-cs01
    DeviceAuxNotify = 20,
    DriverAuxNotify = 21,
    SharedMemory = 8,
}

macro_rules! impl_pci_capability {
    ($name:ident) => {
        // SAFETY: All members are simple numbers and any value is valid.
        unsafe impl ByteValued for $name {}

        impl PciCapability for $name {
            fn pci_capability_type(&self) -> PciCapabilityId {
                self.cap_id.into()
            }

            fn len(&self) -> usize {
                self.as_slice().len()
            }

            fn set_next_cap(&mut self, next: u8) {
                self.cap_next = next;
            }

            fn read_u8(&mut self, offset: usize) -> u8 {
                if offset < self.len() {
                    self.as_slice()[offset]
                } else {
                    0xff
                }
            }

            fn write_u8(&mut self, offset: usize, value: u8) {
                if offset < self.len() {
                    self.as_mut_slice()[offset] = value;
                }
            }
        }
    };
    ($name:ident, $field:ident) => {
        // SAFETY: All members are simple numbers and any value is valid.
        unsafe impl ByteValued for $name {}

        impl PciCapability for $name {
            fn pci_capability_type(&self) -> PciCapabilityId {
                self.$field.cap_id.into()
            }

            fn len(&self) -> usize {
                self.as_slice().len()
            }

            fn set_next_cap(&mut self, next: u8) {
                self.$field.cap_next = next;
            }

            fn read_u8(&mut self, offset: usize) -> u8 {
                if offset < self.len() {
                    self.as_slice()[offset]
                } else {
                    0xff
                }
            }

            fn write_u8(&mut self, offset: usize, value: u8) {
                if offset < self.len() {
                    self.as_mut_slice()[offset] = value;
                }
            }
        }
    };
}

#[allow(dead_code)]
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct VirtioPciCap {
    cap_id: u8,       // Capability ID
    cap_next: u8,     // Offset of next capability structure
    cap_len: u8,      // Generic PCI field: capability length
    cfg_type: u8,     // Identifies the structure.
    pci_bar: u8,      // Where to find it.
    id: u8,           // Multiple capabilities of the same type
    padding: [u8; 2], // Pad to full dword.
    offset: Le32,     // Offset within bar.
    length: Le32,     // Length of the structure, in bytes.
}

impl VirtioPciCap {
    pub fn new(cfg_type: PciCapabilityType, pci_bar: u8, offset: u32, length: u32) -> Self {
        VirtioPciCap {
            cap_id: PciCapabilityId::VendorSpecific as u8,
            cap_next: 0,
            cap_len: std::mem::size_of::<VirtioPciCap>() as u8,
            cfg_type: cfg_type as u8,
            pci_bar,
            id: 0,
            padding: [0; 2],
            offset: Le32::from(offset),
            length: Le32::from(length),
        }
    }
}

#[allow(dead_code)]
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct VirtioPciNotifyCap {
    cap: VirtioPciCap,
    notify_off_multiplier: Le32, // Multiplier for queue_notify_off
}

impl VirtioPciNotifyCap {
    pub fn new(
        cfg_type: PciCapabilityType,
        pci_bar: u8,
        offset: u32,
        length: u32,
        multiplier: Le32,
    ) -> Self {
        VirtioPciNotifyCap {
            cap: VirtioPciCap {
                cap_id: PciCapabilityId::VendorSpecific as u8,
                cap_next: 0,
                cap_len: std::mem::size_of::<VirtioPciNotifyCap>() as u8,
                cfg_type: cfg_type as u8,
                pci_bar,
                id: 0,
                padding: [0; 2],
                offset: Le32::from(offset),
                length: Le32::from(length),
            },
            notify_off_multiplier: multiplier,
        }
    }
}

#[allow(dead_code)]
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct VirtioPciCap64 {
    cap: VirtioPciCap,
    offset_hi: Le32,
    length_hi: Le32,
}

impl VirtioPciCap64 {
    pub fn new(cfg_type: PciCapabilityType, pci_bar: u8, id: u8, offset: u64, length: u64) -> Self {
        VirtioPciCap64 {
            cap: VirtioPciCap {
                cap_id: PciCapabilityId::VendorSpecific as u8,
                cap_next: 0,
                cap_len: std::mem::size_of::<VirtioPciCap64>() as u8,
                cfg_type: cfg_type as u8,
                pci_bar,
                id,
                padding: [0; 2],
                offset: Le32::from(offset as u32),
                length: Le32::from(length as u32),
            },
            offset_hi: Le32::from((offset >> 32) as u32),
            length_hi: Le32::from((length >> 32) as u32),
        }
    }
}

impl_pci_capability!(VirtioPciCap);
impl_pci_capability!(VirtioPciNotifyCap, cap);
impl_pci_capability!(VirtioPciCap64, cap);

/// Subclasses for PciClassCode Other.
#[derive(Copy, Clone)]
#[repr(u8)]
pub enum PciOtherSubclass {
    Other = 0xff,
}

impl PciSubclass for PciOtherSubclass {
    fn get_register_value(&self) -> u8 {
        *self as u8
    }
}

// Allocate one bar for the structs pointed to by the capability structures.
// As per the PCI specification, because the same BAR shares MSI-X and non
// MSI-X structures, it is recommended to use 8KiB alignment for all those
// structures.
const COMMON_CONFIG_BAR_OFFSET: u64 = 0x0000;
const COMMON_CONFIG_SIZE: u64 = 56;
const ISR_CONFIG_BAR_OFFSET: u64 = 0x2000;
const ISR_CONFIG_SIZE: u64 = 1;
const DEVICE_CONFIG_BAR_OFFSET: u64 = 0x4000;
const DEVICE_CONFIG_SIZE: u64 = 0x1000;
const NOTIFICATION_BAR_OFFSET: u64 = 0x6000;
const NOTIFICATION_SIZE: u64 = 0x1000;
const MSIX_TABLE_BAR_OFFSET: u64 = 0x8000;

// The size is 256KiB because the table can hold up to 2048 entries, with each
// entry being 128 bits (4 DWORDS).
const MSIX_TABLE_SIZE: u64 = 0x40000;
const MSIX_PBA_BAR_OFFSET: u64 = 0x48000;
// The size is 2KiB because the Pending Bit Array has one bit per vector and it
// can support up to 2048 vectors.
const MSIX_PBA_SIZE: u64 = 0x800;
// The BAR size must be a power of 2.
pub const CAPABILITY_BAR_SIZE: u64 = 0x80000;
const VIRTIO_COMMON_BAR_INDEX: usize = 0;
const VIRTIO_SHM_BAR_INDEX: usize = 2;

const NOTIFY_OFF_MULTIPLIER: u32 = 4; // A dword per notification address.

const VIRTIO_PCI_VENDOR_ID: u16 = 0x1af4;
const VIRTIO_PCI_DEVICE_ID_BASE: u16 = 0x1040; // Add to device type to get device ID.

#[derive(thiserror::Error, Debug)]
pub enum VirtioPciDeviceError {
    #[error("Failed to get msix resource")]
    InvalidMsixResource,

    #[error("Failed to create eventfd: {0:?}")]
    CreateEventFd(#[source] std::io::Error),

    #[error("Failed to setup capabilties: {0:?}")]
    CapabilitiesSetup(#[source] crate::Error),

    #[error("Failed to create pci configuration: {0:?}")]
    CreatePciConfiguration(#[source] crate::Error),

    #[error("Failed to create device interrupt manager: {0:?}")]
    CreateInterruptManager(#[source] std::io::Error),

    #[error("Failed to setup interrupt working mode: {0:?}")]
    SetInterruptWorkingMode(#[source] std::io::Error),

    #[error("Missing setting bar resource")]
    MissingSettingBarResource,

    #[error("invalid resource: {0:?}")]
    InvalidResource(Resource),

    #[error("Failed to add device bar: {0:?}")]
    AddDeviceBar(#[source] crate::Error),

    #[error("Failed to registration io address is: {0:?}, err: is: {1:?}")]
    IoRegistrationFailed(GuestAddress, #[source] crate::Error),

    #[error("Interrupt group is disable or unactivated")]
    InvalidInterruptGroup,

    #[error("invalid device aux notification size: {0}")]
    InvalidDeviceAuxSize(u32),

    #[error("create virtio queue: {0:?}")]
    VirtioQueue(dbs_virtio_devices::Error),

    #[error("set virtio resource: {0:?}")]
    SetResource(dbs_virtio_devices::Error),

    #[error("failed to upgrade pci device bus since it is already dropped")]
    BusIsDropped,
}
pub type Result<T> = std::result::Result<T, VirtioPciDeviceError>;

pub struct VirtioPciDeviceState<AS: GuestAddressSpace + Clone + 'static, Q: QueueT> {
    vm_as: AS,
    queues: Vec<VirtioQueueConfig<Q>>,
}

impl<AS: GuestAddressSpace + Clone + 'static, Q: QueueT> VirtioPciDeviceState<AS, Q> {
    fn check_queues_valid(&self) -> bool {
        let mem = self.vm_as.memory();
        // All queues must have been enabled, we doesn't allow disabled queues.
        self.queues.iter().all(|c| c.queue.is_valid(mem.deref()))
    }
}

pub struct VirtioPciDevice<AS: GuestAddressSpace + Clone + 'static, Q: QueueT, R: GuestMemoryRegion>
{
    dev_id: u8,
    bus: Weak<PciBus>,
    vm_fd: Arc<VmFd>,
    address_space: AddressSpace,

    // Virtio device reference and status
    device: ArcMutexBoxDynVirtioDevice<AS, Q, R>,
    state: Mutex<VirtioPciDeviceState<AS, Q>>,
    device_resource: DeviceResources,
    shm_regions: Option<VirtioSharedMemoryList<R>>,
    has_ctrl_queue: bool,

    // PCI configuration registers.
    configuration: Mutex<PciConfiguration>,

    // virtio PCI common configuration
    common_config: Mutex<VirtioPciCommonConfig>,

    device_activated: Arc<AtomicBool>,

    ioevent_registered: AtomicBool,

    // ISR Status. The device MUST present at least one VIRTIO_PCI_CAP_ISR_CFG capability.
    // If MSI-X capability is disabled,  ISR Status will be setted for PCI interrupt.
    // Now virtio-pci only support MSI-X interrupts, so interrupt_status will be read and written,
    // but not actually used.
    interrupt_status: Arc<AtomicUsize>,

    // MSI-X interrupts.
    msix_state: Mutex<MsixState>,
    intr_mgr: Mutex<DeviceInterruptManager<Arc<KvmIrqManager>>>,
    msix_num: u16,
    // This is the index of MSI-X capability register in the PCI configuration space.
    // Inited when alloc_bars.
    // Never equals 0 after initalization.
    msix_cap_reg_idx: u32,

    // Settings PCI BAR
    settings_bar: u8,
    setting_bar_res: Resource,

    // Whether to use 64-bit bar location or 32-bit
    use_64bit_bar: bool,

    // Details of bar regions to free
    bar_regions: Vec<PciBarConfiguration>,
}

impl<AS, Q, R> VirtioPciDevice<AS, Q, R>
where
    AS: GuestAddressSpace + Clone,
    Q: QueueT,
    R: GuestMemoryRegion,
{
    /// Constructs a new PCI transport for the given virtio device.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        vm_fd: Arc<VmFd>,
        vm_as: AS,
        address_space: AddressSpace,
        irq_manager: Arc<KvmIrqManager>,
        device_resource: DeviceResources,
        dev_id: u8,
        device: Box<dyn VirtioDevice<AS, Q, R>>,
        use_64bit_bar: bool,
        bus: Weak<PciBus>,
        setting_bar_res: Resource,
    ) -> Result<Self> {
        let (queues, has_ctrl_queue) = Self::create_queues(device.as_ref())?;

        let msix_res = device_resource
            .get_pci_msix_irqs()
            .ok_or(VirtioPciDeviceError::InvalidMsixResource)?;
        info!("{:?}: virtio pci device msix_res: {:?}", dev_id, msix_res);

        let msix_state = MsixState::new(msix_res.1 as u16);

        let mut intr_mgr = DeviceInterruptManager::new(irq_manager, &device_resource)
            .map_err(VirtioPciDeviceError::CreateInterruptManager)?;
        intr_mgr
            .set_working_mode(DeviceInterruptMode::PciMsixIrq)
            .map_err(VirtioPciDeviceError::SetInterruptWorkingMode)?;

        let num_queues = device.queue_max_sizes().len() + has_ctrl_queue as usize;

        let pci_device_id = VIRTIO_PCI_DEVICE_ID_BASE + device.device_type() as u16;

        // Refer to cloud-hypervisor
        let (class, subclass) = match device.device_type() {
            TYPE_NET => (
                PciClassCode::NetworkController,
                &PciNetworkControllerSubclass::EthernetController as &dyn PciSubclass,
            ),
            TYPE_BLOCK => (
                PciClassCode::MassStorage,
                &PciMassStorageSubclass::MassStorage as &dyn PciSubclass,
            ),
            _ => (
                PciClassCode::Other,
                &PciOtherSubclass::Other as &dyn PciSubclass,
            ),
        };

        let configuration = Mutex::new(
            PciConfiguration::new(
                bus.clone(),
                VIRTIO_PCI_VENDOR_ID,
                pci_device_id,
                class,
                subclass,
                None,
                PciHeaderType::Device,
                0,
                0,
                None,
            )
            .map_err(VirtioPciDeviceError::CreatePciConfiguration)?,
        );

        let common_config = VirtioPciCommonConfig::new(VirtioPciCommonConfigState {
            driver_status: 0,
            config_generation: 0,
            device_feature_select: 0,
            driver_feature_select: 0,
            queue_select: 0,
            msix_config: VIRTQ_MSI_NO_VECTOR,
            msix_queues: vec![VIRTQ_MSI_NO_VECTOR; num_queues],
        });

        let (device_activated, interrupt_status) = (false, 0);

        let state = VirtioPciDeviceState { vm_as, queues };

        let virtio_pci_device: VirtioPciDevice<AS, Q, R> = VirtioPciDevice {
            vm_fd,
            address_space,
            dev_id,
            bus,
            device: Arc::new(Mutex::new(device)),
            state: Mutex::new(state),
            device_resource,
            shm_regions: None,
            has_ctrl_queue,
            configuration,
            common_config: Mutex::new(common_config),
            device_activated: Arc::new(AtomicBool::new(device_activated)),
            interrupt_status: Arc::new(AtomicUsize::new(interrupt_status)),
            msix_state: Mutex::new(msix_state),
            intr_mgr: Mutex::new(intr_mgr),
            msix_num: msix_res.1 as u16,
            msix_cap_reg_idx: 0,
            ioevent_registered: AtomicBool::new(false),
            settings_bar: 0,
            setting_bar_res,
            use_64bit_bar,
            bar_regions: vec![],
        };

        Ok(virtio_pci_device)
    }

    /// Get interrupt resources requests that are required by the device.
    pub fn get_interrupt_requirements(
        device: &dyn VirtioDevice<AS, Q, R>,
        requests: &mut Vec<ResourceConstraint>,
    ) {
        // current pci only support msix
        let has_ctrl_queue = (device.ctrl_queue_max_sizes() > 0) as usize;
        requests.push(ResourceConstraint::PciMsixIrq {
            size: (device.queue_max_sizes().len() + has_ctrl_queue + 1) as u32,
        });
    }

    pub(crate) fn create_queues(
        device: &dyn VirtioDevice<AS, Q, R>,
    ) -> Result<(Vec<VirtioQueueConfig<Q>>, bool)> {
        let mut queues = Vec::new();
        for (idx, size) in device.queue_max_sizes().iter().enumerate() {
            queues.push(
                VirtioQueueConfig::create(*size, idx as u16)
                    .map_err(VirtioPciDeviceError::VirtioQueue)?,
            );
        }

        // The ctrl queue must be append to QueueState Vec, because the guest will
        // configure it which is same with other queues.
        let has_ctrl_queue = device.ctrl_queue_max_sizes() > 0;
        if has_ctrl_queue {
            queues.push(
                VirtioQueueConfig::create(device.ctrl_queue_max_sizes(), queues.len() as u16)
                    .map_err(VirtioPciDeviceError::VirtioQueue)?,
            );
        }

        Ok((queues, has_ctrl_queue))
    }

    fn is_driver_ready(&self) -> bool {
        let ready_bits =
            (DEVICE_ACKNOWLEDGE | DEVICE_DRIVER | DEVICE_DRIVER_OK | DEVICE_FEATURES_OK) as u8;
        let config = self.common_config();
        config.driver_status == ready_bits && config.driver_status & DEVICE_FAILED as u8 == 0
    }

    /// Determines if the driver has requested the device (re)init / reset itself
    fn is_driver_init(&self) -> bool {
        self.common_config().driver_status == DEVICE_INIT as u8
    }

    fn register_ioevent(&self) -> std::io::Result<()> {
        let notify_base = self.config_bar_addr() + NOTIFICATION_BAR_OFFSET;
        let state = self.state();
        for (i, q) in state.queues.iter().enumerate() {
            let addr =
                IoEventAddress::Mmio(notify_base + i as u64 * u64::from(NOTIFY_OFF_MULTIPLIER));
            if let Err(e) = self.vm_fd.register_ioevent(&q.eventfd, &addr, NoDatamatch) {
                error!("failed to register ioevent: {:?}", e);
                return Err(std::io::Error::from_raw_os_error(e.errno()));
            }
        }

        self.ioevent_registered.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn unregister_ioevent(&self) {
        if self.ioevent_registered.load(Ordering::SeqCst) {
            let notify_base = self.config_bar_addr() + NOTIFICATION_BAR_OFFSET;
            let state = self.state();
            for (i, q) in state.queues.iter().enumerate() {
                let addr =
                    IoEventAddress::Mmio(notify_base + i as u64 * u64::from(NOTIFY_OFF_MULTIPLIER));
                if let Err(e) = self
                    .vm_fd
                    .unregister_ioevent(&q.eventfd, &addr, NoDatamatch)
                {
                    warn!(
                        "failed to unregister ioevent: {:?}, idx: {:?}, eventfd: {:?}",
                        e, i, q.eventfd
                    );
                }
            }

            self.ioevent_registered.store(false, Ordering::SeqCst);
        }
    }

    pub fn config_bar_addr(&self) -> u64 {
        self.configuration
            .lock()
            .unwrap()
            .get_device_bar_addr(self.settings_bar as usize)
    }

    fn add_pci_capabilities(&mut self, settings_bar: u8) -> Result<()> {
        // Add pointers to the different configuration structures from the PCI capabilities.
        let common_cap = VirtioPciCap::new(
            PciCapabilityType::Common,
            settings_bar,
            COMMON_CONFIG_BAR_OFFSET as u32,
            COMMON_CONFIG_SIZE as u32,
        );
        self.configuration
            .lock()
            .unwrap()
            .add_capability(Arc::new(Mutex::new(Box::new(common_cap))))
            .map_err(VirtioPciDeviceError::CapabilitiesSetup)?;

        let isr_cap = VirtioPciCap::new(
            PciCapabilityType::Isr,
            settings_bar,
            ISR_CONFIG_BAR_OFFSET as u32,
            ISR_CONFIG_SIZE as u32,
        );
        self.configuration
            .lock()
            .unwrap()
            .add_capability(Arc::new(Mutex::new(Box::new(isr_cap))))
            .map_err(VirtioPciDeviceError::CapabilitiesSetup)?;

        // TODO - set based on device's configuration size?
        let device_cap = VirtioPciCap::new(
            PciCapabilityType::Device,
            settings_bar,
            DEVICE_CONFIG_BAR_OFFSET as u32,
            DEVICE_CONFIG_SIZE as u32,
        );
        self.configuration
            .lock()
            .unwrap()
            .add_capability(Arc::new(Mutex::new(Box::new(device_cap))))
            .map_err(VirtioPciDeviceError::CapabilitiesSetup)?;

        let notify_cap = VirtioPciNotifyCap::new(
            PciCapabilityType::Notify,
            settings_bar,
            NOTIFICATION_BAR_OFFSET as u32,
            NOTIFICATION_SIZE as u32,
            Le32::from(NOTIFY_OFF_MULTIPLIER),
        );
        self.configuration
            .lock()
            .unwrap()
            .add_capability(Arc::new(Mutex::new(Box::new(notify_cap))))
            .map_err(VirtioPciDeviceError::CapabilitiesSetup)?;

        if self.msix_num > 0 {
            let msix_cap = MsixCap::new(
                settings_bar,
                self.msix_num,
                MSIX_TABLE_BAR_OFFSET as u32,
                settings_bar,
                MSIX_PBA_BAR_OFFSET as u32,
            );
            let cap_offset = self
                .configuration
                .lock()
                .unwrap()
                .add_capability(Arc::new(Mutex::new(Box::new(msix_cap))))
                .map_err(VirtioPciDeviceError::CapabilitiesSetup)?;

            self.msix_cap_reg_idx = (cap_offset / 4) as u32;
            debug!(
                "{:?}: msix_cap_offset: {:x?}, msix_cap_reg_idx: {:x?}",
                self.dev_id, cap_offset, self.msix_cap_reg_idx
            );
        }

        self.settings_bar = settings_bar;
        Ok(())
    }

    pub fn device(&self) -> MutexGuard<Box<dyn VirtioDevice<AS, Q, R>>> {
        self.device.lock().expect("Poisoned lock of device")
    }

    pub fn arc_device(&self) -> ArcMutexBoxDynVirtioDevice<AS, Q, R> {
        self.device.clone()
    }

    pub fn common_config(&self) -> MutexGuard<VirtioPciCommonConfig> {
        self.common_config
            .lock()
            .expect("Poisoned lock of common_config")
    }

    pub fn state(&self) -> MutexGuard<VirtioPciDeviceState<AS, Q>> {
        self.state.lock().expect("Poisoned lock of state")
    }

    pub fn msix_state(&self) -> MutexGuard<MsixState> {
        self.msix_state.lock().expect("Poisoned lock of msix_state")
    }

    pub fn intr_mgr(&self) -> MutexGuard<DeviceInterruptManager<Arc<KvmIrqManager>>> {
        // Safe to unwrap() because we don't expect poisoned lock here.
        self.intr_mgr.lock().expect("Poisoned lock of intr_mgr")
    }

    pub fn device_id(&self) -> u8 {
        self.dev_id
    }

    pub fn bus_id(&self) -> Result<u8> {
        Ok(self
            .bus
            .upgrade()
            .ok_or(VirtioPciDeviceError::BusIsDropped)?
            .bus_id())
    }
}

impl<
        AS: GuestAddressSpace + Clone + 'static,
        Q: QueueT + Clone + 'static,
        R: GuestMemoryRegion + 'static,
    > VirtioPciDevice<AS, Q, R>
{
    fn needs_activation(&self) -> bool {
        !self.device_activated.load(Ordering::SeqCst) && self.is_driver_ready()
    }

    fn activate(&self) -> ActivateResult {
        if self.device_activated.load(Ordering::SeqCst) {
            return Ok(());
        }

        // If the driver incorrectly sets up the queues, the following check will fail and take
        // the device into an unusable state.
        if !self.state().check_queues_valid() {
            return Err(ActivateError::InvalidQueueConfig);
        }

        self.register_ioevent().map_err(ActivateError::IOError)?;

        self.intr_mgr().enable()?;

        let state = self.state();
        let config = self.common_config();
        let mut queues = Vec::new();

        let interrupt_source_group = self
            .intr_mgr()
            .get_group()
            .ok_or(ActivateError::VirtioPci)?;

        for queue in state.queues.iter() {
            let queue_notifier: Arc<dyn InterruptNotifier> = Arc::new(VirtioNotifierMsix::new(
                Arc::clone(&config.msix_config),
                Arc::clone(&config.msix_queues),
                Arc::clone(&interrupt_source_group),
                VirtioInterruptType::Queue(queue.index()),
            ));

            info!(
                "dev_id: {:?}, queue info: {:?}, {:?}",
                self.dev_id,
                queue.eventfd,
                queue_notifier.notifier()
            );

            queues.push(VirtioQueueConfig::<Q>::new(
                queue.queue.clone(),
                queue.eventfd.clone(),
                queue_notifier,
                queue.index(),
            ));
        }

        let ctrl_queue = if self.has_ctrl_queue {
            queues.pop()
        } else {
            None
        };
        let config_notifier: Arc<dyn InterruptNotifier> = Arc::new(VirtioNotifierMsix::new(
            Arc::clone(&config.msix_config),
            Arc::clone(&config.msix_queues),
            Arc::clone(&interrupt_source_group),
            VirtioInterruptType::Config,
        ));

        info!(
            "dev_id: {:?}, config info: {:?}",
            self.dev_id,
            config_notifier.notifier()
        );

        let mut device_config = VirtioDeviceConfig::new(
            state.vm_as.clone(),
            self.address_space.clone(),
            self.vm_fd.clone(),
            self.device_resource.clone(),
            queues,
            ctrl_queue,
            config_notifier,
        );

        if let Some(shm_regions) = self.shm_regions.as_ref() {
            device_config.set_shm_regions(shm_regions.clone());
        }

        drop(state);

        self.device
            .lock()
            .unwrap()
            .activate(device_config)
            .map(|_| self.device_activated.store(true, Ordering::SeqCst))
            .map_err(|e| {
                error!("device activate error: {:?}", e);
                e
            })?;

        Ok(())
    }

    /// TODO used by pci bar reprograming
    #[allow(dead_code)]
    pub fn move_bar(
        &mut self,
        old_base: u64,
        new_base: u64,
    ) -> std::result::Result<(), std::io::Error> {
        // We only update our idea of the bar in order to support free_bars() above.
        // The majority of the reallocation is done inside DeviceManager.
        for bar in self.bar_regions.iter_mut() {
            if bar.address() == old_base {
                *bar = bar.set_address(new_base);
            }
        }

        Ok(())
    }

    pub fn alloc_bars(&mut self) -> Result<()> {
        let mut bars = Vec::new();
        let settings_bar_addr: Option<u64>;

        if let Resource::MmioAddressRange { base, size: _ } = &self.setting_bar_res {
            settings_bar_addr = Some(*base);
        } else {
            return Err(VirtioPciDeviceError::InvalidResource(
                self.setting_bar_res.clone(),
            ));
        }

        // Error out if no resource was matching the BAR id.
        if settings_bar_addr.is_none() {
            return Err(VirtioPciDeviceError::MissingSettingBarResource);
        }

        // Safely: settings_bar_addr must be valid
        let settings_bar_addr = settings_bar_addr.unwrap();

        info!(
            "{:?}: settings_bar_addr 0x{:x}",
            self.dev_id, settings_bar_addr
        );

        let region_type = if self.use_64bit_bar {
            PciBarRegionType::Memory64BitRegion
        } else {
            PciBarRegionType::Memory32BitRegion
        };

        let bar = PciBarConfiguration::default()
            .set_bar_index(VIRTIO_COMMON_BAR_INDEX)
            .set_address(settings_bar_addr)
            .set_size(CAPABILITY_BAR_SIZE)
            .set_bar_type(region_type);

        self.configuration
            .lock()
            .unwrap()
            .add_device_bar(&bar)
            .map_err(VirtioPciDeviceError::AddDeviceBar)?;

        // Once the BARs are allocated, the capabilities can be added to the PCI configuration.
        self.add_pci_capabilities(VIRTIO_COMMON_BAR_INDEX as u8)?;
        bars.push(bar);

        // Assign requested device resources back to virtio device and let it do necessary setups,
        // as only virtio device knows how to use such resources. And if there's
        // VirtioSharedMemoryList returned, assigned it to VirtioPciDevice
        let shm_regions = self
            .device()
            .set_resource(self.vm_fd.clone(), self.device_resource.clone())
            .map_err(|e| {
                error!("Failed to assign device resource to virtio device: {}", e);
                VirtioPciDeviceError::SetResource(e)
            })?;

        if let Some(shm_list) = shm_regions {
            // Shared memory region should be Prefetchable to achieve performance optimization.
            let bar = PciBarConfiguration::default()
                .set_bar_index(VIRTIO_SHM_BAR_INDEX)
                .set_address(shm_list.guest_addr.raw_value())
                .set_size(shm_list.len)
                .set_prefetchable(PciBarPrefetchable::Prefetchable);

            self.configuration
                .lock()
                .unwrap()
                .add_device_bar(&bar)
                .map_err(|e| VirtioPciDeviceError::IoRegistrationFailed(shm_list.guest_addr, e))?;

            for (idx, shm) in shm_list.region_list.iter().enumerate() {
                let shm_cap = VirtioPciCap64::new(
                    PciCapabilityType::SharedMemory,
                    VIRTIO_SHM_BAR_INDEX as u8,
                    idx as u8,
                    shm.offset,
                    shm.len,
                );
                self.configuration
                    .lock()
                    .unwrap()
                    .add_capability(Arc::new(Mutex::new(Box::new(shm_cap))))
                    .map_err(VirtioPciDeviceError::CapabilitiesSetup)?;
            }

            self.shm_regions = Some(shm_list);
        }

        self.bar_regions.clone_from(&bars);
        Ok(())
    }

    fn read_bar(&self, _base: u64, offset: u64, data: &mut [u8]) {
        trace!(
            "{:?}: read BAR at: offset = 0x{:x}, data = {:x?}",
            self.dev_id,
            offset,
            data
        );
        match offset {
            o if o < COMMON_CONFIG_BAR_OFFSET + COMMON_CONFIG_SIZE => {
                let state = self.state();
                let config = self.common_config();
                config.read(
                    o - COMMON_CONFIG_BAR_OFFSET,
                    data,
                    &state.queues,
                    self.arc_device(),
                )
            }
            o if (ISR_CONFIG_BAR_OFFSET..ISR_CONFIG_BAR_OFFSET + ISR_CONFIG_SIZE).contains(&o) => {
                if let Some(v) = data.get_mut(0) {
                    // Reading this register resets it to 0.
                    *v = self.interrupt_status.swap(0, Ordering::AcqRel) as u8;
                }
            }
            o if (DEVICE_CONFIG_BAR_OFFSET..DEVICE_CONFIG_BAR_OFFSET + DEVICE_CONFIG_SIZE)
                .contains(&o) =>
            {
                let mut device = self.device();
                if let Err(e) = device.read_config(o - DEVICE_CONFIG_BAR_OFFSET, data) {
                    warn!("device read config err: {}", e);
                }
            }
            o if (NOTIFICATION_BAR_OFFSET..NOTIFICATION_BAR_OFFSET + NOTIFICATION_SIZE)
                .contains(&o) =>
            {
                // Handled with ioeventfds.
            }
            o if (MSIX_TABLE_BAR_OFFSET..MSIX_TABLE_BAR_OFFSET + MSIX_TABLE_SIZE).contains(&o) => {
                self.msix_state()
                    .read_table(o - MSIX_TABLE_BAR_OFFSET, data);
            }
            o if (MSIX_PBA_BAR_OFFSET..MSIX_PBA_BAR_OFFSET + MSIX_PBA_SIZE).contains(&o) => {
                let mut msix_state = self.msix_state();
                let mut intr_mgr = self.intr_mgr();
                msix_state.read_pba(o - MSIX_PBA_BAR_OFFSET, data, &mut intr_mgr);
            }
            _ => (),
        }
    }

    fn write_bar(&self, _base: u64, offset: u64, data: &[u8]) -> Option<()> {
        trace!(
            "{:?}: write BAR at: offset = 0x{:x}, data = {:x?}",
            self.dev_id,
            offset,
            data
        );
        match offset {
            o if o < COMMON_CONFIG_BAR_OFFSET + COMMON_CONFIG_SIZE => {
                let mut config = self.common_config();
                let mut state = self.state();
                config.write(
                    o - COMMON_CONFIG_BAR_OFFSET,
                    data,
                    &mut state.queues,
                    Arc::clone(&self.device),
                );
            }
            o if (ISR_CONFIG_BAR_OFFSET..ISR_CONFIG_BAR_OFFSET + ISR_CONFIG_SIZE).contains(&o) => {
                if let Some(v) = data.first() {
                    self.interrupt_status
                        .fetch_and(!(*v as usize), Ordering::AcqRel);
                }
            }
            o if (DEVICE_CONFIG_BAR_OFFSET..DEVICE_CONFIG_BAR_OFFSET + DEVICE_CONFIG_SIZE)
                .contains(&o) =>
            {
                let mut device = self.device();
                if let Err(e) = device.write_config(o - DEVICE_CONFIG_BAR_OFFSET, data) {
                    warn!("pci device write config err: {}", e);
                }
            }
            o if (NOTIFICATION_BAR_OFFSET..NOTIFICATION_BAR_OFFSET + NOTIFICATION_SIZE)
                .contains(&o) =>
            {
                error!(
                    "{:?}: Unexpected write to notification BAR: offset = 0x{:x}",
                    self.dev_id, o
                );
            }
            o if (MSIX_TABLE_BAR_OFFSET..MSIX_TABLE_BAR_OFFSET + MSIX_TABLE_SIZE).contains(&o) => {
                let mut msix_state = self.msix_state();
                let mut intr_mgr = self.intr_mgr();
                if let Err(e) =
                    msix_state.write_table(o - MSIX_TABLE_BAR_OFFSET, data, &mut intr_mgr)
                {
                    error!(
                        "{:?}: Failed to do msix_state.write_table, err: {:?}",
                        self.dev_id, e
                    );
                }
            }
            o if (MSIX_PBA_BAR_OFFSET..MSIX_PBA_BAR_OFFSET + MSIX_PBA_SIZE).contains(&o) => {
                error!(
                    "{:?}: Pending Bit Array is read only: offset = 0x{:x}",
                    self.dev_id, o
                );
            }

            _ => (),
        };

        // Try and activate the device if the driver status has changed
        if self.needs_activation() {
            info!("{:?}: start to activate device", self.dev_id);
            if let Err(e) = self.activate() {
                error!("failed to activate device: {:?}, err {:?}", self.dev_id, e);
                let mut config = self.common_config();
                config.driver_status = DEVICE_FAILED as u8;
            }
        }

        // Device has been reset by the driver
        if self.device_activated.load(Ordering::SeqCst) && self.is_driver_init() {
            let mut device = self.device();
            if let Err(e) = device.reset() {
                error!("Attempt to reset device when not implemented or reset error in underlying device, err: {:?}", e);
                let mut config = self.common_config();
                config.driver_status = DEVICE_FAILED as u8;
            } else {
                info!("Successfully reset device triggered due to the failure of activation");
                self.device_activated.store(false, Ordering::SeqCst);

                let mut state = self.state();
                state.queues.iter_mut().for_each(|q| q.queue.reset());
                drop(state);

                self.unregister_ioevent();
                let mut config = self.common_config();
                config.queue_select = 0;
            }
        }

        None
    }

    pub fn remove(&self) {
        self.device().remove();
    }
}

impl<
        AS: GuestAddressSpace + Clone + 'static + Send + Sync,
        Q: QueueT + Clone + Send + Sync + 'static,
        R: 'static + GuestMemoryRegion + Send + Sync,
    > PciDevice for VirtioPciDevice<AS, Q, R>
{
    fn write_config(&self, offset: u32, data: &[u8]) {
        let reg_idx = offset >> 2;
        let reg_in_offset = offset & 0x3;
        // Handle potential write to MSI-X message control register
        assert!(self.msix_cap_reg_idx != 0);
        if self.msix_cap_reg_idx == reg_idx {
            let mut msix_state = self.msix_state();
            let mut intr_mgr = self.intr_mgr();
            debug!(
                "{:?}: set MSI-X message control, reg: ({:x?}, {:x?}), data: {:x?}",
                self.dev_id, reg_idx, reg_in_offset, data
            );
            if reg_in_offset == 2 && data.len() == 2 {
                if let Err(e) = msix_state.set_msg_ctl(LittleEndian::read_u16(data), &mut intr_mgr)
                {
                    error!("Failed to set MSI-X message control, err: {:?}", e);
                }
            } else if reg_in_offset == 0 && data.len() == 4 {
                if let Err(e) = msix_state
                    .set_msg_ctl((LittleEndian::read_u32(data) >> 16) as u16, &mut intr_mgr)
                {
                    error!("Failed to set MSI-X message control, err: {:?}", e);
                }
            }
        }

        self.configuration
            .lock()
            .unwrap()
            .write_config(offset as usize, data);
    }

    fn read_config(&self, offset: u32, data: &mut [u8]) {
        let _reg_idx = offset >> 2;
        self.configuration
            .lock()
            .unwrap()
            .read_config(offset as usize, data);
    }

    fn id(&self) -> u8 {
        self.dev_id
    }
}

impl<
        AS: GuestAddressSpace + Clone + 'static + Send + Sync,
        Q: QueueT + Clone + Send + Sync + 'static,
        R: 'static + GuestMemoryRegion + Send + Sync,
    > DeviceIo for VirtioPciDevice<AS, Q, R>
{
    fn read(&self, base: IoAddress, offset: IoAddress, data: &mut [u8]) {
        self.read_bar(base.0, offset.0, data);
    }

    fn write(&self, base: IoAddress, offset: IoAddress, data: &[u8]) {
        self.write_bar(base.0, offset.0, data);
    }

    /// Get resources assigned to the device.
    fn get_assigned_resources(&self) -> DeviceResources {
        self.device_resource.clone()
    }

    fn get_trapped_io_resources(&self) -> DeviceResources {
        let mut device = DeviceResources::new();
        device.append(self.setting_bar_res.clone());
        device
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
pub(crate) mod tests {
    #[cfg(target_arch = "aarch64")]
    use arch::aarch64::gic::create_gic;
    use dbs_device::resources::MsiIrqType;
    use dbs_interrupt::kvm::KvmIrqManager;
    use dbs_utils::epoll_manager::EpollManager;
    use kvm_ioctls::Kvm;
    use virtio_queue::QueueSync;
    use vm_memory::{GuestMemoryMmap, GuestRegionMmap, GuestUsize, MmapRegion};

    use super::*;
    use crate::PciAddress;
    use dbs_virtio_devices::ConfigResult;
    use dbs_virtio_devices::Result as VirtIoResult;
    use dbs_virtio_devices::{
        ActivateResult, VirtioDeviceConfig, VirtioDeviceInfo, VirtioSharedMemory,
        DEVICE_ACKNOWLEDGE, DEVICE_DRIVER, DEVICE_DRIVER_OK, DEVICE_FEATURES_OK, DEVICE_INIT,
    };

    use dbs_address_space::{AddressSpaceLayout, AddressSpaceRegion, AddressSpaceRegionType};
    // define macros for unit test
    const GUEST_PHYS_END: u64 = (1 << 46) - 1;
    const GUEST_MEM_START: u64 = 0;
    const GUEST_MEM_END: u64 = GUEST_PHYS_END >> 1;

    #[cfg(target_arch = "x86_64")]
    const DEVICE_STATUS_INIT: u8 = DEVICE_INIT as u8;
    #[cfg(target_arch = "x86_64")]
    const DEVICE_STATUS_ACKNOWLEDE: u8 = DEVICE_STATUS_INIT | DEVICE_ACKNOWLEDGE as u8;
    #[cfg(target_arch = "x86_64")]
    const DEVICE_STATUS_DRIVER: u8 = DEVICE_STATUS_ACKNOWLEDE | DEVICE_DRIVER as u8;
    #[cfg(target_arch = "x86_64")]
    const DEVICE_STATUS_FEATURE_OK: u8 = DEVICE_STATUS_DRIVER | DEVICE_FEATURES_OK as u8;
    #[cfg(target_arch = "x86_64")]
    const DEVICE_STATUS_DRIVER_OK: u8 = DEVICE_STATUS_FEATURE_OK | DEVICE_DRIVER_OK as u8;

    pub fn create_address_space() -> AddressSpace {
        let address_space_region = vec![Arc::new(AddressSpaceRegion::new(
            AddressSpaceRegionType::DefaultMemory,
            GuestAddress(0x0),
            0x1000 as GuestUsize,
        ))];
        let layout = AddressSpaceLayout::new(GUEST_PHYS_END, GUEST_MEM_START, GUEST_MEM_END);
        AddressSpace::from_regions(address_space_region, layout)
    }

    #[test]
    fn test_virtio_pci_cap() {
        let mut test_cap = VirtioPciCap64::new(
            PciCapabilityType::SharedMemory,
            VIRTIO_SHM_BAR_INDEX as u8,
            0,
            0x1111_1111_1111_1111,
            0x2222_2222_2222_2222,
        );

        assert_eq!(test_cap.cap.cap_id, PciCapabilityId::VendorSpecific as u8);
        assert_eq!(test_cap.len(), { std::mem::size_of::<VirtioPciCap64>() });
        let offset = std::mem::size_of::<VirtioPciCap64>();
        assert_eq!(test_cap.read_u8(offset + 1), 0xff_u8);
        assert_eq!(test_cap.read_u8(offset - 1), 0x22_u8);
        assert_eq!(test_cap.read_u8(offset - 5), 0x11_u8);
        test_cap.write_u8(offset - 5, 0x22_u8);
        assert_eq!(test_cap.read_u8(offset - 5), 0x22_u8)
    }

    pub struct DummyDevice<
        AS: GuestAddressSpace + Clone + 'static,
        Q: QueueT,
        R: GuestMemoryRegion = GuestRegionMmap,
    > {
        state: Mutex<VirtioDeviceInfo>,
        config: Mutex<Option<VirtioDeviceConfig<AS, Q, R>>>,
        shm_regions: Option<VirtioSharedMemoryList<R>>,
        device_config: [u8; 1024],
    }

    impl<AS, Q, R> DummyDevice<AS, Q, R>
    where
        AS: GuestAddressSpace + Clone,
        Q: QueueT,
        R: GuestMemoryRegion,
    {
        pub fn new() -> Self {
            let epoll_mgr = EpollManager::default();
            let state = VirtioDeviceInfo::new(
                "dummy".to_string(),
                0xf,
                Arc::new(vec![16u16, 32u16]),
                vec![11u8, 22u8, 33u8, 44u8],
                epoll_mgr,
            );
            DummyDevice {
                state: Mutex::new(state),
                config: Mutex::new(None),
                shm_regions: None,
                device_config: [0; 1024],
            }
        }
    }

    pub(crate) const DUMMY_FEATURES: u64 = 0x5555_aaaa;

    impl VirtioDevice<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap>
        for DummyDevice<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap>
    {
        fn device_type(&self) -> u32 {
            0xFF
        }

        fn queue_max_sizes(&self) -> &[u16] {
            &[16, 32]
        }

        fn get_avail_features(&self, page: u32) -> u32 {
            (DUMMY_FEATURES >> (page * 32)) as u32
        }

        fn set_acked_features(&mut self, page: u32, value: u32) {
            self.state.lock().unwrap().set_acked_features(page, value)
        }

        fn read_config(&mut self, offset: u64, data: &mut [u8]) -> ConfigResult {
            for i in 0..data.len() {
                data[i] = self.device_config[offset as usize + i];
            }

            Ok(())
        }

        fn write_config(&mut self, offset: u64, data: &[u8]) -> ConfigResult {
            for i in 0..data.len() {
                self.device_config[offset as usize + i] = data[i];
            }

            Ok(())
        }

        fn activate(&mut self, config: VirtioDeviceConfig<Arc<GuestMemoryMmap>>) -> ActivateResult {
            self.config.lock().unwrap().replace(config);
            Ok(())
        }

        fn set_resource(
            &mut self,
            _vm_fd: Arc<VmFd>,
            resource: DeviceResources,
        ) -> VirtIoResult<Option<VirtioSharedMemoryList<GuestRegionMmap>>> {
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

            let shm_regions = Some(VirtioSharedMemoryList {
                host_addr: 0x5555_aaaa,
                guest_addr: GuestAddress(0xaaaa_bbbb),
                len: 1024 * 1024 * 1024 as GuestUsize,
                kvm_userspace_memory_region_flags: 0,
                kvm_userspace_memory_region_slot: 0,
                region_list: vec![VirtioSharedMemory {
                    offset: 0,
                    len: 1024 * 1024 * 1024 as GuestUsize,
                }],
                mmap_region: Arc::new(mmap_region),
            });

            self.shm_regions = shm_regions.clone();

            Ok(shm_regions)
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

    fn get_pci_device() -> VirtioPciDevice<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap> {
        let kvm = Kvm::new().unwrap();
        let vm_fd = Arc::new(kvm.create_vm().unwrap());

        #[cfg(target_arch = "aarch64")]
        create_gic(vm_fd.as_ref(), 1).unwrap();
        #[cfg(target_arch = "x86_64")]
        vm_fd.create_irq_chip().unwrap();

        let irq_manager = Arc::new(KvmIrqManager::new(vm_fd.clone()));
        irq_manager.initialize().unwrap();

        let vm_as = Arc::new(GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x1000)]).unwrap());
        let address_space = create_address_space();

        let pci_address = PciAddress::new(0, 0, 0).unwrap();
        let dev_id = pci_address.bus_id();
        let pci_bus = Arc::new(PciBus::new(0));

        let mut device_resource = DeviceResources::new();
        device_resource.append(Resource::MsiIrq {
            ty: MsiIrqType::PciMsix,
            base: 33,
            size: 3,
        });
        let setting_bar_res = Resource::MmioAddressRange {
            base: 0x1000,
            size: CAPABILITY_BAR_SIZE,
        };
        device_resource.append(setting_bar_res.clone());

        let mut pci_dev = VirtioPciDevice::new(
            vm_fd,
            vm_as,
            address_space,
            irq_manager,
            device_resource,
            dev_id,
            Box::new(DummyDevice::new()),
            true,
            Arc::downgrade(&pci_bus),
            setting_bar_res,
        )
        .unwrap();

        pci_dev.alloc_bars().unwrap();
        let mut data = [0u8; 4];
        pci_dev
            .configuration
            .lock()
            .unwrap()
            .read_config(11, &mut data);
        assert!(pci_dev.msix_cap_reg_idx != 0);
        pci_dev
    }

    #[cfg(target_arch = "x86_64")]
    fn activate_device(d: &mut VirtioPciDevice<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap>) {
        set_driver_status(d, DEVICE_ACKNOWLEDGE as u8);
        assert_eq!(get_driver_status(d), DEVICE_STATUS_ACKNOWLEDE);
        set_driver_status(d, DEVICE_ACKNOWLEDGE as u8 | DEVICE_DRIVER as u8);
        assert_eq!(get_driver_status(d), DEVICE_STATUS_DRIVER);
        set_driver_status(
            d,
            DEVICE_ACKNOWLEDGE as u8 | DEVICE_DRIVER as u8 | DEVICE_FEATURES_OK as u8,
        );
        assert_eq!(get_driver_status(d), DEVICE_STATUS_FEATURE_OK);

        // Setup queue data structures
        let size = d.state().queues.len();
        for q in 0..size {
            let mut buf = [0; 2];
            LittleEndian::write_u16(&mut buf[..], q as u16);
            // set queue_select to q
            d.write(
                IoAddress(0),
                IoAddress(COMMON_CONFIG_BAR_OFFSET + 0x16),
                &buf[..],
            );
            // set queue_size to 16
            LittleEndian::write_u16(&mut buf[..], 16);
            d.write(
                IoAddress(0),
                IoAddress(COMMON_CONFIG_BAR_OFFSET + 0x18),
                &buf[..],
            );
            // set queue ready
            LittleEndian::write_u16(&mut buf[..], 1);
            d.write(
                IoAddress(0),
                IoAddress(COMMON_CONFIG_BAR_OFFSET + 0x1c),
                &buf[..],
            );
        }
        assert!(d.state().check_queues_valid());
        assert!(!d.device_activated.load(Ordering::SeqCst));

        set_driver_status(
            d,
            DEVICE_ACKNOWLEDGE as u8
                | DEVICE_DRIVER as u8
                | DEVICE_FEATURES_OK as u8
                | DEVICE_STATUS_DRIVER_OK,
        );
        assert_eq!(get_driver_status(d), DEVICE_STATUS_DRIVER_OK);
        assert!(d.is_driver_ready());
        assert!(d.device_activated.load(Ordering::SeqCst));
    }

    #[cfg(target_arch = "x86_64")]
    fn set_driver_status(
        d: &mut VirtioPciDevice<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap>,
        status: u8,
    ) {
        d.write(
            IoAddress(0),
            IoAddress(COMMON_CONFIG_BAR_OFFSET + 0x14),
            &[status; 1],
        )
    }

    #[cfg(target_arch = "x86_64")]
    fn get_driver_status<
        AS: GuestAddressSpace + Clone + Send + Sync + 'static,
        Q: QueueT + Send + Sync + Clone + 'static,
        R: GuestMemoryRegion + Send + Sync + 'static,
    >(
        d: &mut VirtioPciDevice<AS, Q, R>,
    ) -> u8 {
        let mut data = vec![0u8; 1];
        d.read(
            IoAddress(0),
            IoAddress(COMMON_CONFIG_BAR_OFFSET + 0x14),
            &mut data,
        );
        data[0]
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[test]
    fn test_virtio_pci_device_activate() {
        let mut d: VirtioPciDevice<_, _, _> = get_pci_device();
        assert_eq!(d.state().queues.len(), 2);
        assert!(!d.state().check_queues_valid());

        set_driver_status(&mut d, DEVICE_ACKNOWLEDGE as u8);
        assert_eq!(get_driver_status(&mut d), DEVICE_STATUS_ACKNOWLEDE);
        set_driver_status(&mut d, DEVICE_ACKNOWLEDGE as u8 | DEVICE_DRIVER as u8);
        assert_eq!(get_driver_status(&mut d), DEVICE_STATUS_DRIVER);
        set_driver_status(
            &mut d,
            DEVICE_ACKNOWLEDGE as u8 | DEVICE_DRIVER as u8 | DEVICE_FEATURES_OK as u8,
        );
        assert_eq!(get_driver_status(&mut d), DEVICE_STATUS_FEATURE_OK);

        // Setup queue data structures
        let size = d.state().queues.len();
        for q in 0..size {
            let mut buf = [0; 2];
            LittleEndian::write_u16(&mut buf[..], q as u16);
            // set queue_select to q
            d.write(
                IoAddress(0),
                IoAddress(COMMON_CONFIG_BAR_OFFSET + 0x16),
                &buf[..],
            );
            // set queue_size to 16
            LittleEndian::write_u16(&mut buf[..], 16);
            d.write(
                IoAddress(0),
                IoAddress(COMMON_CONFIG_BAR_OFFSET + 0x18),
                &buf[..],
            );
            // set queue ready
            LittleEndian::write_u16(&mut buf[..], 1);
            d.write(
                IoAddress(0),
                IoAddress(COMMON_CONFIG_BAR_OFFSET + 0x1c),
                &buf[..],
            );
        }
        assert!(d.state().check_queues_valid());
        assert!(!d.device_activated.load(Ordering::SeqCst));

        set_driver_status(
            &mut d,
            DEVICE_ACKNOWLEDGE as u8
                | DEVICE_DRIVER as u8
                | DEVICE_FEATURES_OK as u8
                | DEVICE_STATUS_DRIVER_OK,
        );
        assert_eq!(get_driver_status(&mut d), DEVICE_STATUS_DRIVER_OK);
        assert!(d.is_driver_ready());
        assert!(d.device_activated.load(Ordering::SeqCst));
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[test]
    fn test_bus_device_reset() {
        let mut d: VirtioPciDevice<_, _, _> = get_pci_device();

        assert_eq!(d.state().queues.len(), 2);
        assert!(!d.state().check_queues_valid());
        assert!(!d.is_driver_ready());
        assert_eq!(get_driver_status(&mut d), 0);

        activate_device(&mut d);
        assert!(d.is_driver_ready());
        assert!(d.device_activated.load(Ordering::SeqCst));

        // Marking device as FAILED should not affect device_activated state
        set_driver_status(&mut d, 0x8f_u8);
        assert_eq!(get_driver_status(&mut d), 0x8f);
        assert!(d.device_activated.load(Ordering::SeqCst));

        // Backend driver doesn't support reset
        set_driver_status(&mut d, 0x0_u8);
        assert_eq!(get_driver_status(&mut d), DEVICE_FAILED as u8);
        assert!(d.device_activated.load(Ordering::SeqCst));
    }

    #[test]
    fn test_virtio_pci_device_resources() {
        let d: VirtioPciDevice<_, _, _> = get_pci_device();

        let resources = d.get_assigned_resources();
        assert_eq!(resources.len(), 2);
        let pci_msix_irqs = resources.get_pci_msix_irqs();
        assert!(pci_msix_irqs.is_some());
        assert_eq!(pci_msix_irqs.unwrap(), (33, 3));

        let resources = d.get_trapped_io_resources();
        assert_eq!(resources.len(), 1);
        let cap_bar_res = resources.get_mmio_address_ranges();
        assert_eq!(cap_bar_res.len(), 1);
        assert_eq!(cap_bar_res[0].1, CAPABILITY_BAR_SIZE);
    }

    #[test]
    fn test_virtio_pci_register_ioevent() {
        let d: VirtioPciDevice<_, _, _> = get_pci_device();
        d.register_ioevent().unwrap();
        assert!(d.ioevent_registered.load(Ordering::SeqCst));
        d.unregister_ioevent();
        assert!(!d.ioevent_registered.load(Ordering::SeqCst));
        d.register_ioevent().unwrap();
        assert!(d.ioevent_registered.load(Ordering::SeqCst));
    }

    #[test]
    fn test_get_interrupt_requirements() {
        let device = Box::new(DummyDevice::new());
        let mut req = vec![];
        VirtioPciDevice::get_interrupt_requirements(device.as_ref(), &mut req);
        assert_eq!(req.len(), 1);
        // 2 quue + 2 aux notification + 1 config
        assert!(matches!(req[0], ResourceConstraint::PciMsixIrq{ size } if size == 2 + 1));
    }

    #[test]
    fn test_read_bar() {
        let d: VirtioPciDevice<_, _, _> = get_pci_device();
        let origin_data = vec![1u8];
        // driver status
        d.write_bar(0, COMMON_CONFIG_BAR_OFFSET + 0x14, &origin_data);
        let mut new_data = vec![0u8];
        d.read_bar(0, COMMON_CONFIG_BAR_OFFSET + 0x14, &mut new_data);
        assert_eq!(origin_data, new_data);

        let origin_data = vec![1, 2, 3, 4];
        d.write_bar(0, DEVICE_CONFIG_BAR_OFFSET, &origin_data);
        let mut new_data = vec![0, 0, 0, 0];
        d.arc_device()
            .lock()
            .unwrap()
            .read_config(0, &mut new_data)
            .unwrap();
        assert_eq!(origin_data, new_data);
    }
}
