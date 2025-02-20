// Copyright (C) 2019 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Descriptors representing device resource allocation requirements and assigned resources.
//!
//! There are several components related to resource management:
//! - the Dragonball Secure Sandbox (VMM), which is responsible for creating and registering devices
//!   to the device manager.
//! - the device manager, which manages all devices of a Dragonball Secure Sandbox instance.
//! - the devices, which implement virtual device backends for the guest.
//!
//! They cooperate with each to provide resources required by each device. The high level flow of
//! resource management is as below:
//! 1) the VMM creates a new device object.
//! 2) the device returns an array of [ResourceConstraint](self::ResourceConstraint),
//!    describing the required resources and resource allocation constraints.
//! 3) the VMM allocates required resources from a resource manager,
//! 4) the VMM passes the allocated resources [DeviceResources](self::DeviceResources),
//!    which is an array of [Resource](self::Resource), to the device object.
//! 5) the VMM registers the new device onto corresponding device managers according the allocated
//!    resources.

use std::ops::Deref;

/// Enumeration describing a device's resource allocation requirements and constraints.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ResourceConstraint {
    /// Constraint for an IO Port address range.
    PioAddress {
        /// Allocating resource within the range [`min`, `max`] if specified.
        range: Option<(u16, u16)>,
        /// Alignment for the allocated address.
        align: u16,
        /// Size for the allocated address range.
        size: u16,
    },
    /// Constraint for a Memory Mapped IO address range.
    MmioAddress {
        /// Allocating resource within the range [`min`, `max`] if specified.
        range: Option<(u64, u64)>,
        /// Alignment for the allocated address.
        align: u64,
        /// Size for the allocated address range.
        size: u64,
    },
    /// Constraint for a Guest Mem address range.
    MemAddress {
        /// Allocating resource within the range [`min`, `max`] if specified.
        range: Option<(u64, u64)>,
        /// Alignment for the allocated address.
        align: u64,
        /// Size for the allocated address range.
        size: u64,
    },
    /// Constraint for a legacy IRQ.
    LegacyIrq {
        /// Reserving the pre-allocated IRQ if it's specified.
        irq: Option<u32>,
    },
    /// Constraint for PCI MSI IRQs.
    PciMsiIrq {
        /// Number of Irqs to allocate.
        size: u32,
    },
    /// Constraint for PCI MSIx IRQs.
    PciMsixIrq {
        /// Number of Irqs to allocate.
        size: u32,
    },
    /// Constraint for generic IRQs.
    GenericIrq {
        /// Number of Irqs to allocate.
        size: u32,
    },
    /// Constraint for KVM mem_slot indexes to map memory into the guest.
    KvmMemSlot {
        /// Allocating kvm memory slots starting from the index `slot` if
        /// specified.
        slot: Option<u32>,
        /// Number of slots to allocate.
        size: u32,
    },
}

impl ResourceConstraint {
    /// Create a new PIO address constraint object with default configuration.
    pub fn new_pio(size: u16) -> Self {
        ResourceConstraint::PioAddress {
            range: None,
            align: 0x1,
            size,
        }
    }

    /// Create a new PIO address constraint object.
    pub fn pio_with_constraints(size: u16, range: Option<(u16, u16)>, align: u16) -> Self {
        ResourceConstraint::PioAddress { range, align, size }
    }

    /// Create a new MMIO address constraint object with default configuration.
    pub fn new_mmio(size: u64) -> Self {
        ResourceConstraint::MmioAddress {
            range: None,
            align: 0x1000,
            size,
        }
    }

    /// Create a new MMIO address constraint object.
    pub fn mmio_with_constraints(size: u64, range: Option<(u64, u64)>, align: u64) -> Self {
        ResourceConstraint::MmioAddress { range, align, size }
    }

    /// Create a new Mem address constraint object with default configuration.
    pub fn new_mem(size: u64) -> Self {
        ResourceConstraint::MemAddress {
            range: None,
            align: 0x1000,
            size,
        }
    }

    /// Create a new Mem address constraint object.
    pub fn mem_with_constraints(size: u64, range: Option<(u64, u64)>, align: u64) -> Self {
        ResourceConstraint::MemAddress { range, align, size }
    }

