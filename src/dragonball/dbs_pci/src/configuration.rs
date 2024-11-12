// Copyright (C) 2023 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0
// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.

//! Simulate PCI device configuration header and manage PCI BAR configuration.
//!
//! The PCI Specification defines the organization of the 256-byte Configuration Space registers
//! and imposes a specific template for the space. All PCI compliant devices must support the
//! Vendor ID, Device ID, Command and Status, Revision ID, Class Code and Header Type fields.
//! Implementation of the other registers is optional, depending upon the devices functionality.
//!
//! Note:
//! - only ready for little endian platforms.
//! - no support for PCIe 4K configuration header.
//! - no support for legacy IRQ, only supports MSI/MSI-x.
//! - no support for COMMAND register, so `interrupt disable`, `memory space`, `i/o space`
//!   control bits don't work as expected yet.
//!

use std::ops::BitAnd;
use std::sync::{Arc, Mutex, Weak};

use byteorder::{ByteOrder, LittleEndian};
use dbs_device::resources::{DeviceResources, Resource, ResourceConstraint};
use log::{debug, warn};

use crate::{Error, PciBus, Result};

/// The number of 32bit registers in the config space, 256 bytes.
pub const NUM_CONFIGURATION_REGISTERS: usize = 64;
/// Number of PCI BAR registers.
pub const NUM_BAR_REGS: usize = 6;

const STATUS_REG: usize = 1;
const BAR0_REG: usize = 4;
const ROM_BAR_REG: usize = 12;
const CAPABILITY_LIST_HEAD_REG: usize = 13;
const INTERRUPT_LINE_PIN_REG: usize = 15;

const STATUS_REG_CAPABILITIES_USED_MASK: u32 = 0x0010_0000;
const BAR_IO_ADDR_MASK: u32 = 0xffff_fffc;
const BAR_MEM_ADDR_MASK: u32 = 0xffff_fff0;
const ROM_BAR_ADDR_MASK: u32 = 0xffff_f800;
const FIRST_CAPABILITY_OFFSET: usize = 0x40;
const CAPABILITY_MAX_OFFSET: usize = 0xc0;

type Capabilities = Vec<(usize, usize, Arc<Mutex<Box<dyn PciCapability>>>)>;

/// Represents the types of PCI headers allowed in the configuration registers.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum PciHeaderType {
    /// Normal PCI device
    Device = 0x0,
    /// PCI to PCI bridge
    Bridge = 0x1,
    /// PCI-to-CardBus bridge
    CardBus = 0x2,
}

/// Classes of PCI device.
#[derive(Copy, Clone)]
pub enum PciClassCode {
    /// Unclassified
    TooOld = 0x00,
    MassStorage = 0x01,
    NetworkController = 0x02,
    DisplayController = 0x03,
    MultimediaController = 0x04,
    MemoryController = 0x05,
    BridgeDevice = 0x06,
    SimpleCommunicationController = 0x07,
    BaseSystemPeripheral = 0x08,
    InputDevice = 0x09,
    DockingStation = 0x0a,
    Processor = 0x0b,
    SerialBusController = 0x0c,
    WirelessController = 0x0d,
    IntelligentIoController = 0x0e,
    SatelliteCommunicationController = 0x0f,
    EncryptionController = 0x10,
    DataAcquisitionSignalProcessing = 0x11,
    Other = 0xff,
}

impl PciClassCode {
    /// Get the PCI class code as an `u8`.
    pub fn get_register_value(self) -> u8 {
        self as u8
    }
}

/// A PCI sublcass. Each class in `PciClassCode` can specify a unique set of subclasses. This trait
/// is implemented by each subclass. It allows use of a trait object to generate configurations.
pub trait PciSubclass {
    /// Convert this subclass to the value used in the PCI specification.
    fn get_register_value(&self) -> u8;
}

/// Subclasses of the MultimediaController class.
#[derive(Copy, Clone)]
pub enum PciMultimediaSubclass {
    /// Multimedia Video Controller
    VideoController = 0x00,
    /// Multimedia Audio Controller
    AudioController = 0x01,
    /// Computer Telephony Device
    TelephonyDevice = 0x02,
    /// Audio Device
    AudioDevice = 0x03,
    /// Other Multimedia Device defined by vendor
    Other = 0x80,
}

impl PciSubclass for PciMultimediaSubclass {
    fn get_register_value(&self) -> u8 {
        *self as u8
    }
}

/// Subclasses of the BridgeDevice
#[derive(Copy, Clone)]
pub enum PciBridgeSubclass {
    HostBridge = 0x00,
    IsaBridge = 0x01,
    EisaBridge = 0x02,
    McaBridge = 0x03,
    PciToPciBridge = 0x04,
    PcmciaBridge = 0x05,
    NuBusBridge = 0x06,
    CardBusBridge = 0x07,
    RACEwayBridge = 0x08,
    PciToPciSemiTransparentBridge = 0x09,
    InfiniBrandToPciHostBridge = 0x0a,
    OtherBridgeDevice = 0x80,
}

impl PciSubclass for PciBridgeSubclass {
    fn get_register_value(&self) -> u8 {
        *self as u8
    }
}

/// Subclass of the SerialBus
#[derive(Copy, Clone)]
pub enum PciSerialBusSubClass {
    Firewire = 0x00,
    ACCESSbus = 0x01,
    SSA = 0x02,
    USB = 0x03,
}

impl PciSubclass for PciSerialBusSubClass {
    fn get_register_value(&self) -> u8 {
        *self as u8
    }
}

/// Mass Storage Sub Classes
#[derive(Copy, Clone)]
pub enum PciMassStorageSubclass {
    SCSIStorage = 0x00,
    IDEInterface = 0x01,
    FloppyController = 0x02,
    IPIController = 0x03,
    RAIDController = 0x04,
    ATAController = 0x05,
    SATAController = 0x06,
    SerialSCSIController = 0x07,
    NVMController = 0x08,
    MassStorage = 0x80,
}

impl PciSubclass for PciMassStorageSubclass {
    fn get_register_value(&self) -> u8 {
        *self as u8
    }
}

/// Network Controller Sub Classes
#[derive(Copy, Clone)]
pub enum PciNetworkControllerSubclass {
    EthernetController = 0x00,
    TokenRingController = 0x01,
    FDDIController = 0x02,
    ATMController = 0x03,
    ISDNController = 0x04,
    WorldFipController = 0x05,
    PICMGController = 0x06,
    InfinibandController = 0x07,
    FabricController = 0x08,
    NetworkController = 0x80,
}

impl PciSubclass for PciNetworkControllerSubclass {
    fn get_register_value(&self) -> u8 {
        *self as u8
    }
}

/// A PCI class programming interface. Each combination of `PciClassCode` and
/// `PciSubclass` can specify a set of register-level programming interfaces.
/// This trait is implemented by each programming interface.
/// It allows use of a trait object to generate configurations.
pub trait PciProgrammingInterface {
    /// Convert this programming interface to the value used in the PCI specification.
    fn get_register_value(&self) -> u8;
}

/// Types of PCI capabilities.
#[repr(u8)]
#[derive(PartialEq, Copy, Clone)]
pub enum PciCapabilityId {
    ListID = 0,
    PowerManagement = 0x01,
    AcceleratedGraphicsPort = 0x02,
    VitalProductData = 0x03,
    SlotIdentification = 0x04,
    MessageSignalledInterrupts = 0x05,
    CompactPCIHotSwap = 0x06,
    PCIX = 0x07,
    HyperTransport = 0x08,
    VendorSpecific = 0x09,
    Debugport = 0x0A,
    CompactPCICentralResourceControl = 0x0B,
    PCIStandardHotPlugController = 0x0C,
    BridgeSubsystemVendorDeviceID = 0x0D,
    AGPTargetPCIPCIbridge = 0x0E,
    SecureDevice = 0x0F,
    PCIExpress = 0x10,
    MSIX = 0x11,
    SATADataIndexConf = 0x12,
    PCIAdvancedFeatures = 0x13,
    PCIEnhancedAllocation = 0x14,
    #[cfg(test)]
    Test = 0xFF,
}

