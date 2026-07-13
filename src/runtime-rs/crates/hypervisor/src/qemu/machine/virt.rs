// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use super::platform::BaseMachine;

pub(crate) struct Virt {
    pub base: BaseMachine,
    pub gic_version: Option<u8>,
    pub ras: bool,
    /// Required for Grace GPU passthrough; must be a power of 2.
    /// 4T for GH200/GB200 with <=4 GPUs, 8T for GB300 NVL72 with 4 GPUs.
    pub highmem_mmio_size: Option<u64>,
}