    /// Create a new legacy IRQ constraint object.
    ///
    /// Allocating the pre-allocated legacy Irq `irq` if specified.
    pub fn new_legacy_irq(irq: Option<u32>) -> Self {
        ResourceConstraint::LegacyIrq { irq }
    }

    /// Create a new PCI MSI IRQ constraint object.
    pub fn new_pci_msi_irq(size: u32) -> Self {
        ResourceConstraint::PciMsiIrq { size }
    }

    /// Create a new PCI MSIX IRQ constraint object.
    pub fn new_pci_msix_irq(size: u32) -> Self {
        ResourceConstraint::PciMsixIrq { size }
    }

    /// Create a new Generic IRQ constraint object.
    pub fn new_generic_irq(size: u32) -> Self {
        ResourceConstraint::GenericIrq { size }
    }

    /// Create a new KVM memory slot constraint object.
    ///
    /// Allocating kvm memory slots starting from the index `slot` if specified.
    pub fn new_kvm_mem_slot(size: u32, slot: Option<u32>) -> Self {
        ResourceConstraint::KvmMemSlot { slot, size }
    }
}

/// Type of Message Singaled Interrupt
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MsiIrqType {
    /// PCI MSI IRQ numbers.
    PciMsi,
    /// PCI MSIx IRQ numbers.
    PciMsix,
    /// Generic MSI IRQ numbers.
    GenericMsi,
}

/// Enumeration for device resources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Resource {
    /// IO Port resource range.
    PioAddressRange {
        /// Pio resource base
        base: u16,
        /// Pio resource size
        size: u16,
    },
    /// Memory Mapped IO resource range.
    MmioAddressRange {
        /// Mmio resource base
        base: u64,
        /// Mmio resource size
        size: u64,
    },
    /// Guest Mem resource range.
    MemAddressRange {
        /// Mem resource base
        base: u64,
        /// Mem resource size
        size: u64,
    },
    /// Legacy IRQ number.
    LegacyIrq(u32),
    /// Message Signaled Interrupt
    MsiIrq {
        /// Msi irq type
        ty: MsiIrqType,
        /// Msi irq base
        base: u32,
        /// Msi irq size
        size: u32,
    },
    /// Network Interface Card MAC address.
    MacAddress(String),
    /// KVM memslot index.
    KvmMemSlot(u32),
}

/// Newtype to store a set of device resources.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeviceResources(Vec<Resource>);

impl DeviceResources {
    /// Create a container object to store device resources.
    pub fn new() -> Self {
        DeviceResources(Vec::new())
    }

    /// Append a device resource to the container object.
    pub fn append(&mut self, entry: Resource) {
        self.0.push(entry);
    }

    /// Get the IO port address resources.
    pub fn get_pio_address_ranges(&self) -> Vec<(u16, u16)> {
        let mut vec = Vec::new();
        for entry in self.0.iter().as_ref() {
            if let Resource::PioAddressRange { base, size } = entry {
                vec.push((*base, *size));
            }
        }
        vec
    }

    /// Get the Memory Mapped IO address resources.
    pub fn get_mmio_address_ranges(&self) -> Vec<(u64, u64)> {
        let mut vec = Vec::new();
        for entry in self.0.iter().as_ref() {
            if let Resource::MmioAddressRange { base, size } = entry {
                vec.push((*base, *size));
            }
        }
        vec
    }

    /// Get the Guest Memory address resources.
    pub fn get_mem_address_ranges(&self) -> Vec<(u64, u64)> {
        let mut vec = Vec::new();
        for entry in self.0.iter().as_ref() {
            if let Resource::MemAddressRange { base, size } = entry {
                vec.push((*base, *size));
            }
        }
        vec
    }

    /// Get the first legacy interrupt number(IRQ).
    pub fn get_legacy_irq(&self) -> Option<u32> {
        for entry in self.0.iter().as_ref() {
            if let Resource::LegacyIrq(base) = entry {
                return Some(*base);
            }
        }
        None
    }

    /// Get information about the first PCI MSI interrupt resource.
    pub fn get_pci_msi_irqs(&self) -> Option<(u32, u32)> {
        self.get_msi_irqs(MsiIrqType::PciMsi)
    }