impl From<u8> for PciCapabilityId {
    fn from(c: u8) -> Self {
        match c {
            0 => PciCapabilityId::ListID,
            0x01 => PciCapabilityId::PowerManagement,
            0x02 => PciCapabilityId::AcceleratedGraphicsPort,
            0x03 => PciCapabilityId::VitalProductData,
            0x04 => PciCapabilityId::SlotIdentification,
            0x05 => PciCapabilityId::MessageSignalledInterrupts,
            0x06 => PciCapabilityId::CompactPCIHotSwap,
            0x07 => PciCapabilityId::PCIX,
            0x08 => PciCapabilityId::HyperTransport,
            0x09 => PciCapabilityId::VendorSpecific,
            0x0A => PciCapabilityId::Debugport,
            0x0B => PciCapabilityId::CompactPCICentralResourceControl,
            0x0C => PciCapabilityId::PCIStandardHotPlugController,
            0x0D => PciCapabilityId::BridgeSubsystemVendorDeviceID,
            0x0E => PciCapabilityId::AGPTargetPCIPCIbridge,
            0x0F => PciCapabilityId::SecureDevice,
            0x10 => PciCapabilityId::PCIExpress,
            0x11 => PciCapabilityId::MSIX,
            0x12 => PciCapabilityId::SATADataIndexConf,
            0x13 => PciCapabilityId::PCIAdvancedFeatures,
            0x14 => PciCapabilityId::PCIEnhancedAllocation,
            #[cfg(test)]
            0xFF => PciCapabilityId::Test,
            _ => PciCapabilityId::ListID,
        }
    }
}

/// A PCI capability list.
///
/// Devices can optionally specify capabilities in their configuration space.
#[allow(clippy::len_without_is_empty)]
pub trait PciCapability: Send + Sync {
    /// Get size of the whole capability structure, including the capability header.
    fn len(&self) -> usize;

    /// Link to next PCI capability.
    fn set_next_cap(&mut self, next: u8);

    /// Read a 8bit value from the capability.
    fn read_u8(&mut self, offset: usize) -> u8;

    /// Read a 16bit value from the capability.
    fn read_u16(&mut self, offset: usize) -> u16 {
        self.read_u8(offset) as u16 | (self.read_u8(offset + 1) as u16) << 8
    }

    /// Read a 32bit value from the capability.
    fn read_u32(&mut self, offset: usize) -> u32 {
        self.read_u16(offset) as u32 | (self.read_u16(offset + 2) as u32) << 16
    }

    /// Write a 8bit value to the capability.
    fn write_u8(&mut self, offset: usize, value: u8);

    /// Write a 16bit value to the capability.
    fn write_u16(&mut self, offset: usize, value: u16) {
        self.write_u8(offset, value as u8);
        self.write_u8(offset + 1, (value >> 8) as u8);
    }

    /// Write a 32bit value to the capability.
    fn write_u32(&mut self, offset: usize, value: u32) {
        self.write_u16(offset, value as u16);
        self.write_u16(offset + 2, (value >> 16) as u16);
    }

    /// The type of PCI Interrupt
    fn pci_capability_type(&self) -> PciCapabilityId;
}

/// PCI device has four interrupt pins A->D.
#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum PciInterruptPin {
    IntA = 1,
    IntB = 2,
    IntC = 3,
    IntD = 4,
}

impl PciInterruptPin {
    fn to_mask(self) -> u32 {
        self as u32
    }
}

/// Type of PCI Bars defined by the PCI specification.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PciBarRegionType {
    /// 32-bit MMIO Bar.
    Memory32BitRegion = 0,
    /// IO port Bar.
    IoRegion = 0x01,
    /// 64-bit MMIO Bar.
    Memory64BitRegion = 0x04,
    /// Fake type for the upper Bar of 64-bit MMIO Bar.
    Memory64BitRegionUpper = 0x80,
}

/// Flag indicating whether the Bar content is prefetchable.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum PciBarPrefetchable {
    /// Content is non-prefetchable.
    #[default]
    NotPrefetchable = 0,
    /// Content is prefetchable.
    Prefetchable = 0x08,
}

/// Configuration information for a PCI Bar.
#[derive(Copy, Clone, Debug)]
pub struct PciBarConfiguration {
    /// PCI Bar index.
    bar_idx: usize,
    /// Type of the Bar.
    bar_type: PciBarRegionType,
    /// Flag for content prefetch.
    prefetchable: PciBarPrefetchable,
    /// Base address of the Bar window.
    addr: u64,
    /// Size of the Bar window.
    size: u64,
}

impl PciBarConfiguration {
    /// Create a new PCI Bar Configuration object.
    pub fn new(
        bar_idx: usize,
        size: u64,
        bar_type: PciBarRegionType,
        prefetchable: PciBarPrefetchable,
    ) -> Self {
        PciBarConfiguration {
            bar_idx,
            bar_type,
            prefetchable,
            addr: 0,
            size,
        }
    }

    /// Get the type of the PCI Bar.
    pub fn bar_type(&self) -> PciBarRegionType {
        self.bar_type
    }

    /// Get the register index of the PCI Bar.
    pub fn bar_index(&self) -> usize {
        self.bar_idx
    }

    /// Get the base address of the PCI Bar window.
    pub fn address(&self) -> u64 {
        self.addr
    }

    /// Get the size of the PCI Bar Window.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Check whether the Bar content is prefetachable.
    pub fn prefetchable(&self) -> bool {
        self.prefetchable == PciBarPrefetchable::Prefetchable
    }

    /// Set the type of the PCI Bar.
    pub fn set_bar_type(mut self, bar_type: PciBarRegionType) -> Self {
        assert_ne!(bar_type, PciBarRegionType::Memory64BitRegionUpper);
        self.bar_type = bar_type;
        self
    }

    /// Check whether the Bar content is prefetachable.
    pub fn set_prefetchable(mut self, prefetchable: PciBarPrefetchable) -> Self {
        self.prefetchable = prefetchable;
        self
    }

    /// Set the register index of the PCI Bar.
    pub fn set_bar_index(mut self, bar_idx: usize) -> Self {
        assert!(bar_idx <= NUM_BAR_REGS);
        self.bar_idx = bar_idx;
        self
    }

    /// Set the base address of the PCI Bar window.
    pub fn set_address(mut self, addr: u64) -> Self {
        self.addr = addr;
        self
    }

    /// Set the size of the PCI Bar window.
    pub fn set_size(mut self, size: u64) -> Self {
        self.size = size;
        self
    }
}

impl Default for PciBarConfiguration {
    fn default() -> Self {
        PciBarConfiguration {
            bar_idx: 0,
            bar_type: PciBarRegionType::Memory64BitRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0,
            size: 0,
        }
    }
}

/// Struct to share PCI BAR programming information.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarProgrammingParams {
    pub bar_idx: usize,
    pub bar_type: PciBarRegionType,
    pub old_base: u64,
    pub new_base: u64,
    pub len: u64,
}

impl BarProgrammingParams {
    pub fn to_resources(&self, old: bool) -> DeviceResources {
        let base = if old { self.old_base } else { self.new_base };

        let mut resources = DeviceResources::new();
        match self.bar_type {
            PciBarRegionType::IoRegion => resources.append(Resource::PioAddressRange {
                base: base as u16,
                size: self.len as u16,
            }),
            PciBarRegionType::Memory32BitRegion | PciBarRegionType::Memory64BitRegion => resources
                .append(Resource::MmioAddressRange {
                    base,
                    size: self.len,
                }),
            _ => panic!("invalid PCI BAR type"),
        }

        resources
    }
}

#[derive(Default, PartialEq, Clone, Debug, Copy)]
pub struct PciBarState {
    addr: u32,
    size: u32,
    type_: Option<PciBarRegionType>,
    allocated: bool,
}

impl PciBarState {
    fn set(&mut self, addr: u32, size: u32, type_: PciBarRegionType, allocated: bool) {
        self.addr = addr;
        self.size = size;
        self.type_ = Some(type_);
        self.allocated = allocated;
    }

    fn mask(&self) -> u32 {
        match self.type_ {
            None => 0,
            Some(PciBarRegionType::IoRegion) => !(self.size - 1),
            Some(PciBarRegionType::Memory32BitRegion) => !(self.size - 1),
            Some(PciBarRegionType::Memory64BitRegion) => {
                if self.size == 0 {
                    0
                } else {
                    !(self.size - 1)
                }
            }
            Some(PciBarRegionType::Memory64BitRegionUpper) => {
                if self.size == 0 {
                    0xffff_ffff
                } else {
                    !(self.size - 1)
                }
            }
        }
    }
}

/*
 * Nvidia GPU Device reserve C8h PCI Express Virtual P2P Approval Capability.
 * +----------------+----------------+----------------+----------------+
 * | sig 7:0 ('P')  |  vndr len (8h) |    next (0h)   |   cap id (9h)  |
 * +----------------+----------------+----------------+----------------+
 *  * | rsvd 15:7(0h),id 6:3,ver 2:0(0h)|          sig 23:8 ('P2')        |
 * +---------------------------------+---------------------------------+
 */
#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[allow(dead_code)]
pub struct Vp2pCap {
    id: u8,
    next: u8,
    length: u8,
    sig_1: u8,
    sig_2: u8,
    sig_3: u8,
    clique_id: u16,
}

impl Vp2pCap {
    fn new(clique_id: u8) -> Self {
        Vp2pCap {
            id: PciCapabilityId::VendorSpecific as u8,
            next: 0,
            length: 8,
            sig_1: 0x50,
            sig_2: 0x32,
            sig_3: 0x50,
            clique_id: ((clique_id & 0xf) << 3) as u16,
        }
    }
}

