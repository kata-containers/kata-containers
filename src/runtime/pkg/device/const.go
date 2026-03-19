// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package device

const (
	IommufdDevPath = "/dev/vfio/devices"

	// VfioNoIOMMUPrefix is the prefix used by the kernel for VFIO group device
	// files when enable_unsafe_noiommu_mode is active. In this mode devices appear
	// as /dev/vfio/noiommu-<GROUP> instead of /dev/vfio/<GROUP>, and IOMMUFD
	// cannot be used.
	VfioNoIOMMUPrefix = "noiommu-"
)
