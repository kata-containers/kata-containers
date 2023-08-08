// Copyright (C) 2019 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//
// Portions Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

//! Implementations of the Virtio MMIO Transport Layer.
//!
//! The Virtio specifications have defined two versions for the Virtio MMIO transport layer. The
//! version 1 is called legacy mode, and the version 2 is preferred currently. The common parts
//! of both versions are defined here.

mod mmio_state;
pub use self::mmio_state::*;

mod mmio_v2;
pub use self::mmio_v2::*;

mod dragonball;
pub use self::dragonball::*;

/// Magic number for MMIO virtio devices.
/// Required by the virtio mmio device register layout at offset 0 from base
pub const MMIO_MAGIC_VALUE: u32 = 0x74726976;

/// Version number for legacy MMIO virito devices.
pub const MMIO_VERSION_1: u32 = 1;

/// Current version specified by the mmio standard.
pub const MMIO_VERSION_2: u32 = 2;

/// Offset from the base MMIO address of a virtio device used by the guest to notify the device of
/// queue events.
pub const MMIO_NOTIFY_REG_OFFSET: u32 = 0x50;

/// Default size for MMIO device configuration address space.
///
/// This represents the size of the mmio device specified to the kernel as a cmdline option
/// It has to be larger than 0x100 (the offset where the configuration space starts from
/// the beginning of the memory mapped device registers) + the size of the configuration space
/// Currently hardcoded to 4K
pub const MMIO_DEFAULT_CFG_SIZE: u64 = 0x1000;

///
/// Control registers

// Magic value ("virt" string) - Read Only
pub const REG_MMIO_MAGIC_VALUE: u64 = 0x000;

// Virtio device version - Read Only
pub const REG_MMIO_VERSION: u64 = 0x004;

// Virtio device ID - Read Only
pub const REG_MMIO_DEVICE_ID: u64 = 0x008;

// Virtio vendor ID - Read Only
pub const REG_MMIO_VENDOR_ID: u64 = 0x00c;

// Bitmask of the features supported by the device (host)
// (32 bits per set) - Read Only
pub const REG_MMIO_DEVICE_FEATURE: u64 = 0x010;

// Device (host) features set selector - Write Only
pub const REG_MMIO_DEVICE_FEATURES_S: u64 = 0x014;

// Bitmask of features activated by the driver (guest)
//  (32 bits per set) - Write Only
pub const REG_MMIO_DRIVER_FEATURE: u64 = 0x020;

// Activated features set selector - Write Only */
pub const REG_MMIO_DRIVER_FEATURES_S: u64 = 0x024;

// Guest's memory page size in bytes - Write Only
pub const REG_MMIO_GUEST_PAGE_SIZ: u64 = 0x028;

// Queue selector - Write Only
pub const REG_MMIO_QUEUE_SEL: u64 = 0x030;

// Maximum size of the currently selected queue - Read Only
pub const REG_MMIO_QUEUE_NUM_MA: u64 = 0x034;

// Queue size for the currently selected queue - Write Only
pub const REG_MMIO_QUEUE_NUM: u64 = 0x038;

// Used Ring alignment for the currently selected queue - Write Only
pub const REG_MMIO_QUEUE_ALIGN: u64 = 0x03c;

// Guest's PFN for the currently selected queue - Read Write
pub const REG_MMIO_QUEUE_PFN: u64 = 0x040;

// Ready bit for the currently selected queue - Read Write
pub const REG_MMIO_QUEUE_READY: u64 = 0x044;

// Queue notifier - Write Only
pub const REG_MMIO_QUEUE_NOTIF: u64 = 0x050;

// Interrupt status - Read Only
pub const REG_MMIO_INTERRUPT_STAT: u64 = 0x060;

// Interrupt acknowledge - Write Only
pub const REG_MMIO_INTERRUPT_AC: u64 = 0x064;

// Device status register - Read Write
pub const REG_MMIO_STATUS: u64 = 0x070;

// Selected queue's Descriptor Table address, 64 bits in two halves
pub const REG_MMIO_QUEUE_DESC_LOW: u64 = 0x080;
pub const REG_MMIO_QUEUE_DESC_HIGH: u64 = 0x084;

// Selected queue's Available Ring address, 64 bits in two halves
pub const REG_MMIO_QUEUE_AVAIL_LOW: u64 = 0x090;
pub const REG_MMIO_QUEUE_AVAIL_HIGH: u64 = 0x094;

// Selected queue's Used Ring address, 64 bits in two halves
pub const REG_MMIO_QUEUE_USED_LOW: u64 = 0x0a0;
pub const REG_MMIO_QUEUE_USED_HIGH: u64 = 0x0a4;

// Shared memory region id
pub const REG_MMIO_SHM_SEL: u64 = 0x0ac;

// Shared memory region length, 64 bits in two halves
pub const REG_MMIO_SHM_LEN_LOW: u64 = 0x0b0;
pub const REG_MMIO_SHM_LEN_HIGH: u64 = 0x0b4;

// Shared memory region base address, 64 bits in two halves
pub const REG_MMIO_SHM_BASE_LOW: u64 = 0x0b8;
pub const REG_MMIO_SHM_BASE_HIGH: u64 = 0x0bc;

// Configuration atomicity value
pub const REG_MMIO_CONFIG_GENERATI: u64 = 0x0fc;

// The config space is defined by each driver
// the per-driver configuration space - Read Write
pub const REG_MMIO_CONFIG: u64 = 0x100;