impl PciCapability for Vp2pCap {
    fn len(&self) -> usize {
        8
    }

    fn set_next_cap(&mut self, next: u8) {
        self.next = next;
    }

    fn read_u8(&mut self, offset: usize) -> u8 {
        match offset {
            0 => self.id,
            1 => self.next,
            2 => self.length,
            3 => self.sig_1,
            4 => self.sig_2,
            5 => self.sig_3,
            6 => (self.clique_id & 0xff) as u8,
            _ => 0x0,
        }
    }

    fn write_u8(&mut self, _offset: usize, _value: u8) {}

    fn pci_capability_type(&self) -> PciCapabilityId {
        PciCapabilityId::VendorSpecific
    }
}

/// Manage and handle access to the configuration space of a PCI node.
///
/// All PCI compliant devices must support the Vendor ID, Device ID, Command and Status, Revision
/// ID, Class Code and Header Type fields. Implementation of the other registers is optional,
/// depending upon the devices functionality.
/// See the [specification](https://en.wikipedia.org/wiki/PCI_configuration_space).
pub struct PciConfiguration {
    header_type: PciHeaderType,
    registers: [u32; NUM_CONFIGURATION_REGISTERS],
    writable_bits: [u32; NUM_CONFIGURATION_REGISTERS], // writable bits for each register.
    bars: [PciBarState; NUM_BAR_REGS + 1],             // one extra entry for ROM BAR.
    bar_programming_params: Option<BarProgrammingParams>,
    capabilities: Capabilities,
    bus: Weak<PciBus>,
}

impl PciConfiguration {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bus: Weak<PciBus>,
        vendor_id: u16,
        device_id: u16,
        class_code: PciClassCode,
        subclass: &dyn PciSubclass,
        programming_interface: Option<&dyn PciProgrammingInterface>,
        header_type: PciHeaderType,
        subsystem_vendor_id: u16,
        subsystem_id: u16,
        clique_id: Option<u8>,
    ) -> Result<Self> {
        // Only supports normal PCI devices, no P2P bridge or Cardbus yet.
        assert!(header_type == PciHeaderType::Device);

        let mut registers = [0u32; NUM_CONFIGURATION_REGISTERS];
        let mut writable_bits = [0u32; NUM_CONFIGURATION_REGISTERS];
        registers[0] = u32::from(device_id) << 16 | u32::from(vendor_id);
        writable_bits[1] = 0x0000_ffff; // Status (r/o), command (r/w)
        let pi = if let Some(pi) = programming_interface {
            pi.get_register_value()
        } else {
            0
        };
        registers[2] = u32::from(class_code.get_register_value()) << 24
            | u32::from(subclass.get_register_value()) << 16
            | u32::from(pi) << 8;
        writable_bits[3] = 0x0000_00ff; // Cacheline size (r/w)
        registers[3] = 0x0000_0000; // Header type 0 (device)
        registers[11] = u32::from(subsystem_id) << 16 | u32::from(subsystem_vendor_id);
        writable_bits[15] = 0x0000_00ff; // Interrupt line (r/w)

        let mut configuration = PciConfiguration {
            header_type,
            registers,
            writable_bits,
            bars: Default::default(),
            bar_programming_params: None,
            capabilities: Vec::new(),
            bus,
        };
        if let Some(id) = clique_id {
            let vp2p_cap = Box::new(Vp2pCap::new(id));
            configuration.add_capability(Arc::new(Mutex::new(vp2p_cap)))?;
        }

        Ok(configuration)
    }
    /// Configures the IRQ line and pin used by this device.
    pub fn set_irq(&mut self, line: u8, pin: PciInterruptPin) {
        // `pin` is 1-based in the pci config space.
        let pin_idx = pin.to_mask();
        self.registers[INTERRUPT_LINE_PIN_REG] &= 0xffff_0000;
        self.registers[INTERRUPT_LINE_PIN_REG] |= (pin_idx << 8) | u32::from(line);
    }
}

impl PciConfiguration {
    /// Read data from the configuration space.
    ///
    /// Only support naturally aligned Byte, Word and Dword accesses, otherwise return `data`
    /// with all bits set.
    pub fn read_config(&self, offset: usize, data: &mut [u8]) {
        if offset.trailing_zeros() >= 2 && data.len() == 4 {
            LittleEndian::write_u32(data, self.read_u32(offset).1);
        } else if offset.trailing_zeros() >= 1 && data.len() == 2 {
            LittleEndian::write_u16(data, self.read_u16(offset).1);
        } else if data.len() == 1 {
            data[0] = self.read_u8(offset).1;
        } else {
            for pos in data {
                *pos = 0xff;
            }
        }
    }

    /// Write data into the configuration space.
    ///
    /// Only support naturally aligned Byte, Word and Dword accesses, otherwise silently drops
    /// the write operation.
    pub fn write_config(&mut self, offset: usize, data: &[u8]) {
        if offset.trailing_zeros() >= 2 && data.len() == 4 {
            self.write_u32(offset, LittleEndian::read_u32(data));
        } else if offset.trailing_zeros() >= 1 && data.len() == 2 {
            self.write_u16(offset, LittleEndian::read_u16(data));
        } else if data.len() == 1 {
            self.write_u8(offset, data[0]);
        }
    }

    /// Read a 32bit value from naturally aligned `offset` in the register map.
    ///
    /// Return a bool flag to indicate whether the register is a well known PCI register and has
    /// been handled by the framework.
    pub fn read_u32(&self, offset: usize) -> (bool, u32) {
        let reg_idx = offset >> 2;
        if (offset & 0x3) != 0 || offset >= 256 {
            warn!("configuration read_u32 offset invalid: 0x{:x}", offset);
            return (false, 0xffff_ffff);
        }

        // Handle common registers 0-3 for all PCI devices.
        if reg_idx < 4 {
            return (true, self.registers[reg_idx]);
        }

        // Handle registers for normal PCI devices.
        if self.header_type == PciHeaderType::Device && reg_idx < 16 {
            return (true, self.registers[reg_idx]);
        }

        // Handle PCI Capabilities
        if let (true, value) = self.cap_read_u32(offset) {
            return (true, value);
        }

        (false, self.registers[reg_idx])
    }

    /// Read a 16bit value from naturally aligned `offset` in the register map.
    ///
    /// Return a bool flag to indicate whether the register is a well known PCI register and has
    /// been handled by the framework.
    pub fn read_u16(&self, offset: usize) -> (bool, u16) {
        if (offset & 0x1) != 0 || offset >= 256 {
            warn!("configuration read_u16 offset invalid: 0x{:x}", offset);
            return (false, 0xffff);
        }

        // Handle PCI Capabilities first
        if let (true, value) = self.cap_read_u16(offset) {
            return (true, value);
        }

        let (handled, value) = self.read_u32(offset & !0x3);
        (handled, (value >> ((offset as u32 & 0x2) * 8)) as u16)
    }

    /// Read a 8bit value from `offset` in the register map.
    ///
    /// Return a bool flag to indicate whether the register is a well known PCI register and has
    /// been handled by the framework.
    pub fn read_u8(&self, offset: usize) -> (bool, u8) {
        if offset >= 256 {
            warn!("configuration read_8 offset invalid: 0x{:x}", offset);
            return (false, 0xff);
        }

        // Handle PCI Capabilities first
        if let (true, value) = self.cap_read_u8(offset) {
            return (true, value);
        }

        let (handled, value) = self.read_u16(offset & !0x1);
        (handled, (value >> ((offset as u16 & 0x1) * 8)) as u8)
    }

    /// Write a 32bit value to naturally aligned `offset` in the register map.
    ///
    /// Return a bool flag to indicate whether the register is a well known PCI register and has
    /// been handled by the framework.
    pub fn write_u32(&mut self, offset: usize, value: u32) -> bool {
        let reg_idx = offset >> 2;
        let mask = self.writable_bits[reg_idx];
        if (offset & 0x3) != 0 || offset >= 256 {
            warn!("configuration write_u32 offset invalid: 0x{:x}", offset);
            return false;
        }

        if reg_idx == 0x0 || reg_idx == 0x2 {
            // DeviceId, VendorId, class/subclass/prog if/rev id are readonly.
            return true;
        } else if reg_idx == 1 || reg_idx == 3 {
            self.registers[reg_idx] &= !mask;
            self.registers[reg_idx] |= value & mask;
            return true;
        }

        // Handle registers for normal PCI devices.
        if self.header_type == PciHeaderType::Device && reg_idx < 16 {
            if (BAR0_REG..BAR0_REG + NUM_BAR_REGS).contains(&reg_idx) || reg_idx == ROM_BAR_REG {
                return self.write_device_bar_reg(reg_idx, value);
            } else if reg_idx == 0xa || reg_idx == 0xb || reg_idx == 0xd || reg_idx == 0xe {
                // CIS, Subsystem, subvendor, capability pointer and reserved are readonly
                return true;
            } else if reg_idx == 0xf {
                // Interrupt registers
                self.registers[reg_idx] &= !mask;
                self.registers[reg_idx] |= value & mask;
                return true;
            }
        }

        // Handle PCI Capabilities
        if self.cap_write_u32(offset, value) {
            return true;
        }

        self.registers[reg_idx] &= !mask;
        self.registers[reg_idx] |= value & mask;

        false
    }

