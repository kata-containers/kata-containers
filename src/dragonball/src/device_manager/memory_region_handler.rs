// Copyright 2022 Alibaba, Inc. or its affiliates. All Rights Reserved.
//
// SPDX-License-Identifier: Apache-2.0

use std::io;
use std::sync::Arc;

use dbs_address_space::{AddressSpace, AddressSpaceRegion, AddressSpaceRegionType};
use dbs_virtio_devices::{Error as VirtioError, VirtioRegionHandler};
use log::{debug, error};
use vm_memory::{GuestAddressSpace, GuestMemoryRegion, GuestRegionMmap};

use crate::address_space_manager::GuestAddressSpaceImpl;

/// This struct implements the VirtioRegionHandler trait, which inserts the memory
/// region of the virtio device into vm_as and address_space.
///
/// * After region is inserted into the vm_as, the virtio device can read guest memory
///   data using vm_as.get_slice with GuestAddress.
///
/// * Insert virtio memory into address_space so that the correct guest last address can
///   be found when initializing the e820 table. The e820 table is a table that describes
///   guest memory prepared before the guest startup. we need to config the correct guest
///   memory address and length in the table. The virtio device memory belongs to the MMIO
///   space and does not belong to the Guest Memory space. Therefore, it cannot be configured
///   into the e820 table. When creating AddressSpaceRegion we use
///   AddressSpaceRegionType::DAXMemory type, in this way, address_space will know that
///   this region a special memory, it will don't put the this memory in e820 table.
///
/// This function relies on the atomic-guest-memory feature. Without this feature enabled, memory
/// regions cannot be inserted into vm_as. Because the insert_region interface of vm_as does
/// not insert regions in place, but returns an array of inserted regions. We need to manually
/// replace this array of regions with vm_as, and that's what atomic-guest-memory feature does.
/// So we rely on the atomic-guest-memory feature here
pub struct DeviceVirtioRegionHandler {
    pub(crate) vm_as: GuestAddressSpaceImpl,
    pub(crate) address_space: AddressSpace,
}

impl VirtioRegionHandler for DeviceVirtioRegionHandler {
    fn insert_regions(
        &mut self,
        regions: Vec<Arc<GuestRegionMmap>>,
    ) -> std::result::Result<(), VirtioError> {
        let guard = self.vm_as.lock().unwrap();
        let mut memory = (*self.vm_as.memory()).clone();
        let mut address_regions = Vec::with_capacity(regions.len());
        for region in regions {
            debug!("add guest device memory region: {region:?}");
            address_regions.push(Arc::new(AddressSpaceRegion::build(
                AddressSpaceRegionType::DAXMemory,
                region.start_addr(),
                region.len(),
                None,
                region.file_offset().cloned(),
                region.flags(),
                region.prot(),
                false,
            )));
            memory = memory.insert_region(region).map_err(|e| {
                error!("failed to insert guest device memory region: {e:?}");
                VirtioError::InsertMmap
            })?;
        }
        // Validate the whole batch before publishing either address space.
        self.address_space
            .insert_regions(address_regions)
            .map_err(|e| {
                VirtioError::IOError(io::Error::other(format!(
                    "invalid device address space: {e}"
                )))
            })?;
        guard.replace(memory);
        Ok(())
    }

    fn remove_regions(
        &mut self,
        regions: &[Arc<GuestRegionMmap>],
    ) -> std::result::Result<(), VirtioError> {
        let guard = self.vm_as.lock().unwrap();
        let mut memory = (*self.vm_as.memory()).clone();
        for region in regions {
            let (updated, _) = memory
                .remove_region(region.start_addr(), region.len())
                .map_err(|e| {
                    error!("failed to remove guest device memory region: {e:?}");
                    VirtioError::InsertMmap
                })?;
            memory = updated;
        }
        let address_regions = regions
            .iter()
            .map(|region| (region.start_addr(), region.len()))
            .collect::<Vec<_>>();
        self.address_space
            .remove_regions(&address_regions)
            .map_err(|e| VirtioError::IOError(io::Error::other(e.to_string())))?;
        guard.replace(memory);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dbs_address_space::{AddressSpaceLayout, AddressSpaceRegionType};
    use vm_memory::{GuestAddress, GuestMemory, GuestMemoryMmap, MmapRegion};

    #[test]
    fn test_insert_and_remove_regions() {
        let vm_as = AddressSpace::convert_into_vm_as(
            GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x1000)]).unwrap(),
        );
        let layout = AddressSpaceLayout::new((1 << 46) - 1, 0, 0x1fff);
        let address_space = AddressSpace::from_regions(
            vec![Arc::new(AddressSpaceRegion::new(
                AddressSpaceRegionType::DefaultMemory,
                GuestAddress(0),
                0x1000,
            ))],
            layout,
        );
        let mut handler = DeviceVirtioRegionHandler {
            vm_as: vm_as.clone(),
            address_space: address_space.clone(),
        };
        let region = Arc::new(
            GuestRegionMmap::new(MmapRegion::new(0x1000).unwrap(), GuestAddress(0x2000)).unwrap(),
        );

        handler.insert_regions(vec![region.clone()]).unwrap();
        assert!(vm_as.memory().find_region(GuestAddress(0x2000)).is_some());
        assert!(address_space.is_dax_region(GuestAddress(0x2000)));

        handler.remove_regions(&[region]).unwrap();
        assert!(vm_as.memory().find_region(GuestAddress(0x2000)).is_none());
        assert!(!address_space.is_dax_region(GuestAddress(0x2000)));
    }
}
