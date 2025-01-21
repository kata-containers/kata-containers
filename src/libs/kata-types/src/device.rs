// Copyright (c) 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::handler::HandlerManager;

/// DRIVER_BLK_PCI_TYPE is the device driver for virtio-blk
pub const DRIVER_BLK_PCI_TYPE: &str = "blk";
/// DRIVER_BLK_CCW_TYPE is the device driver for virtio-blk-ccw
pub const DRIVER_BLK_CCW_TYPE: &str = "blk-ccw";
/// DRIVER_BLK_MMIO_TYPE is the device driver for virtio-mmio
pub const DRIVER_BLK_MMIO_TYPE: &str = "mmioblk";
/// DRIVER_SCSI_TYPE is the device driver for virtio-scsi
pub const DRIVER_SCSI_TYPE: &str = "scsi";
/// DRIVER_NVDIMM_TYPE is the device driver for nvdimm
pub const DRIVER_NVDIMM_TYPE: &str = "nvdimm";
/// DRIVER_VFIO_PCI_GK_TYPE is the device driver for vfio-pci 
/// while the device will be bound to a guest kernel driver
pub const DRIVER_VFIO_PCI_GK_TYPE: &str = "vfio-pci-gk";
/// DRIVER_VFIO_PCI_TYPE is the device driver for vfio-pci
/// VFIO PCI device to be bound to vfio-pci and made available inside the
/// container as a VFIO device node
pub const DRIVER_VFIO_PCI_TYPE: &str = "vfio-pci";
/// DRIVER_VFIO_AP_TYPE is the device driver for vfio-ap hotplug.
pub const DRIVER_VFIO_AP_TYPE: &str = "vfio-ap";
/// DRIVER_VFIO_AP_COLD_TYPE is the device driver for vfio-ap coldplug.
pub const DRIVER_VFIO_AP_COLD_TYPE: &str = "vfio-ap-cold";

/// DRIVER_9P_TYPE is the driver for 9pfs volume.
pub const DRIVER_9P_TYPE: &str = "9p";
/// DRIVER_EPHEMERAL_TYPE is the driver for ephemeral volume.
pub const DRIVER_EPHEMERAL_TYPE: &str = "ephemeral";
/// DRIVER_LOCAL_TYPE is the driver for local volume.
pub const DRIVER_LOCAL_TYPE: &str = "local";
/// DRIVER_OVERLAYFS_TYPE is the driver for overlayfs volume.
pub const DRIVER_OVERLAYFS_TYPE: &str = "overlayfs";
/// DRIVER_VIRTIOFS_TYPE is the driver for virtio-fs volume.
pub const DRIVER_VIRTIOFS_TYPE: &str = "virtio-fs";
/// DRIVER_VIRTIOFS_TYPE is the driver for Bind watch volume.
pub const DRIVER_WATCHABLE_BIND_TYPE: &str = "watchable-bind";

/// Manager to manage registered device handlers.
pub type DeviceHandlerManager<H> = HandlerManager<H>;