    /// Write a 16bit value to naturally aligned `offset` in the register map.
    ///
    /// Return a bool flag to indicate whether the register is a well known PCI register and has
    /// been handled by the framework.
    pub fn write_u16(&mut self, offset: usize, value: u16) -> bool {
        if (offset & 0x1) != 0 || offset >= 256 {
            warn!("configuration write_u16 offset invalid: 0x{:x}", offset);
            return false;
        }

        if self.cap_write_u16(offset, value) {
            return true;
        }

        let (_, mut old) = self.read_u32(offset & !0x3);
        let mask = 0xffffu32 << ((offset as u32 & 0x2) << 3);
        old &= !mask;
        old |= u32::from(value) & mask;
        self.write_u32(offset & !0x3, old)
    }

    /// Write a 8bit value to `offset` in the register map.
    ///
    /// Return a bool flag to indicate whether the register is a well known PCI register and has
    /// been handled by the framework.
    pub fn write_u8(&mut self, offset: usize, value: u8) -> bool {
        if offset >= 256 {
            return false;
        }

        if self.cap_write_u8(offset, value) {
            return true;
        }

        let (_, mut old) = self.read_u32(offset & !0x3);
        let mask = 0xffu32 << ((offset as u32 & 0x3) << 3);
        old &= !mask;
        old |= u32::from(value) & mask;
        self.write_u32(offset & !0x3, old)
    }
}

// Manage PCI/PCIe capabilities.
impl PciConfiguration {
    /// Adds the PCI capability `cap_data` to the device's list of capabilities.
    pub fn add_capability(&mut self, cap: Arc<Mutex<Box<dyn PciCapability>>>) -> Result<usize> {
        // Don't expect poisoned lock.
        let mut cap_data = cap.lock().unwrap();
        let total_len = cap_data.len();
        if total_len <= 2 {
            return Err(Error::CapabilityEmpty);
        }

        let cap_offset = match self.capabilities.len() {
            0 => FIRST_CAPABILITY_OFFSET,
            sz => {
                let offset = self.capabilities[sz - 1].0;
                let len = self.capabilities[sz - 1].1;
                Self::next_dword(offset, len)
            }
        };
        let end_offset = cap_offset
            .checked_add(total_len)
            .ok_or(Error::CapabilitySpaceFull(total_len))?;
        if end_offset > CAPABILITY_MAX_OFFSET {
            return Err(Error::CapabilitySpaceFull(total_len));
        }

        self.registers[STATUS_REG] |= STATUS_REG_CAPABILITIES_USED_MASK;
        let value = self.registers[CAPABILITY_LIST_HEAD_REG];
        self.registers[CAPABILITY_LIST_HEAD_REG] &= !0xff;
        self.registers[CAPABILITY_LIST_HEAD_REG] |= cap_offset as u32;
        cap_data.set_next_cap(value as u8);
        drop(cap_data);
        self.capabilities.push((cap_offset, total_len, cap));

        Ok(cap_offset)
    }

    fn cap_read_u8(&self, offset: usize) -> (bool, u8) {
        for (base, len, cap) in self.capabilities.iter().as_ref() {
            if *base <= offset && offset < *base + *len {
                // Don't expect poisoned lock.
                let mut cap_data = cap.lock().unwrap();
                return (true, cap_data.read_u8(offset - *base));
            }
        }
        (false, 0xff)
    }

    // Caller needs to ensure offset is naturally aligned on WORD.
    fn cap_read_u16(&self, offset: usize) -> (bool, u16) {
        for (base, len, cap) in self.capabilities.iter().as_ref() {
            if *base <= offset && offset < *base + *len {
                // Don't expect poisoned lock.
                let mut cap_data = cap.lock().unwrap();
                return (true, cap_data.read_u16(offset - *base));
            }
        }
        (false, 0xffff)
    }

    // Caller needs to ensure offset is naturally aligned on DWORD.
    fn cap_read_u32(&self, offset: usize) -> (bool, u32) {
        for (base, len, cap) in self.capabilities.iter().as_ref() {
            if *base <= offset && offset < *base + *len {
                // Don't expect poisoned lock.
                let mut cap_data = cap.lock().unwrap();
                return (true, cap_data.read_u32(offset - *base));
            }
        }
        (false, 0xffff_ffff)
    }

    fn cap_write_u8(&self, offset: usize, value: u8) -> bool {
        for (base, len, cap) in self.capabilities.iter().as_ref() {
            if *base <= offset && offset < *base + *len {
                // Don't expect poisoned lock.
                let mut cap_data = cap.lock().unwrap();
                cap_data.write_u8(offset - *base, value);
                return true;
            }
        }
        false
    }

    // Caller needs to ensure offset is naturally aligned on WORD.
    fn cap_write_u16(&self, offset: usize, value: u16) -> bool {
        for (base, len, cap) in self.capabilities.iter().as_ref() {
            if *base <= offset && offset < *base + *len {
                // Don't expect poisoned lock.
                let mut cap_data = cap.lock().unwrap();
                cap_data.write_u16(offset - *base, value);
                return true;
            }
        }
        false
    }

    // Caller needs to ensure offset is naturally aligned on DWORD.
    fn cap_write_u32(&self, offset: usize, value: u32) -> bool {
        for (base, len, cap) in self.capabilities.iter().as_ref() {
            if *base <= offset && offset < *base + *len {
                // Don't expect poisoned lock.
                let mut cap_data = cap.lock().unwrap();
                cap_data.write_u32(offset - *base, value);
                return true;
            }
        }
        false
    }

    // Naturally align the PCI capability to the next Dword. This helps to avoid mixing two
    // PCI capabilities on the same configuration register.
    fn next_dword(offset: usize, len: usize) -> usize {
        let next = offset + len;
        (next + 3) & !3
    }
}

// Methods to manage PCI BAR and ROM BAR.
// This is the common part for vfio-pci device passthrough and PCI device emulation.
impl PciConfiguration {
    /// Adds a BAR region specified by `config` for normal PCI devices.
    ///
    /// Configures the specified BAR(s) to report this region and size to the guest kernel.
    /// Enforces a few constraints (i.e, region size must be power of two, register not already
    /// used).
    /// Returns 'None' on failure all, `Some(BarIndex)` on success.
    pub fn add_device_bar(&mut self, config: &PciBarConfiguration) -> Result<usize> {
        if self.header_type != PciHeaderType::Device {
            return Err(Error::BarInvalid(config.bar_idx));
        }
        if config.bar_idx >= NUM_BAR_REGS {
            return Err(Error::BarInvalid(config.bar_idx));
        }
        if self.bar_used(config.bar_idx) {
            return Err(Error::BarInUse(config.bar_idx));
        }
        if config.size.count_ones() != 1 {
            return Err(Error::BarSizeInvalid(config.size));
        }

        let reg_idx = BAR0_REG + config.bar_idx;
        let end_addr = config
            .addr
            .checked_add(config.size - 1)
            .ok_or(Error::BarAddressInvalid(config.addr, config.size))?;
        match config.bar_type {
            PciBarRegionType::IoRegion => {
                if config.size < 0x4 || config.size > u64::from(u32::max_value()) {
                    return Err(Error::BarSizeInvalid(config.size));
                }
                if end_addr > u64::from(u32::max_value()) {
                    return Err(Error::BarAddressInvalid(config.addr, config.size));
                }
            }
            PciBarRegionType::Memory32BitRegion => {
                if config.size < 0x10 || config.size > u64::from(u32::max_value()) {
                    return Err(Error::BarSizeInvalid(config.size));
                }
                if end_addr > u64::from(u32::max_value()) {
                    return Err(Error::BarAddressInvalid(config.addr, config.size));
                }
            }
            PciBarRegionType::Memory64BitRegion => {
                if config.bar_idx + 1 >= NUM_BAR_REGS {
                    return Err(Error::BarInvalid64(config.bar_idx));
                }
                if self.bar_used(config.bar_idx + 1) {
                    return Err(Error::BarInUse64(config.bar_idx));
                }
                if end_addr > u64::max_value() {
                    return Err(Error::BarAddressInvalid(config.addr, config.size));
                }

                self.registers[reg_idx + 1] = (config.addr >> 32) as u32;
                self.writable_bits[reg_idx + 1] = 0xffff_ffff;
                self.bars[config.bar_idx + 1].set(
                    self.registers[reg_idx + 1],
                    (config.size >> 32) as u32,
                    PciBarRegionType::Memory64BitRegionUpper,
                    false,
                );
            }
            PciBarRegionType::Memory64BitRegionUpper => {
                panic!("Invalid PCI Bar type");
            }
        }

        let (mask, lower_bits) = match config.bar_type {
            PciBarRegionType::Memory32BitRegion | PciBarRegionType::Memory64BitRegion => (
                BAR_MEM_ADDR_MASK,
                config.prefetchable as u32 | config.bar_type as u32,
            ),
            PciBarRegionType::IoRegion => (BAR_IO_ADDR_MASK, config.bar_type as u32),
            PciBarRegionType::Memory64BitRegionUpper => {
                panic!("Invalid PCI Bar type");
            }
        };
        self.registers[reg_idx] = ((config.addr as u32) & mask) | lower_bits;
        self.writable_bits[reg_idx] = mask;
        self.bars[config.bar_idx].set(
            self.registers[reg_idx] & mask,
            config.size as u32,
            config.bar_type,
            false,
        );

        Ok(config.bar_idx)
    }

