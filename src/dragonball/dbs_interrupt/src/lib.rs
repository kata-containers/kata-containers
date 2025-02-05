// Copyright (C) 2019-2020 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Traits and Structs to manage interrupt sources for devices.
//!
//! Software indicating an event that needs immediate attention. An interrupt alerts the processor
//! to a high-priority condition requiring the interruption of the current code the processor is
//! executing. The processor responds by suspending its current activities, saving its state, and
//! executing a function called an interrupt handler (or an interrupt service routine, ISR) to deal
//! with the event. This interruption is temporary, and, after the interrupt handler finishes,
//! unless handling the interrupt has emitted a fatal error, the processor resumes normal
//! activities.
//!
//! Hardware interrupts are used by devices to communicate that they require attention from the
//! operating system, or a bare-metal program running on the CPU if there are no OSes. The act of
//! initiating a hardware interrupt is referred to as an interrupt request (IRQ). Different devices
//! are usually associated with different interrupts using a unique value associated with each
//! interrupt. This makes it possible to know which hardware device caused which interrupts. These
//! interrupt values are often called IRQ lines, or just interrupt lines.
//!
//! Nowadays, IRQ lines is not the only mechanism to deliver device interrupts to processors. MSI
//! [(Message Signaled Interrupt)](https://en.wikipedia.org/wiki/Message_Signaled_Interrupts) is
//! another commonly used alternative in-band method of signaling an interrupt, using special
//! in-band messages to replace traditional out-of-band assertion of dedicated interrupt lines.
//! While more complex to implement in a device, message signaled interrupts have some significant
//! advantages over pin-based out-of-band interrupt signaling. Message signaled interrupts are
//! supported in PCI bus since its version 2.2, and in later available PCI Express bus. Some non-PCI
//! architectures also use message signaled interrupts.
//!
//! While IRQ is a term commonly used by Operating Systems when dealing with hardware interrupts,
//! the IRQ numbers managed by OSes are independent of the ones managed by VMM. For simplicity sake,
//! the term `Interrupt Source` is used instead of IRQ to represent both pin-based interrupts and
//! MSI interrupts.
//!
//! A device may support multiple types of interrupts, and each type of interrupt may support one or
//! multiple interrupt sources. For example, a PCI device may support:
//! * Legacy Irq: exactly one interrupt source.
//! * PCI MSI Irq: 1,2,4,8,16,32 interrupt sources.
//! * PCI MSIx Irq: 2^n(n=0-11) interrupt sources.
//!
//! A distinct Interrupt Source Identifier (ISID) will be assigned to each interrupt source. An ID
//! allocator will be used to allocate and free Interrupt Source Identifiers for devices. To
//! decouple this crate from the ID allocator, here we doesn't take the responsibility to
//! allocate/free Interrupt Source IDs but only makes use of assigned IDs.
//!
//! The overall flow to deal with interrupts is:
//! * the VMM creates an interrupt manager
//! * the VMM creates a device manager, passing on an reference to the interrupt manager
//! * the device manager passes on an reference to the interrupt manager to all registered devices
//! * guest kernel loads drivers for virtual devices
//! * guest device driver determines the type and number of interrupts needed, and update the device
//!   configuration
//! * the virtual device backend requests the interrupt manager to create an interrupt group
//!   according to guest configuration information

use std::io::Error;
use std::ops::Deref;
use std::sync::Arc;

use vmm_sys_util::eventfd::EventFd;

mod manager;
pub use manager::MSI_DEVICE_ID_SHIFT;
pub use manager::{DeviceInterruptManager, DeviceInterruptMode, InterruptStatusRegister32};

mod notifier;
pub use self::notifier::*;

#[cfg(feature = "kvm-irq")]
pub mod kvm;
#[cfg(feature = "kvm-irq")]
pub use self::kvm::KvmIrqManager;

/// Reuse std::io::Result to simplify interoperability among crates.
pub type Result<T> = std::io::Result<T>;

/// Data type to store an interrupt source identifier.
pub type InterruptIndex = u32;

/// Type of interrupt source.
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum InterruptSourceType {
    #[cfg(feature = "legacy-irq")]
    /// Legacy Pin-based Interrupt.
    /// On x86 platforms, legacy interrupts are routed through 8259 PICs and/or IOAPICs.
    LegacyIrq,
    #[cfg(feature = "msi-irq")]
    /// Message Signaled Interrupt (PCI MSI/PCI MSIx etc).
    /// Some non-PCI devices (like HPET on x86) make use of generic MSI in platform specific ways.
    MsiIrq,
}

/// Configuration data for an interrupt source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterruptSourceConfig {
    #[cfg(feature = "legacy-irq")]
    /// Configuration data for Legacy interrupts.
    LegacyIrq(LegacyIrqSourceConfig),
    #[cfg(feature = "msi-irq")]
    /// Configuration data for PciMsi, PciMsix and generic MSI interrupts.
    MsiIrq(MsiIrqSourceConfig),
}

/// Configuration data for legacy interrupts.
///
/// On x86 platforms, legacy interrupts means those interrupts routed through PICs or IOAPICs.
#[cfg(feature = "legacy-irq")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyIrqSourceConfig {}

