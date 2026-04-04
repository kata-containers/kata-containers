// Copyright (c) 2024 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
mod devices;
mod pci_manager;

pub use devices::calc_fw_cfg_mmio64_mb;
pub use devices::get_bars_max_addressable_memory;