    /// Adds rom expansion BAR for normal PCI devices.
    pub fn add_device_rom_bar(
        &mut self,
        config: &PciBarConfiguration,
        active: u32,
    ) -> Result<usize> {
        if self.header_type != PciHeaderType::Device {
            return Err(Error::BarInvalid(config.bar_idx));
        }
        if config.bar_idx != NUM_BAR_REGS {
            return Err(Error::RomBarInvalid(config.bar_idx));
        }
        if self.bar_used(NUM_BAR_REGS) {
            return Err(Error::RomBarInUse(config.bar_idx));
        }
        if config.size.count_ones() != 1 || (config.size & u64::from(ROM_BAR_ADDR_MASK)) == 0 {
            return Err(Error::RomBarSizeInvalid(config.size));
        }
        let end_addr = config
            .addr
            .bitand(!(u64::from(!ROM_BAR_ADDR_MASK)))
            .checked_add(config.size - 1)
            .ok_or(Error::RomBarAddressInvalid(config.addr, config.size))?;
        if end_addr > u64::from(u32::max_value()) {
            return Err(Error::RomBarAddressInvalid(config.addr, config.size));
        }

        let mask = ROM_BAR_ADDR_MASK & !(config.size as u32 - 1);
        self.registers[ROM_BAR_REG] = ((config.addr as u32) & mask) | active;
        self.writable_bits[ROM_BAR_REG] = mask;
        self.bars[NUM_BAR_REGS].set(
            self.registers[ROM_BAR_REG] & mask,
            config.size as u32,
            PciBarRegionType::Memory32BitRegion,
            false,
        );
        Ok(config.bar_idx)
    }

    /// Returns the address of the given BAR region.
    pub fn get_device_bar_addr(&self, bar_idx: usize) -> u64 {
        assert!(self.header_type == PciHeaderType::Device);
        match bar_idx {
            0..=6 => match self.bar_type(bar_idx) {
                None => 0,
                Some(PciBarRegionType::Memory64BitRegionUpper) => 0,
                _ => {
                    let mut addr = u64::from(self.bar_addr(bar_idx));
                    if let Some(PciBarRegionType::Memory64BitRegion) = self.bar_type(bar_idx) {
                        addr |= u64::from(self.bar_addr(bar_idx + 1)) << 32;
                    }
                    addr
                }
            },
            _ => panic!("invalid PCI BAR index {}", bar_idx),
        }
    }

    pub fn get_bar_programming_params(&mut self) -> Option<BarProgrammingParams> {
        self.bar_programming_params.take()
    }
}

// Implement PCI Bus resource allocation/free.
impl PciConfiguration {
    fn write_device_bar_reg(&mut self, reg_idx: usize, value: u32) -> bool {
        debug_assert!(
            (BAR0_REG..BAR0_REG + NUM_BAR_REGS).contains(&reg_idx) || reg_idx == ROM_BAR_REG
        );
        let mut mask = self.writable_bits[reg_idx];
        let probing = value == 0xffff_ffff;

        // Handle very specific case where the BAR is being written with all 1's to retrieve the
        // BAR size on next BAR reading.
        if probing {
            if reg_idx == ROM_BAR_REG {
                mask = self.bars[NUM_BAR_REGS].mask();
            } else {
                mask = self.bars[reg_idx - BAR0_REG].mask();
            }
        }

        let val = self.registers[reg_idx];
        self.registers[reg_idx] = (val & !self.writable_bits[reg_idx]) | (value & mask);
        if !probing {
            if let Some(param) = self.detect_bar_programming(reg_idx) {
                self.free_bar_resource(&param);
                self.allocate_bar_resource(&param);
                assert!(self.bar_programming_params.is_none());
                self.bar_programming_params = Some(param);
            }
        }

        true
    }

    fn detect_bar_programming(&mut self, reg_idx: usize) -> Option<BarProgrammingParams> {
        // Handle special case where the address being written is different from the address
        // initially provided. This is a BAR reprogramming case which needs to be properly caught.
        if (BAR0_REG..BAR0_REG + NUM_BAR_REGS).contains(&reg_idx) {
            let bar_idx = reg_idx - BAR0_REG;
            let mask = self.writable_bits[reg_idx];
            let value = self.registers[reg_idx] & mask;
            if let Some(bar_type) = self.bar_type(bar_idx) {
                match bar_type {
                    PciBarRegionType::Memory32BitRegion | PciBarRegionType::IoRegion => {
                        if (value & mask) != self.bar_addr(bar_idx) {
                            debug!(
                                "DETECT BAR REPROG: current 0x{:x}, new 0x{:x}",
                                self.registers[reg_idx], value
                            );
                            self.bars[bar_idx].addr = value;
                            return Some(BarProgrammingParams {
                                bar_idx,
                                bar_type,
                                old_base: u64::from(self.bar_addr(bar_idx)),
                                new_base: u64::from(value & mask),
                                len: u64::from(self.bar_size(bar_idx)),
                            });
                        }
                    }

                    // 64-bit BAR will be handled when updating the upper 32-bit address.
                    PciBarRegionType::Memory64BitRegion => {}

                    PciBarRegionType::Memory64BitRegionUpper => {
                        debug!(
                            "DETECT BAR REPROG: current 0x{:x}, new 0x{:x}",
                            self.registers[reg_idx], value
                        );
                        let mask2 = self.writable_bits[reg_idx - 1];
                        if (value & mask) != self.bar_addr(bar_idx)
                            || (self.registers[reg_idx - 1] & mask2) != self.bar_addr(bar_idx - 1)
                        {
                            let old_base = u64::from(self.bar_addr(bar_idx)) << 32
                                | u64::from(self.bar_addr(bar_idx - 1));
                            let new_base = u64::from(value & mask) << 32
                                | u64::from(self.registers[reg_idx - 1] & mask2);
                            let len = u64::from(self.bar_size(bar_idx)) << 32
                                | u64::from(self.bar_size(bar_idx - 1));
                            let bar_type = PciBarRegionType::Memory64BitRegion;

                            self.bars[bar_idx].addr = value;
                            self.bars[bar_idx - 1].addr = self.registers[reg_idx - 1];

                            return Some(BarProgrammingParams {
                                bar_idx: bar_idx - 1,
                                bar_type,
                                old_base,
                                new_base,
                                len,
                            });
                        }
                    }
                }
            }
        } else if reg_idx == ROM_BAR_REG && self.bar_used(NUM_BAR_REGS) {
            debug!(
                "DETECT ROM BAR REPROG: current 0x{:x}, new 0x{:x}",
                self.registers[reg_idx],
                self.bar_addr(NUM_BAR_REGS)
            );
            if self.is_device_bar_addr_changed(reg_idx) {
                let mask = self.writable_bits[reg_idx];
                let value = self.registers[reg_idx] & mask;
                let old_addr = self.bar_addr(NUM_BAR_REGS);

                self.bars[NUM_BAR_REGS].addr = value;
                return Some(BarProgrammingParams {
                    bar_idx: NUM_BAR_REGS,
                    bar_type: PciBarRegionType::Memory32BitRegion,
                    old_base: u64::from(old_addr),
                    new_base: u64::from(value),
                    len: u64::from(self.bar_size(NUM_BAR_REGS)),
                });
            }
        }

        None
    }

    fn is_device_bar_addr_changed(&self, reg_idx: usize) -> bool {
        let value = self.registers[reg_idx];
        let mask = self.writable_bits[reg_idx];

        if (BAR0_REG..BAR0_REG + NUM_BAR_REGS).contains(&reg_idx) {
            self.bar_addr(reg_idx - BAR0_REG) != (value & mask)
        } else if reg_idx == ROM_BAR_REG {
            self.bar_addr(NUM_BAR_REGS) != (value & mask)
        } else {
            false
        }
    }