    /// Get information about the first PCI MSIx interrupt resource.
    pub fn get_pci_msix_irqs(&self) -> Option<(u32, u32)> {
        self.get_msi_irqs(MsiIrqType::PciMsix)
    }

    /// Get information about the first Generic MSI interrupt resource.
    pub fn get_generic_msi_irqs(&self) -> Option<(u32, u32)> {
        self.get_msi_irqs(MsiIrqType::GenericMsi)
    }

    fn get_msi_irqs(&self, ty: MsiIrqType) -> Option<(u32, u32)> {
        for entry in self.0.iter().as_ref() {
            if let Resource::MsiIrq {
                ty: msi_type,
                base,
                size,
            } = entry
            {
                if ty == *msi_type {
                    return Some((*base, *size));
                }
            }
        }
        None
    }

    /// Get the KVM memory slots to map memory into the guest.
    pub fn get_kvm_mem_slots(&self) -> Vec<u32> {
        let mut vec = Vec::new();
        for entry in self.0.iter().as_ref() {
            if let Resource::KvmMemSlot(index) = entry {
                vec.push(*index);
            }
        }
        vec
    }

    /// Get the first resource information for NIC MAC address.
    pub fn get_mac_address(&self) -> Option<String> {
        for entry in self.0.iter().as_ref() {
            if let Resource::MacAddress(addr) = entry {
                return Some(addr.clone());
            }
        }
        None
    }

    /// Get immutable reference to all the resources.
    pub fn get_all_resources(&self) -> &[Resource] {
        &self.0
    }
}

impl Deref for DeviceResources {
    type Target = [Resource];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    const PIO_ADDRESS_SIZE: u16 = 5;
    const PIO_ADDRESS_BASE: u16 = 0;
    const MMIO_ADDRESS_SIZE: u64 = 0x8765_4321;
    const MMIO_ADDRESS_BASE: u64 = 0x1234_5678;
    const MEM_ADDRESS_SIZE: u64 = 0x8765_4321;
    const MEM_ADDRESS_BASE: u64 = 0x1234_5678;
    const LEGACY_IRQ: u32 = 0x168;
    const PCI_MSI_IRQ_SIZE: u32 = 0x8888;
    const PCI_MSI_IRQ_BASE: u32 = 0x6666;
    const PCI_MSIX_IRQ_SIZE: u32 = 0x16666;
    const PCI_MSIX_IRQ_BASE: u32 = 0x8888;
    const GENERIC_MSI_IRQS_SIZE: u32 = 0x16888;
    const GENERIC_MSI_IRQS_BASE: u32 = 0x16688;
    const MAC_ADDRESS: &str = "00:08:63:66:86:88";
    const KVM_SLOT_ID: u32 = 0x0100;

    pub fn get_device_resource() -> DeviceResources {
        let mut resource = DeviceResources::new();

        let entry = Resource::PioAddressRange {
            base: PIO_ADDRESS_BASE,
            size: PIO_ADDRESS_SIZE,
        };
        resource.append(entry.clone());
        assert_eq!(entry, resource[0]);

        let entry = Resource::MmioAddressRange {
            base: MMIO_ADDRESS_BASE,
            size: MMIO_ADDRESS_SIZE,
        };
        resource.append(entry.clone());
        assert_eq!(entry, resource[1]);

        let entry = Resource::MemAddressRange {
            base: MEM_ADDRESS_BASE,
            size: MEM_ADDRESS_SIZE,
        };
        resource.append(entry.clone());
        assert_eq!(entry, resource[2]);

        let entry = Resource::LegacyIrq(LEGACY_IRQ);
        resource.append(entry.clone());
        assert_eq!(entry, resource[3]);

        let entry = Resource::MsiIrq {
            ty: MsiIrqType::PciMsi,
            base: PCI_MSI_IRQ_BASE,
            size: PCI_MSI_IRQ_SIZE,
        };
        resource.append(entry.clone());
        assert_eq!(entry, resource[4]);

        let entry = Resource::MsiIrq {
            ty: MsiIrqType::PciMsix,
            base: PCI_MSIX_IRQ_BASE,
            size: PCI_MSIX_IRQ_SIZE,
        };
        resource.append(entry.clone());
        assert_eq!(entry, resource[5]);

        let entry = Resource::MsiIrq {
            ty: MsiIrqType::GenericMsi,
            base: GENERIC_MSI_IRQS_BASE,
            size: GENERIC_MSI_IRQS_SIZE,
        };
        resource.append(entry.clone());
        assert_eq!(entry, resource[6]);

        let entry = Resource::MacAddress(MAC_ADDRESS.to_string());
        resource.append(entry.clone());
        assert_eq!(entry, resource[7]);

        let entry = Resource::KvmMemSlot(KVM_SLOT_ID);
        resource.append(entry.clone());
        assert_eq!(entry, resource[8]);

        resource
    }

