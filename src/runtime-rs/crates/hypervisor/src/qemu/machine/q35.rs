// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use super::platform::BaseMachine;

pub(crate) struct Q35 {
    pub base: BaseMachine,
    /// `kernel_irqchip=split` is required for CoCo (SNP/TDX); absent on vanilla Q35.
    pub kernel_irqchip: Option<String>,
    /// References the id of `Objects::protection`.  Set by `apply_host_defaults`
    /// when `HostTopology::protection` is `Some`.
    pub confidential_guest_support: Option<String>,
    /// Not bus-attached; contrast with BusIommu on PciRootComplex.
    pub intel_iommu: Option<IntelIommuConfig>,
    // pub runtime: RuntimeFeatures,  -- Phase 3+
}

pub(crate) struct IntelIommuConfig {
    pub intremap: bool,
    pub caching_mode: bool,
}