    fn allocate_bar_resource(&mut self, param: &BarProgrammingParams) {
        // Treat zero base as BAR disabled.
        if param.new_base == 0 {
            return;
        }

        assert!(!self.bar_allocated(param.bar_idx));
        let constraint = match param.bar_type {
            PciBarRegionType::IoRegion => {
                let range = (
                    param.new_base as u16,
                    (param.new_base + param.len - 1) as u16,
                );
                ResourceConstraint::pio_with_constraints(param.len as u16, Some(range), 1)
            }
            PciBarRegionType::Memory32BitRegion | PciBarRegionType::Memory64BitRegion => {
                ResourceConstraint::mmio_with_constraints(
                    param.len,
                    Some((param.new_base, param.new_base + param.len - 1)),
                    1,
                )
            }
            PciBarRegionType::Memory64BitRegionUpper => {
                panic!("Invalid fake PCI Bar type when freeing Bar resources!");
            }
        };
        let constraints = vec![constraint];
        if let Err(e) = self.bus.upgrade().unwrap().allocate_resources(&constraints) {
            debug!("failed to allocate resource for PCI BAR: {:?}", e);
        } else {
            self.set_bar_allocated(param.bar_idx, true);
        }
    }

    fn free_bar_resource(&mut self, param: &BarProgrammingParams) {
        if self.bar_allocated(param.bar_idx) {
            let res = match param.bar_type {
                PciBarRegionType::Memory32BitRegion | PciBarRegionType::Memory64BitRegion => {
                    Resource::MmioAddressRange {
                        base: param.old_base,
                        size: param.len,
                    }
                }
                PciBarRegionType::IoRegion => Resource::PioAddressRange {
                    base: param.old_base as u16,
                    size: param.len as u16,
                },
                PciBarRegionType::Memory64BitRegionUpper => {
                    panic!("Invalid fake PCI Bar type when freeing Bar resources!");
                }
            };
            let mut resources = DeviceResources::new();
            resources.append(res);
            self.bus.upgrade().unwrap().free_resources(resources);
            self.set_bar_allocated(param.bar_idx, false);
        }
    }

    fn bar_addr(&self, bar_idx: usize) -> u32 {
        self.bars[bar_idx].addr
    }

    fn bar_size(&self, bar_idx: usize) -> u32 {
        self.bars[bar_idx].size
    }

    fn bar_type(&self, bar_idx: usize) -> Option<PciBarRegionType> {
        self.bars[bar_idx].type_
    }

    fn bar_used(&self, bar_idx: usize) -> bool {
        self.bars[bar_idx].type_.is_some()
    }

    fn bar_allocated(&self, bar_idx: usize) -> bool {
        self.bars[bar_idx].allocated
    }