/// Configuration data for GenericMsi, PciMsi, PciMsix interrupts.
#[cfg(feature = "msi-irq")]
#[derive(Default, Clone, Debug, Eq, PartialEq)]
pub struct MsiIrqSourceConfig {
    /// High address to deliver message signaled interrupt.
    pub high_addr: u32,
    /// Low address to deliver message signaled interrupt.
    pub low_addr: u32,
    /// Data to write to deliver message signaled interrupt.
    pub data: u32,
    /// Interrupt control state.
    pub msg_ctl: u32,
    /// Device id indicate the device who triggers this msi irq.
    pub device_id: Option<u32>,
}

/// Trait to manage interrupt sources for virtual device backends.
///
/// The InterruptManager implementations should protect itself from concurrent accesses internally,
/// so it could be invoked from multi-threaded context.
pub trait InterruptManager {
    /// Create an [InterruptSourceGroup](trait.InterruptSourceGroup.html) object to manage interrupt
    /// sources for a virtual device.
    ///
    /// An [InterruptSourceGroup](trait.InterruptSourceGroup.html) object manages all interrupt
    /// sources of the same type for a virtual device.
    ///
    /// # Arguments
    /// * type_: type of interrupt source.
    /// * base: base Interrupt Source ID to be managed by the group object.
    /// * count: number of Interrupt Sources to be managed by the group object.
    fn create_group(
        &self,
        type_: InterruptSourceType,
        base: InterruptIndex,
        count: InterruptIndex,
    ) -> Result<Arc<Box<dyn InterruptSourceGroup>>>;

    /// Destroy an [InterruptSourceGroup](trait.InterruptSourceGroup.html) object created by
    /// [create_group()](trait.InterruptManager.html#tymethod.create_group).
    ///
    /// Assume the caller takes the responsibility to disable all interrupt sources of the group
    /// before calling destroy_group(). This assumption helps to simplify InterruptSourceGroup
    /// implementations.
    fn destroy_group(&self, group: Arc<Box<dyn InterruptSourceGroup>>) -> Result<()>;
}

impl<T: InterruptManager> InterruptManager for Arc<T> {
    fn create_group(
        &self,
        type_: InterruptSourceType,
        base: u32,
        count: u32,
    ) -> std::result::Result<Arc<Box<dyn InterruptSourceGroup>>, Error> {
        self.deref().create_group(type_, base, count)
    }

    fn destroy_group(
        &self,
        group: Arc<Box<dyn InterruptSourceGroup>>,
    ) -> std::result::Result<(), Error> {
        self.deref().destroy_group(group)
    }
}

/// Trait to manage a group of interrupt sources for a device.
///
/// A device may support several types of interrupts, and each type of interrupt may contain one or
/// multiple continuous interrupt sources. For example, a PCI device may concurrently support:
/// * Legacy Irq: exactly one interrupt source.
/// * PCI MSI Irq: 1,2,4,8,16,32 interrupt sources.
/// * PCI MSIx Irq: 2^n(n=0-11) interrupt sources.
///
/// PCI MSI interrupts of a device may not be configured individually, and must configured as a
/// whole block. So all interrupts of the same type of a device are abstracted as an
/// [InterruptSourceGroup](trait.InterruptSourceGroup.html) object, instead of abstracting each
/// interrupt source as a distinct InterruptSource.
#[allow(clippy::len_without_is_empty)]
pub trait InterruptSourceGroup: Send + Sync {
    /// Get type of interrupt sources managed by the group.
    fn interrupt_type(&self) -> InterruptSourceType;

    /// Get number of interrupt sources managed by the group.
    fn len(&self) -> InterruptIndex;

    /// Get base of the assigned Interrupt Source Identifiers.
    fn base(&self) -> InterruptIndex;

    /// Enable the interrupt sources in the group to generate interrupts.
    fn enable(&self, configs: &[InterruptSourceConfig]) -> Result<()>;

    /// Disable the interrupt sources in the group to generate interrupts.
    fn disable(&self) -> Result<()>;

    /// Update the interrupt source group configuration.
    ///
    /// # Arguments
    /// * index: sub-index into the group.
    /// * config: configuration data for the interrupt source.
    fn update(&self, index: InterruptIndex, config: &InterruptSourceConfig) -> Result<()>;

    /// Returns an interrupt notifier from this interrupt.
    ///
    /// An interrupt notifier allows for external components and processes to inject interrupts into
    /// guest, by writing to the file returned by this method.
    fn notifier(&self, _index: InterruptIndex) -> Option<&EventFd> {
        None
    }

    /// Inject an interrupt from this interrupt source into the guest.
    ///
    /// If the interrupt has an associated `interrupt_status` register, all bits set in `flag` will
    /// be atomically ORed into the `interrupt_status` register.
    fn trigger(&self, index: InterruptIndex) -> Result<()>;

    /// Mask an interrupt from this interrupt source.
    fn mask(&self, _index: InterruptIndex) -> Result<()> {
        // Not all interrupt sources can be disabled.
        // To accommodate this, we can have a no-op here.
        Ok(())
    }

    /// Unmask an interrupt from this interrupt source.
    fn unmask(&self, _index: InterruptIndex) -> Result<()> {
        // Not all interrupt sources can be disabled.
        // To accommodate this, we can have a no-op here.
        Ok(())
    }

    /// Check whether there's pending interrupt.
    fn get_pending_state(&self, _index: InterruptIndex) -> bool {
        false
    }
}

impl PartialEq for dyn InterruptSourceGroup {
    fn eq(&self, other: &dyn InterruptSourceGroup) -> bool {
        self.interrupt_type() == other.interrupt_type()
            && self.len() == other.len()
            && self.base() == other.base()
    }
}
