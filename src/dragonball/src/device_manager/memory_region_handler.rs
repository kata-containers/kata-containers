// Copyright 2022 Alibaba, Inc. or its affiliates. All Rights Reserved.
//
// SPDX-License-Identifier: Apache-2.0

use std::io;
use std::sync::Arc;

use dbs_address_space::{AddressSpace, AddressSpaceRegion, AddressSpaceRegionType};
use dbs_virtio_devices::{Error as VirtioError, VirtioRegionHandler};
use log::{debug, error};
use vm_memory::{FileOffset, GuestAddressSpace, GuestMemoryRegion, GuestRegionMmap};

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
///   AddressSpaceRegionType::ReservedMemory type, in this way, address_space will know that
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

impl DeviceVirtioRegionHandler {
    fn insert_address_space(
        &mut self,
        region: Arc<GuestRegionMmap>,
    ) -> std::result::Result<(), VirtioError> {
        let file_offset = match region.file_offset() {
            // TODO: use from_arc
            Some(f) => Some(FileOffset::new(f.file().try_clone()?, 0)),
            None => None,
        };

        let as_region = Arc::new(AddressSpaceRegion::build(
            AddressSpaceRegionType::DAXMemory,
            region.start_addr(),
            region.size() as u64,
            None,
            file_offset,
            region.flags(),
            region.prot(),
            false,
        ));

        self.address_space.insert_region(as_region).map_err(|e| {
            error!("inserting address apace error: {}", e);
            // dbs-virtio-devices should not depend on dbs-address-space.
            // So here io::Error is used instead of AddressSpaceError directly.
            VirtioError::IOError(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "invalid address space region ({0:#x}, {1:#x})",
                    region.start_addr().0,
                    region.len()
                ),
            ))
        })?;
        Ok(())
    }

    fn insert_vm_as(
        &mut self,
        region: Arc<GuestRegionMmap>,
    ) -> std::result::Result<(), VirtioError> {
        let vm_as_new = self.vm_as.memory().insert_region(region).map_err(|e| {
            error!(
                "DeviceVirtioRegionHandler failed to insert guest memory region: {:?}.",
                e
            );
            VirtioError::InsertMmap(e)
        })?;
        // Do not expect poisoned lock here, so safe to unwrap().
        self.vm_as.lock().unwrap().replace(vm_as_new);

        Ok(())
    }
}

impl VirtioRegionHandler for DeviceVirtioRegionHandler {
    fn insert_region(
        &mut self,
        region: Arc<GuestRegionMmap>,
    ) -> std::result::Result<(), VirtioError> {
        debug!(
            "add geust memory region to address_space/vm_as, new region: {:?}",
            region
        );

        self.insert_address_space(region.clone())?;
        self.insert_vm_as(region)?;

        Ok(())
    }
}