    fn set_bar_allocated(&mut self, bar_idx: usize, allocated: bool) {
        self.bars[bar_idx].allocated = allocated;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    const CAPABILITY_LIST_HEAD_OFFSET: usize = 0x34;

    #[derive(Copy, Clone)]
    enum TestPI {
        Test = 0x5a,
    }

    impl PciProgrammingInterface for TestPI {
        fn get_register_value(&self) -> u8 {
            *self as u8
        }
    }

    pub(crate) fn create_new_config(bus: &Arc<PciBus>) -> PciConfiguration {
        let mut resources = DeviceResources::new();
        resources.append(Resource::MmioAddressRange {
            base: 0x0u64,
            size: 0x10_0000_0000,
        });
        bus.assign_resources(resources).unwrap();

        PciConfiguration::new(
            Arc::downgrade(bus),
            0x8086,
            0x0386,
            PciClassCode::NetworkController,
            &PciMultimediaSubclass::AudioController,
            Some(&TestPI::Test),
            PciHeaderType::Device,
            0xABCD,
            0x2468,
            None,
        )
        .unwrap()
    }

    #[test]
    fn test_pci_config_new() {
        let bus = Arc::new(PciBus::new(0));
        let config = create_new_config(&bus);
        assert_eq!(config.registers[0], 0x03868086);
        assert_eq!(config.registers[1], 0x00000000);
        assert_eq!(config.registers[2], 0x02015a00);
        assert_eq!(config.registers[3], 0x00000000);
        assert_eq!(config.registers[4], 0x00000000);
        assert_eq!(config.registers[5], 0x00000000);
        assert_eq!(config.registers[6], 0x00000000);
        assert_eq!(config.registers[7], 0x00000000);
        assert_eq!(config.registers[8], 0x00000000);
        assert_eq!(config.registers[9], 0x00000000);
        assert_eq!(config.registers[10], 0x00000000);
        assert_eq!(config.registers[11], 0x2468abcd);
        assert_eq!(config.registers[12], 0x00000000);
        assert_eq!(config.registers[13], 0x00000000);
        assert_eq!(config.registers[14], 0x00000000);
        assert_eq!(config.registers[15], 0x00000000);
    }

    #[repr(C, packed)]
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    #[allow(dead_code)]
    pub(crate) struct TestCap {
        pub(crate) id: u8,
        pub(crate) next: u8,
        pub(crate) len: u8,
        pub(crate) foo: u8,
        pub(crate) bar: u32,
        pub(crate) zoo: u8,
    }

    impl PciCapability for TestCap {
        fn len(&self) -> usize {
            9
        }

        fn set_next_cap(&mut self, next: u8) {
            self.next = next;
        }

        fn read_u8(&mut self, offset: usize) -> u8 {
            match offset {
                0 => self.id,
                1 => self.next,
                2 => self.len,
                3 => self.foo,
                4..=7 => (self.bar >> ((offset as u32 & 0x3) * 8)) as u8,
                8 => self.zoo,
                _ => 0xff,
            }
        }

        fn write_u8(&mut self, offset: usize, value: u8) {
            match offset {
                2 => self.len = value,
                3 => self.foo = value,
                4..=7 => {
                    let mask = 0xff << ((offset as u32 & 0x3) * 8);
                    self.bar &= !mask;
                    self.bar |= (value as u32) << ((offset as u32 & 0x3) * 8);
                }
                8 => self.zoo = value,
                _ => {}
            }
        }

        fn pci_capability_type(&self) -> PciCapabilityId {
            PciCapabilityId::Test
        }
    }

    #[test]
    fn add_capability() {
        let bus = Arc::new(PciBus::new(0));
        let mut config = create_new_config(&bus);

        // Add two capabilities with different contents.
        let cap1 = Box::new(TestCap {
            id: PciCapabilityId::VendorSpecific as u8,
            next: 0,
            len: 4,
            foo: 0xAA,
            bar: 0x12345678,
            zoo: 0x3c,
        });

        let cap1_offset = config.add_capability(Arc::new(Mutex::new(cap1))).unwrap();
        assert_eq!(cap1_offset % 4, 0);

        let cap2 = Box::new(TestCap {
            id: PciCapabilityId::VendorSpecific as u8,
            next: 0,
            len: 8,
            foo: 0xBB,
            bar: 0x11223344,
            zoo: 0x1f,
        });
        let cap2_offset = config.add_capability(Arc::new(Mutex::new(cap2))).unwrap();
        assert_eq!(cap2_offset % 4, 0);

        // The capability list head should be pointing to cap2.
        let (handled, cap_ptr) = config.read_u8(CAPABILITY_LIST_HEAD_OFFSET);
        assert!(handled);
        assert_eq!(cap2_offset, cap_ptr as usize);

        // Verify the contents of the capabilities.
        let (handled, cap1_data) = config.read_u32(cap1_offset);
        assert!(handled);
        assert_eq!(cap1_data & 0xFF, 0x09); // capability ID
        assert_eq!((cap1_data >> 8) & 0xFF, 0); // next capability pointer
        assert_eq!((cap1_data >> 16) & 0xFF, 0x04); // cap1.len
        assert_eq!((cap1_data >> 24) & 0xFF, 0xAA); // cap1.foo
        let (handled, cap1_data) = config.read_u16(cap1_offset + 4);
        assert!(handled);
        assert_eq!(cap1_data, 0x5678);
        let (handled, cap1_data) = config.read_u8(cap1_offset + 7);
        assert!(handled);
        assert_eq!(cap1_data, 0x12);
        let (handled, _cap1_data) = config.read_u16(cap1_offset + 7);
        assert!(!handled);

        assert!(config.write_u16(cap1_offset + 4, 0x1111));
        let (handled, cap1_data) = config.read_u32(cap1_offset + 4);
        assert!(handled);
        assert_eq!(cap1_data, 0x1234_1111);

        let (handled, cap1_data) = config.read_u32(cap1_offset + 8);
        assert!(handled);
        assert_eq!(cap1_data, 0xffff_ff3c);
        let (handled, cap1_data) = config.read_u16(cap1_offset + 8);
        assert!(handled);
        assert_eq!(cap1_data, 0xff3c);
        let (handled, cap1_data) = config.read_u16(cap1_offset + 10);
        assert!(handled);
        assert_eq!(cap1_data, 0xffff);

        assert!(config.write_u32(cap1_offset + 8, 0xa5a5_a5a5));
        let (handled, cap1_data) = config.read_u32(cap1_offset + 8);
        assert!(handled);
        assert_eq!(cap1_data, 0xffff_ffa5);

        let (handled, cap2_data) = config.read_u32(cap2_offset);
        assert!(handled);
        assert_eq!(cap2_data & 0xFF, 0x09); // capability ID
        assert_eq!((cap2_data >> 8) & 0xFF, cap1_offset as u32); // next capability pointer
        assert_eq!((cap2_data >> 16) & 0xFF, 0x08); // cap2.len
        assert_eq!((cap2_data >> 24) & 0xFF, 0xbb); // cap2.foo
    }

    #[test]
    fn class_code() {
        let bus = Arc::new(PciBus::new(0));
        let config = create_new_config(&bus);

        let (handled, class_reg) = config.read_u32(0x8);
        let class_code = (class_reg >> 24) & 0xFF;
        let subclass = (class_reg >> 16) & 0xFF;
        let prog_if = (class_reg >> 8) & 0xFF;
        assert!(handled);
        assert_eq!(class_code, 0x02);
        assert_eq!(subclass, 0x01);
        assert_eq!(prog_if, 0x5a);
    }

    #[test]
    fn test_bar_configuration() {
        let mut config: PciBarConfiguration = Default::default();

        assert_eq!(config.bar_idx, 0);
        assert_eq!(config.bar_type, PciBarRegionType::Memory64BitRegion);
        assert!(!config.prefetchable());
        assert_eq!(config.size(), 0);
        assert_eq!(config.addr, 0);

        config = config.set_bar_type(PciBarRegionType::IoRegion);
        assert_eq!(config.bar_type(), PciBarRegionType::IoRegion);
        config = config.set_address(0x1000);
        assert_eq!(config.address(), 0x1000);
        config = config.set_size(0x2000);
        assert_eq!(config.size(), 0x2000);
        config = config.set_bar_index(5);
        assert_eq!(config.bar_index(), 5);
    }

    #[test]
    fn test_add_device_bar() {
        let bus = Arc::new(PciBus::new(0));
        let mut config = create_new_config(&bus);

        // Get address of unused BAR
        assert_eq!(config.get_device_bar_addr(0), 0);

        // Create a 32bit MMIO BAR0
        let bar = PciBarConfiguration {
            bar_idx: 0,
            bar_type: PciBarRegionType::Memory32BitRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0x10000000,
            size: 0x1000,
        };
        config.add_device_bar(&bar).unwrap();
        assert_eq!(config.get_device_bar_addr(0), 0x1000_0000);
        assert_eq!(config.registers[4], 0x1000_0000);
        assert_eq!(config.writable_bits[4], !0xf);
        assert_eq!(config.bar_addr(0), 0x1000_0000);
        assert_eq!(config.bar_size(0), 0x1000);
        assert_eq!(
            config.bar_type(0),
            Some(PciBarRegionType::Memory32BitRegion)
        );

        // Can't use BAR0 again.
        match config.add_device_bar(&bar).unwrap_err() {
            Error::BarInUse(i) => assert_eq!(i, 0),
            _ => panic!("expected error BarInUse"),
        }

        // Invalid size
        let bar = PciBarConfiguration {
            bar_idx: 1,
            bar_type: PciBarRegionType::Memory32BitRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0x1000_0000,
            size: 0x1_1000,
        };
        match config.add_device_bar(&bar).unwrap_err() {
            Error::BarSizeInvalid(i) => assert_eq!(i, 0x1_1000),
            _ => panic!("expected error BarInUse"),
        }

        // Invalid size
        let bar = PciBarConfiguration {
            bar_idx: 1,
            bar_type: PciBarRegionType::Memory32BitRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0x1000_0000,
            size: 0x1_0000_0000,
        };
        match config.add_device_bar(&bar).unwrap_err() {
            Error::BarSizeInvalid(i) => assert_eq!(i, 0x100000000),
            _ => panic!("expected error BarInUse"),
        }

        // BAR size too small
        let bar = PciBarConfiguration {
            bar_idx: 1,
            bar_type: PciBarRegionType::Memory32BitRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0xffff_ffff,
            size: 0x1,
        };
        match config.add_device_bar(&bar).unwrap_err() {
            Error::BarSizeInvalid(s) => assert_eq!(s, 0x1),
            _ => panic!("expected error BarInUse"),
        }

        // Size overflow
        let bar = PciBarConfiguration {
            bar_idx: 1,
            bar_type: PciBarRegionType::Memory32BitRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0xffff_ffff,
            size: 0x10,
        };
        match config.add_device_bar(&bar).unwrap_err() {
            Error::BarAddressInvalid(a, _s) => assert_eq!(a, 0xffff_ffff),
            _ => panic!("expected error BarInUse"),
        }

        // Invalid BAR address
        let bar = PciBarConfiguration {
            bar_idx: 1,
            bar_type: PciBarRegionType::Memory32BitRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0x100000000,
            size: 0x10,
        };
        match config.add_device_bar(&bar).unwrap_err() {
            Error::BarAddressInvalid(a, _s) => assert_eq!(a, 0x1_0000_0000),
            _ => panic!("expected error BarInUse"),
        }

        // Size overflow
        let bar = PciBarConfiguration {
            bar_idx: 1,
            bar_type: PciBarRegionType::Memory64BitRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0xffff_ffff_ffff_ffff,
            size: 0x2,
        };
        match config.add_device_bar(&bar).unwrap_err() {
            Error::BarAddressInvalid(_a, s) => assert_eq!(s, 2),
            _ => panic!("expected error BarInUse"),
        }

        // Can't use BAR6 again.
        let bar = PciBarConfiguration {
            bar_idx: 6,
            bar_type: PciBarRegionType::Memory32BitRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0x1000_0000,
            size: 0x1000,
        };
        match config.add_device_bar(&bar).unwrap_err() {
            Error::BarInvalid(i) => assert_eq!(i, 6),
            _ => panic!("expected error BarInUse"),
        }

        // Allocate BAR2
        let bar = PciBarConfiguration {
            bar_idx: 2,
            bar_type: PciBarRegionType::Memory32BitRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0x2000_0000,
            size: 0x1000,
        };
        config.add_device_bar(&bar).unwrap();

        // Can't use BAR2 for 64bit Bar 1.
        let bar = PciBarConfiguration {
            bar_idx: 1,
            bar_type: PciBarRegionType::Memory64BitRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0x3000_0000,
            size: 0x1000,
        };
        match config.add_device_bar(&bar).unwrap_err() {
            Error::BarInUse64(i) => assert_eq!(i, 1),
            _ => panic!("expected error BarInUse"),
        }

        // Allocate BAR3,4
        let bar = PciBarConfiguration {
            bar_idx: 3,
            bar_type: PciBarRegionType::Memory64BitRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0x1_4000_0000,
            size: 0x1000,
        };
        config.add_device_bar(&bar).unwrap();
        assert_eq!(config.get_device_bar_addr(3), 0x1_4000_0000);
        assert_eq!(config.get_device_bar_addr(4), 0);
        assert_eq!(config.registers[7], 0x4000_0004);
        assert_eq!(config.writable_bits[7], !0xf);
        assert_eq!(config.bar_addr(3), 0x4000_0000);
        assert_eq!(config.bar_size(3), 0x1000);
        assert_eq!(
            config.bar_type(3),
            Some(PciBarRegionType::Memory64BitRegion)
        );
        assert_eq!(config.registers[8], 0x0000_0001);
        assert_eq!(config.writable_bits[8], !0x0);
        assert_eq!(config.bar_addr(4), 0x0000_0001);
        assert_eq!(config.bar_size(4), 0x0);
        assert_eq!(
            config.bar_type(4),
            Some(PciBarRegionType::Memory64BitRegionUpper)
        );

        // Allocate BAR5
        let bar = PciBarConfiguration {
            bar_idx: 5,
            bar_type: PciBarRegionType::IoRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0x2000,
            size: 0x1000,
        };
        config.add_device_bar(&bar).unwrap();
    }

    #[test]
    fn test_add_device_rom_bar() {
        let bus = Arc::new(PciBus::new(0));
        let mut config = create_new_config(&bus);

        // Invalid BAR index
        let bar = PciBarConfiguration {
            bar_idx: 0,
            bar_type: PciBarRegionType::Memory32BitRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0x1000_0000,
            size: 0x1000,
        };
        match config.add_device_rom_bar(&bar, 0).unwrap_err() {
            Error::RomBarInvalid(i) => assert_eq!(i, 0),
            _ => panic!("expected error BarInUse"),
        }

        // Bar size too large
        let bar = PciBarConfiguration {
            bar_idx: 6,
            bar_type: PciBarRegionType::Memory32BitRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0x1000_0000,
            size: 0x1_0000_0000,
        };
        match config.add_device_rom_bar(&bar, 0).unwrap_err() {
            Error::RomBarSizeInvalid(s) => assert_eq!(s, 0x1_0000_0000),
            _ => panic!("expected error BarInUse"),
        }

        // BAR overflow
        let bar = PciBarConfiguration {
            bar_idx: 6,
            bar_type: PciBarRegionType::Memory32BitRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0xF000_0000,
            size: 0x2000_0000,
        };
        match config.add_device_rom_bar(&bar, 0).unwrap_err() {
            Error::RomBarAddressInvalid(a, _s) => assert_eq!(a, 0xf000_0000),
            _ => panic!("expected error BarInUse"),
        }

        let bar = PciBarConfiguration {
            bar_idx: 6,
            bar_type: PciBarRegionType::Memory32BitRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0xE000_00F0,
            size: 0x2000_0000,
        };
        config.add_device_rom_bar(&bar, 1).unwrap();
        assert_eq!(config.registers[12], 0xe000_0001);
        assert_eq!(config.writable_bits[12], 0xe000_0000);
        assert_eq!(config.bar_addr(NUM_BAR_REGS), 0xe000_0000);
        assert_eq!(config.bar_size(NUM_BAR_REGS), 0x2000_0000);
        assert!(config.bar_type(NUM_BAR_REGS).is_some());
        assert_eq!(config.get_device_bar_addr(6), 0xe000_0000);

        match config.add_device_rom_bar(&bar, 0).unwrap_err() {
            Error::RomBarInUse(i) => assert_eq!(i, 6),
            _ => panic!("expected error BarInUse"),
        }
    }

    #[test]
    fn test_write_device_bar_reg() {
        let bus = Arc::new(PciBus::new(0));
        let mut config = create_new_config(&bus);

        let bar = PciBarConfiguration {
            bar_idx: 0,
            bar_type: PciBarRegionType::Memory32BitRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0x10000000,
            size: 0x1000,
        };
        config.add_device_bar(&bar).unwrap();

        // Detecting BAR size
        assert!(config.write_u32(0x10, 0xffffffff));
        assert_eq!(config.read_u32(0x10), (true, 0xfffff000));
        assert_eq!(config.get_bar_programming_params(), None);

        // Allocate BAR2,3
        let bar = PciBarConfiguration {
            bar_idx: 2,
            bar_type: PciBarRegionType::Memory64BitRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0x1_4000_0000,
            size: 0x2_0000_0000,
        };
        config.add_device_bar(&bar).unwrap();
        assert!(config.write_u32(0x18, 0xffff_ffff));
        assert_eq!(config.get_bar_programming_params(), None);
        assert!(config.write_u32(0x1c, 0xffff_ffff));
        assert_eq!(config.get_bar_programming_params(), None);
        assert_eq!(config.read_u32(0x18), (true, 0x4));
        assert_eq!(config.read_u32(0x1c), (true, 0xffff_fffe));

        assert!(config.write_u32(0x18, 0x2000_0000));
        assert_eq!(config.get_bar_programming_params(), None);
        assert!(config.write_u32(0x1c, 0x0000_0002));
        let params = config.get_bar_programming_params().unwrap();
        assert_eq!(params.bar_idx, 2);
        assert_eq!(params.bar_type, PciBarRegionType::Memory64BitRegion);
        assert_eq!(params.new_base, 0x2_2000_0000);
        assert!(config.bar_allocated(params.bar_idx));

        assert!(config.write_u32(0x20, 0xffff_ffff));
        assert_eq!(config.registers[12], 0x0);
    }

    #[test]
    fn test_update_rom_bar() {
        let bus = Arc::new(PciBus::new(0));
        let mut config = create_new_config(&bus);

        let bar = PciBarConfiguration {
            bar_idx: 6,
            bar_type: PciBarRegionType::Memory32BitRegion,
            prefetchable: PciBarPrefetchable::NotPrefetchable,
            addr: 0xE0000000,
            size: 0x10000000,
        };
        config.add_device_rom_bar(&bar, 1).unwrap();
        assert_eq!(config.get_device_bar_addr(NUM_BAR_REGS), 0xe0000000);

        // Probe size of the BAR
        assert!(config.write_u32(0x30, 0xffffffff));
        assert!(config.get_bar_programming_params().is_none());
        assert_eq!(config.read_u32(0x30), (true, 0xf0000001));

        // Write the BAR with the same address
        assert!(config.write_u32(0x30, 0xe0001000));
        assert_eq!(config.get_bar_programming_params(), None);

        // Write the BAR with a different address
        assert!(config.write_u32(0x30, 0xc0001000));
        assert_eq!(config.read_u32(0x30), (true, 0xc0000001));
        let params = config.get_bar_programming_params().unwrap();
        assert_eq!(params.bar_idx, 6);
        assert_eq!(params.old_base, 0xe000_0000);
        assert_eq!(params.new_base, 0xc000_0000);
        assert_eq!(params.len, 0x1000_0000);
        assert_eq!(params.bar_type, PciBarRegionType::Memory32BitRegion);
        assert!(config.bar_allocated(params.bar_idx));

        // Reset BAR without address assigned
        assert!(config.write_u32(0x30, 0x00000000));
        assert_eq!(config.read_u32(0x30), (true, 0x00000001));
        let params = config.get_bar_programming_params().unwrap();
        assert_eq!(params.bar_idx, 6);
        assert_eq!(params.old_base, 0xc000_0000);
        assert_eq!(params.new_base, 0x0000_0000);
        assert_eq!(params.len, 0x1000_0000);
        assert_eq!(params.bar_type, PciBarRegionType::Memory32BitRegion);
        assert!(!config.bar_allocated(params.bar_idx));
    }

    #[test]
    fn test_read_write_u32() {
        let bus = Arc::new(PciBus::new(0));
        let mut config = create_new_config(&bus);

        assert!(!config.write_u32(1, 0x1));
        assert!(config.write_u32(0x0, 0));
        assert_eq!(config.read_u32(0x0), (true, 0x0386_8086));
        assert!(config.write_u32(0x4, 0xffff_ffff));
        assert_eq!(config.read_u32(0x4), (true, 0x0000_ffff));
        assert!(config.write_u32(0x8, 0xffff_ffff));
        assert_eq!(config.read_u32(0x8), (true, 0x0201_5a00));
        assert!(config.write_u32(0xc, 0xffff_ffff));
        assert_eq!(config.read_u32(0xc), (true, 0x0000_00ff));
        assert!(config.write_u32(0x28, 0xffff_ffff));
        assert_eq!(config.read_u32(0x28), (true, 0x0000_0000));
        assert!(config.write_u32(0x2c, 0xffff_ffff));
        assert_eq!(config.read_u32(0x2c), (true, 0x2468_abcd));
        assert!(config.write_u32(0x34, 0xffff_ffff));
        assert_eq!(config.read_u32(0x34), (true, 0x0000_0000));
        assert!(config.write_u32(0x38, 0xffff_ffff));
        assert_eq!(config.read_u32(0x38), (true, 0x0000_0000));
        assert!(config.write_u32(0x3c, 0xffff_ffff));
        assert_eq!(config.read_u32(0x3c), (true, 0x0000_00ff));
    }

    #[test]
    fn test_read_write_config() {
        let bus = Arc::new(PciBus::new(0));
        let mut config = create_new_config(&bus);

        let mut data = [0u8; 4];
        config.read_config(0x3c, &mut data);
        assert_eq!(data, [0u8, 0u8, 0u8, 0u8]);

        let mut data = [0xffu8; 4];
        config.write_config(0x3d, &data);
        config.read_config(0x3c, &mut data);
        assert_eq!(data, [0u8, 0u8, 0u8, 0u8]);

        let mut data = [0xffu8; 4];
        config.write_config(0x3b, &data);
        config.read_config(0x3c, &mut data);
        assert_eq!(data, [0u8, 0u8, 0u8, 0u8]);

        let mut data = [0xffu8; 4];
        config.write_config(0x3c, &data);
        config.read_config(0x3c, &mut data);
        assert_eq!(data, [0xffu8, 0u8, 0u8, 0u8]);

        let data = [0xa5u8];
        config.write_config(0x3c, &data);
        let mut data = [0xffu8; 4];
        config.read_config(0x3c, &mut data);
        assert_eq!(data, [0xa5u8, 0u8, 0u8, 0u8]);
    }

    #[test]
    fn test_set_irq() {
        let bus = Arc::new(PciBus::new(0));
        let mut config = create_new_config(&bus);
        config.set_irq(15, PciInterruptPin::IntD);
        assert_eq!(config.registers[15] & 0xffff, 0x40f);
    }
}