    #[test]
    fn get_pio_address_ranges() {
        let resources = get_device_resource();
        assert!(
            resources.get_pio_address_ranges()[0].0 == PIO_ADDRESS_BASE
                && resources.get_pio_address_ranges()[0].1 == PIO_ADDRESS_SIZE
        );
        assert_eq!(
            resources[0],
            Resource::PioAddressRange {
                base: PIO_ADDRESS_BASE,
                size: PIO_ADDRESS_SIZE,
            }
        );
        assert_ne!(resources[0], resources[1]);

        let resources2 = resources.clone();
        assert_eq!(resources.len(), resources2.len());
        drop(resources);
        assert_eq!(
            resources2[0],
            Resource::PioAddressRange {
                base: PIO_ADDRESS_BASE,
                size: PIO_ADDRESS_SIZE,
            }
        );
    }

    #[test]
    fn test_get_mmio_address_ranges() {
        let resources = get_device_resource();
        assert!(
            resources.get_mmio_address_ranges()[0].0 == MMIO_ADDRESS_BASE
                && resources.get_mmio_address_ranges()[0].1 == MMIO_ADDRESS_SIZE
        );
    }

    #[test]
    fn test_get_mem_address_ranges() {
        let resources = get_device_resource();
        assert!(
            resources.get_mem_address_ranges()[0].0 == MEM_ADDRESS_BASE
                && resources.get_mem_address_ranges()[0].1 == MEM_ADDRESS_SIZE
        );
    }

    #[test]
    fn test_get_legacy_irq() {
        let resources = get_device_resource();
        assert!(resources.get_legacy_irq().unwrap() == LEGACY_IRQ);

        // None case.
        let resources = DeviceResources::new();
        assert!(resources.get_legacy_irq().is_none());
    }

    #[test]
    fn test_get_pci_msi_irqs() {
        let resources = get_device_resource();
        assert!(
            resources.get_pci_msi_irqs().unwrap().0 == PCI_MSI_IRQ_BASE
                && resources.get_pci_msi_irqs().unwrap().1 == PCI_MSI_IRQ_SIZE
        );

        // None case.
        let resources = DeviceResources::new();
        assert!(resources.get_generic_msi_irqs().is_none());
    }

    #[test]
    fn test_get_pci_msix_irqs() {
        let resources = get_device_resource();
        assert!(
            resources.get_pci_msix_irqs().unwrap().0 == PCI_MSIX_IRQ_BASE
                && resources.get_pci_msix_irqs().unwrap().1 == PCI_MSIX_IRQ_SIZE
        );

        // None case.
        let resources = DeviceResources::new();
        assert!(resources.get_generic_msi_irqs().is_none());
    }

    #[test]
    fn test_get_generic_msi_irqs() {
        let resources = get_device_resource();
        assert!(
            resources.get_generic_msi_irqs().unwrap().0 == GENERIC_MSI_IRQS_BASE
                && resources.get_generic_msi_irqs().unwrap().1 == GENERIC_MSI_IRQS_SIZE
        );

        // None case.
        let resources = DeviceResources::new();
        assert!(resources.get_generic_msi_irqs().is_none());
    }

    #[test]
    fn test_get_mac_address() {
        let resources = get_device_resource();
        assert_eq!(resources.get_mac_address().unwrap(), MAC_ADDRESS);

        // None case.
        let resources = DeviceResources::new();
        assert!(resources.get_mac_address().is_none());
    }

