// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

/// Volume to support dm-verity over block devices.
pub const KATA_VOLUME_TYPE_DMVERITY: &str = "dmverity";

/// Key to identify dmverity information in `Storage.driver_options`
pub const KATA_VOLUME_DMVERITY_OPTION_VERITY_INFO: &str = "verity_info";
/// Key to identify type of source device in `Storage.driver_options`
pub const KATA_VOLUME_DMVERITY_OPTION_SOURCE_TYPE: &str = "source_type";
/// Source device of dmverity volume is a Virtio PCI device
pub const KATA_VOLUME_DMVERITY_SOURCE_TYPE_VIRTIO_PCI: &str = "virtio_pci";
/// Source device of dmverity volume is a Virtio MMIO device
pub const KATA_VOLUME_DMVERITY_SOURCE_TYPE_VIRTIO_MMIO: &str = "virtio_mmio";
/// Source device of dmverity volume is a Virtio CCW device
pub const KATA_VOLUME_DMVERITY_SOURCE_TYPE_VIRTIO_CCW: &str = "virtio_ccw";
/// Source device of dmverity volume is a SCSI disk.
pub const KATA_VOLUME_DMVERITY_SOURCE_TYPE_SCSI: &str = "scsi";
/// Source device of dmverity volume is a pmem disk.
pub const KATA_VOLUME_DMVERITY_SOURCE_TYPE_PMEM: &str = "pmem";

/// Key to indentify directory creation in `Storage.driver_options`.
pub const KATA_VOLUME_OVERLAYFS_CREATE_DIR: &str =
    "io.katacontainers.volume.overlayfs.create_directory";
