// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use bitmask_enum::bitmask;

/// CapabilityBits
#[bitmask(u8)]
pub enum CapabilityBits {
    /// hypervisor supports use block device
    BlockDeviceSupport,
    /// hypervisor supports block device hotplug
    BlockDeviceHotplugSupport,
    /// hypervisor supports multi queue
    MultiQueueSupport,
    /// hypervisor supports filesystem share
    FsSharingSupport,
    /// hypervisor supports hybrid-vsock
    HybridVsockSupport,
    /// hypervisor supports memory hotplug probe interface
    GuestMemoryProbe,
}

/// Capabilities describe a virtcontainers hypervisor capabilities through a bit mask.
#[derive(Debug, Clone)]
pub struct Capabilities {
    /// Capability flags
    flags: CapabilityBits,
}

impl Default for Capabilities {
    fn default() -> Self {
        Self::new()
    }
}

impl Capabilities {
    /// new Capabilities struct
    pub fn new() -> Self {
        Capabilities {
            flags: CapabilityBits { bits: 0 },
        }
    }

    /// set CapabilityBits
    pub fn set(&mut self, flags: CapabilityBits) {
        self.flags = flags;
    }

    /// add CapabilityBits
    pub fn add(&mut self, flags: CapabilityBits) {
        self.flags |= flags;
    }

    /// is_block_device_supported tells if the hypervisor supports block devices.
    pub fn is_block_device_supported(&self) -> bool {
        self.flags.and(CapabilityBits::BlockDeviceSupport) != 0
    }

    /// is_block_device_hotplug_supported tells if the hypervisor supports block devices.
    pub fn is_block_device_hotplug_supported(&self) -> bool {
        self.flags.and(CapabilityBits::BlockDeviceHotplugSupport) != 0
    }

    /// is_multi_queue_supported tells if the hypervisor supports device multi queue support.
    pub fn is_multi_queue_supported(&self) -> bool {
        self.flags.and(CapabilityBits::MultiQueueSupport) != 0
    }

    /// is_hybrid_vsock_supported tells if an hypervisor supports hybrid-vsock support.
    pub fn is_hybrid_vsock_supported(&self) -> bool {
        self.flags.and(CapabilityBits::HybridVsockSupport) != 0
    }

    /// is_fs_sharing_supported tells if an hypervisor supports host filesystem sharing.
    pub fn is_fs_sharing_supported(&self) -> bool {
        self.flags.and(CapabilityBits::FsSharingSupport) != 0
    }

    /// is_mem_hotplug_probe_supported tells if the hypervisor supports hotplug probe interface
    pub fn is_mem_hotplug_probe_supported(&self) -> bool {
        self.flags.and(CapabilityBits::GuestMemoryProbe) != 0
    }
}

#[cfg(test)]
mod tests {
    use crate::capabilities::CapabilityBits;

    use super::Capabilities;

    #[test]
    fn test_set_hypervisor_capabilities() {
        let mut cap = Capabilities::new();
        assert!(!cap.is_block_device_supported());

        // test legacy vsock support
        assert!(!cap.is_hybrid_vsock_supported());

        // test set block device support
        cap.set(CapabilityBits::BlockDeviceSupport);
        assert!(cap.is_block_device_supported());
        assert!(!cap.is_block_device_hotplug_supported());

        // test set block device hotplug support
        cap.set(CapabilityBits::BlockDeviceSupport | CapabilityBits::BlockDeviceHotplugSupport);
        assert!(cap.is_block_device_hotplug_supported());
        assert!(!cap.is_multi_queue_supported());

        // test set multi queue support
        cap.set(
            CapabilityBits::BlockDeviceSupport
                | CapabilityBits::BlockDeviceHotplugSupport
                | CapabilityBits::MultiQueueSupport,
        );
        assert!(cap.is_multi_queue_supported());

        // test set host filesystem sharing support
        cap.set(
            CapabilityBits::BlockDeviceSupport
                | CapabilityBits::BlockDeviceHotplugSupport
                | CapabilityBits::MultiQueueSupport
                | CapabilityBits::FsSharingSupport,
        );
        assert!(cap.is_fs_sharing_supported());

        // test set hybrid-vsock support
        cap.add(CapabilityBits::HybridVsockSupport);
        assert!(cap.is_hybrid_vsock_supported());
        // test append capabilities
        cap.add(CapabilityBits::GuestMemoryProbe);
        assert!(cap.is_mem_hotplug_probe_supported());
        assert!(cap.is_fs_sharing_supported());
    }
}
