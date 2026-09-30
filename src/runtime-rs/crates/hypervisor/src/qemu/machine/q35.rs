// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use super::platform::BaseMachine;

pub(crate) struct Q35 {
    pub base: BaseMachine,
    pub kernel_irqchip: Option<String>,
    /// Global IOMMU for Q35. Emitted as a top-level -device intel-iommu,
    /// not attached to any pxb-pcie. Contrast with BusIommu on PciRootComplex.
    pub intel_iommu: Option<IntelIommuConfig>,
}

pub(crate) struct IntelIommuConfig {
    pub intremap: bool,
    pub caching_mode: bool,
}