    #[test]
    fn test_get_kvm_slot() {
        let resources = get_device_resource();
        assert_eq!(resources.get_kvm_mem_slots(), vec![KVM_SLOT_ID]);
    }

    #[test]
    fn test_get_all_resources() {
        let resources = get_device_resource();
        assert_eq!(resources.get_all_resources().len(), 9);
    }

    #[test]
    fn test_resource_constraint() {
        let pio = ResourceConstraint::new_pio(2);
        let pio2 = pio;
        let mmio = ResourceConstraint::new_mmio(0x1000);
        assert_eq!(pio, pio2);
        assert_ne!(pio, mmio);

        if let ResourceConstraint::PioAddress { range, align, size } =
            ResourceConstraint::new_pio(2)
        {
            assert_eq!(range, None);
            assert_eq!(align, 1);
            assert_eq!(size, 2);
        } else {
            panic!("Pio resource constraint is invalid.");
        }

        if let ResourceConstraint::PioAddress { range, align, size } =
            ResourceConstraint::pio_with_constraints(2, Some((15, 16)), 2)
        {
            assert_eq!(range, Some((15, 16)));
            assert_eq!(align, 2);
            assert_eq!(size, 2);
        } else {
            panic!("Pio resource constraint is invalid.");
        }

        if let ResourceConstraint::MmioAddress { range, align, size } =
            ResourceConstraint::new_mmio(0x2000)
        {
            assert_eq!(range, None);
            assert_eq!(align, 0x1000);
            assert_eq!(size, 0x2000);
        } else {
            panic!("Mmio resource constraint is invalid.");
        }

        if let ResourceConstraint::MmioAddress { range, align, size } =
            ResourceConstraint::mmio_with_constraints(0x2000, Some((0x0, 0x2000)), 0x2000)
        {
            assert_eq!(range, Some((0x0, 0x2000)));
            assert_eq!(align, 0x2000);
            assert_eq!(size, 0x2000);
        } else {
            panic!("Mmio resource constraint is invalid.");
        }

        if let ResourceConstraint::MemAddress { range, align, size } =
            ResourceConstraint::new_mem(0x2000)
        {
            assert_eq!(range, None);
            assert_eq!(align, 0x1000);
            assert_eq!(size, 0x2000);
        } else {
            panic!("Mem resource constraint is invalid.");
        }

        if let ResourceConstraint::MemAddress { range, align, size } =
            ResourceConstraint::mem_with_constraints(0x2000, Some((0x0, 0x2000)), 0x2000)
        {
            assert_eq!(range, Some((0x0, 0x2000)));
            assert_eq!(align, 0x2000);
            assert_eq!(size, 0x2000);
        } else {
            panic!("Mem resource constraint is invalid.");
        }

        if let ResourceConstraint::LegacyIrq { irq } =
            ResourceConstraint::new_legacy_irq(Some(0x123))
        {
            assert_eq!(irq, Some(0x123));
        } else {
            panic!("IRQ resource constraint is invalid.");
        }

        if let ResourceConstraint::PciMsiIrq { size } = ResourceConstraint::new_pci_msi_irq(0x123) {
            assert_eq!(size, 0x123);
        } else {
            panic!("Pci MSI irq resource constraint is invalid.");
        }

        if let ResourceConstraint::PciMsixIrq { size } = ResourceConstraint::new_pci_msix_irq(0x123)
        {
            assert_eq!(size, 0x123);
        } else {
            panic!("Pci MSIx irq resource constraint is invalid.");
        }

        if let ResourceConstraint::GenericIrq { size } = ResourceConstraint::new_generic_irq(0x123)
        {
            assert_eq!(size, 0x123);
        } else {
            panic!("generic irq resource constraint is invalid.");
        }

        if let ResourceConstraint::KvmMemSlot { slot, size } =
            ResourceConstraint::new_kvm_mem_slot(0x1000, Some(0x2000))
        {
            assert_eq!(slot, Some(0x2000));
            assert_eq!(size, 0x1000);
        } else {
            panic!("KVM slot resource constraint is invalid.");
        }
    }

    #[test]
    fn test_resources_deref() {
        let resources = get_device_resource();
        let mut count = 0;
        for _res in resources.iter() {
            count += 1;
        }
        assert_eq!(count, resources.0.len());
    }
}
