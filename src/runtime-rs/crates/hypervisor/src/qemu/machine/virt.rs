// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use super::platform::BaseMachine;

pub(crate) struct Virt {
    pub base: BaseMachine,
    pub gic_version: Option<u8>,
    pub ras: bool,
    /// Highmem MMIO window size in bytes; must be a power of 2.
    /// Required for Grace GPU passthrough: 4T for GH200/GB200 with <=4
    /// GPUs, 8T for GB300 NVL72 with 4 GPUs.
    pub highmem_mmio_size: Option<u64>,
}
