// Copyright 2016 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![allow(non_camel_case_types)]
#![deny(warnings)]

use std::{cmp, fmt};

pub type zx_addr_t = usize;
pub type zx_duration_t = u64;
pub type zx_futex_t = i32;
pub type zx_handle_t = u32;
pub type zx_off_t = u64;
pub type zx_paddr_t = usize;
pub type zx_rights_t = u32;
pub type zx_signals_t = u32;
pub type zx_size_t = usize;
pub type zx_ssize_t = isize;
pub type zx_status_t = i32;
pub type zx_time_t = u64;
pub type zx_vaddr_t = usize;

// TODO: combine these macros with the bitflags and assoc consts macros below
// so that we only have to do one macro invocation.
// The result would look something like:
// multiconst!(bitflags, zx_rights_t, Rights, [RIGHT_NONE => ZX_RIGHT_NONE = 0; ...]);
// multiconst!(assoc_consts, zx_status_t, Status, [OK => ZX_OK = 0; ...]);
// Note that the actual name of the inner macro (e.g. `bitflags`) can't be a variable.
// It'll just have to be matched on manually
macro_rules! multiconst {
    ($typename:ident, [$($rawname:ident = $value:expr;)*]) => {
        $(
            pub const $rawname: $typename = $value;
        )*
    }
}

multiconst!(zx_handle_t, [
    ZX_HANDLE_INVALID = 0;
]);

multiconst!(zx_time_t, [
    ZX_TIME_INFINITE = ::std::u64::MAX;
]);

multiconst!(zx_rights_t, [
    ZX_RIGHT_NONE         = 0;
    ZX_RIGHT_DUPLICATE    = 1 << 0;
    ZX_RIGHT_TRANSFER     = 1 << 1;
    ZX_RIGHT_READ         = 1 << 2;
    ZX_RIGHT_WRITE        = 1 << 3;
    ZX_RIGHT_EXECUTE      = 1 << 4;
    ZX_RIGHT_MAP          = 1 << 5;
    ZX_RIGHT_GET_PROPERTY = 1 << 6;
    ZX_RIGHT_SET_PROPERTY = 1 << 7;
    ZX_RIGHT_ENUMERATE    = 1 << 8;
    ZX_RIGHT_DESTROY      = 1 << 9;
    ZX_RIGHT_SET_POLICY   = 1 << 10;
    ZX_RIGHT_GET_POLICY   = 1 << 11;
    ZX_RIGHT_SIGNAL       = 1 << 12;
    ZX_RIGHT_SIGNAL_PEER  = 1 << 13;
    ZX_RIGHT_WAIT         = 0 << 14; // Coming Soon!
    ZX_RIGHT_SAME_RIGHTS  = 1 << 31;
]);

// TODO: add an alias for this type in the C headers.
multiconst!(u32, [
    ZX_VMO_OP_COMMIT = 1;
    ZX_VMO_OP_DECOMMIT = 2;
    ZX_VMO_OP_LOCK = 3;
    ZX_VMO_OP_UNLOCK = 4;
    ZX_VMO_OP_LOOKUP = 5;
    ZX_VMO_OP_CACHE_SYNC = 6;
    ZX_VMO_OP_CACHE_INVALIDATE = 7;
    ZX_VMO_OP_CACHE_CLEAN = 8;
    ZX_VMO_OP_CACHE_CLEAN_INVALIDATE = 9;
]);

// TODO: add an alias for this type in the C headers.
multiconst!(u32, [
    ZX_VM_FLAG_PERM_READ          = 1  << 0;
    ZX_VM_FLAG_PERM_WRITE         = 1  << 1;
    ZX_VM_FLAG_PERM_EXECUTE       = 1  << 2;
    ZX_VM_FLAG_COMPACT            = 1  << 3;
    ZX_VM_FLAG_SPECIFIC           = 1  << 4;
    ZX_VM_FLAG_SPECIFIC_OVERWRITE = 1  << 5;
    ZX_VM_FLAG_CAN_MAP_SPECIFIC   = 1  << 6;
    ZX_VM_FLAG_CAN_MAP_READ       = 1  << 7;
    ZX_VM_FLAG_CAN_MAP_WRITE      = 1  << 8;
    ZX_VM_FLAG_CAN_MAP_EXECUTE    = 1  << 9;
]);

multiconst!(zx_status_t, [
    ZX_OK                    = 0;
    ZX_ERR_INTERNAL          = -1;
    ZX_ERR_NOT_SUPPORTED     = -2;
    ZX_ERR_NO_RESOURCES      = -3;
    ZX_ERR_NO_MEMORY         = -4;
    ZX_ERR_CALL_FAILED       = -5;
    ZX_ERR_INTERRUPTED_RETRY = -6;
    ZX_ERR_INVALID_ARGS      = -10;
    ZX_ERR_BAD_HANDLE        = -11;
    ZX_ERR_WRONG_TYPE        = -12;
    ZX_ERR_BAD_SYSCALL       = -13;
    ZX_ERR_OUT_OF_RANGE      = -14;
    ZX_ERR_BUFFER_TOO_SMALL  = -15;
    ZX_ERR_BAD_STATE         = -20;
    ZX_ERR_TIMED_OUT         = -21;
    ZX_ERR_SHOULD_WAIT       = -22;
    ZX_ERR_CANCELED          = -23;
    ZX_ERR_PEER_CLOSED       = -24;
    ZX_ERR_NOT_FOUND         = -25;
    ZX_ERR_ALREADY_EXISTS    = -26;
    ZX_ERR_ALREADY_BOUND     = -27;
    ZX_ERR_UNAVAILABLE       = -28;
    ZX_ERR_ACCESS_DENIED     = -30;
    ZX_ERR_IO                = -40;
    ZX_ERR_IO_REFUSED        = -41;
    ZX_ERR_IO_DATA_INTEGRITY = -42;
    ZX_ERR_IO_DATA_LOSS      = -43;
    ZX_ERR_BAD_PATH          = -50;
    ZX_ERR_NOT_DIR           = -51;
    ZX_ERR_NOT_FILE          = -52;
    ZX_ERR_FILE_BIG          = -53;
    ZX_ERR_NO_SPACE          = -54;
    ZX_ERR_STOP              = -60;
    ZX_ERR_NEXT              = -61;
]);

multiconst!(zx_signals_t, [
    ZX_SIGNAL_NONE              = 0;
    ZX_OBJECT_SIGNAL_ALL        = 0x00ffffff;
    ZX_USER_SIGNAL_ALL          = 0xff000000;
    ZX_OBJECT_SIGNAL_0          = 1 << 0;
    ZX_OBJECT_SIGNAL_1          = 1 << 1;
    ZX_OBJECT_SIGNAL_2          = 1 << 2;
    ZX_OBJECT_SIGNAL_3          = 1 << 3;
    ZX_OBJECT_SIGNAL_4          = 1 << 4;
    ZX_OBJECT_SIGNAL_5          = 1 << 5;
    ZX_OBJECT_SIGNAL_6          = 1 << 6;
    ZX_OBJECT_SIGNAL_7          = 1 << 7;
    ZX_OBJECT_SIGNAL_8          = 1 << 8;
    ZX_OBJECT_SIGNAL_9          = 1 << 9;
    ZX_OBJECT_SIGNAL_10         = 1 << 10;
    ZX_OBJECT_SIGNAL_11         = 1 << 11;
    ZX_OBJECT_SIGNAL_12         = 1 << 12;
    ZX_OBJECT_SIGNAL_13         = 1 << 13;
    ZX_OBJECT_SIGNAL_14         = 1 << 14;
    ZX_OBJECT_SIGNAL_15         = 1 << 15;
    ZX_OBJECT_SIGNAL_16         = 1 << 16;
    ZX_OBJECT_SIGNAL_17         = 1 << 17;
    ZX_OBJECT_SIGNAL_18         = 1 << 18;
    ZX_OBJECT_SIGNAL_19         = 1 << 19;
    ZX_OBJECT_SIGNAL_20         = 1 << 20;
    ZX_OBJECT_SIGNAL_21         = 1 << 21;
    ZX_OBJECT_SIGNAL_22         = 1 << 22;
    ZX_OBJECT_HANDLE_CLOSED     = 1 << 23;
    ZX_USER_SIGNAL_0            = 1 << 24;
    ZX_USER_SIGNAL_1            = 1 << 25;
    ZX_USER_SIGNAL_2            = 1 << 26;
    ZX_USER_SIGNAL_3            = 1 << 27;
    ZX_USER_SIGNAL_4            = 1 << 28;
    ZX_USER_SIGNAL_5            = 1 << 29;
    ZX_USER_SIGNAL_6            = 1 << 30;
    ZX_USER_SIGNAL_7            = 1 << 31;

    ZX_OBJECT_READABLE          = ZX_OBJECT_SIGNAL_0;
    ZX_OBJECT_WRITABLE          = ZX_OBJECT_SIGNAL_1;
    ZX_OBJECT_PEER_CLOSED       = ZX_OBJECT_SIGNAL_2;

    // Cancelation (handle was closed while waiting with it)
    ZX_SIGNAL_HANDLE_CLOSED     = ZX_OBJECT_HANDLE_CLOSED;

    // Event
    ZX_EVENT_SIGNALED           = ZX_OBJECT_SIGNAL_3;

    // EventPair
    ZX_EPAIR_SIGNALED           = ZX_OBJECT_SIGNAL_3;
    ZX_EPAIR_CLOSED             = ZX_OBJECT_SIGNAL_2;

    // Task signals (process, thread, job)
    ZX_TASK_TERMINATED          = ZX_OBJECT_SIGNAL_3;

    // Channel
    ZX_CHANNEL_READABLE         = ZX_OBJECT_SIGNAL_0;
    ZX_CHANNEL_WRITABLE         = ZX_OBJECT_SIGNAL_1;
    ZX_CHANNEL_PEER_CLOSED      = ZX_OBJECT_SIGNAL_2;

    // Socket
    ZX_SOCKET_READABLE          = ZX_OBJECT_SIGNAL_0;
    ZX_SOCKET_WRITABLE          = ZX_OBJECT_SIGNAL_1;
    ZX_SOCKET_PEER_CLOSED       = ZX_OBJECT_SIGNAL_2;

    // Port
    ZX_PORT_READABLE            = ZX_OBJECT_READABLE;

    // Resource
    ZX_RESOURCE_DESTROYED       = ZX_OBJECT_SIGNAL_3;
    ZX_RESOURCE_READABLE        = ZX_OBJECT_READABLE;
    ZX_RESOURCE_WRITABLE        = ZX_OBJECT_WRITABLE;
    ZX_RESOURCE_CHILD_ADDED     = ZX_OBJECT_SIGNAL_4;

    // Fifo
    ZX_FIFO_READABLE            = ZX_OBJECT_READABLE;
    ZX_FIFO_WRITABLE            = ZX_OBJECT_WRITABLE;
    ZX_FIFO_PEER_CLOSED         = ZX_OBJECT_PEER_CLOSED;

    // Job
    ZX_JOB_NO_PROCESSES         = ZX_OBJECT_SIGNAL_3;
    ZX_JOB_NO_JOBS              = ZX_OBJECT_SIGNAL_4;

    // Process
    ZX_PROCESS_TERMINATED       = ZX_OBJECT_SIGNAL_3;

    // Thread
    ZX_THREAD_TERMINATED        = ZX_OBJECT_SIGNAL_3;

    // Log
    ZX_LOG_READABLE             = ZX_OBJECT_READABLE;
    ZX_LOG_WRITABLE             = ZX_OBJECT_WRITABLE;

    // Timer
    ZX_TIMER_SIGNALED           = ZX_OBJECT_SIGNAL_3;
]);

// clock ids
pub const ZX_CLOCK_MONOTONIC: u32 = 0;

// Buffer size limits on the cprng syscalls
pub const ZX_CPRNG_DRAW_MAX_LEN: usize = 256;
pub const ZX_CPRNG_ADD_ENTROPY_MAX_LEN: usize = 256;

// Socket flags and limits.
pub const ZX_SOCKET_HALF_CLOSE: u32 = 1;

// VM Object clone flags
pub const ZX_VMO_CLONE_COPY_ON_WRITE: u32 = 1;

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum zx_cache_policy_t {
    ZX_CACHE_POLICY_CACHED = 0,
    ZX_CACHE_POLICY_UNCACHED = 1,
    ZX_CACHE_POLICY_UNCACHED_DEVICE = 2,
    ZX_CACHE_POLICY_WRITE_COMBINING = 3,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct zx_wait_item_t {
    pub handle: zx_handle_t,
    pub waitfor: zx_signals_t,
    pub pending: zx_signals_t,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct zx_waitset_result_t {
    pub cookie: u64,
    pub status: zx_status_t,
    pub observed: zx_signals_t,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct zx_channel_call_args_t {
    pub wr_bytes: *const u8,
    pub wr_handles: *const zx_handle_t,
    pub rd_bytes: *mut u8,
    pub rd_handles: *mut zx_handle_t,
    pub wr_num_bytes: u32,
    pub wr_num_handles: u32,
    pub rd_num_bytes: u32,
    pub rd_num_handles: u32,
}

pub type zx_pci_irq_swizzle_lut_t = [[[u32; 4]; 8]; 32];

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct zx_pci_init_arg_t {
    pub dev_pin_to_global_irq: zx_pci_irq_swizzle_lut_t,
    pub num_irqs: u32,
    pub irqs: [zx_irq_t; 32],
    pub ecam_window_count: u32,
    // Note: the ecam_windows field is actually a variable size array.
    // We use a fixed size array to match the C repr.
    pub ecam_windows: [zx_ecam_window_t; 1],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct zx_irq_t {
    pub global_irq: u32,
    pub level_triggered: bool,
    pub active_high: bool,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct zx_ecam_window_t {
    pub base: u64,
    pub size: usize,
    pub bus_start: u8,
    pub bus_end: u8,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct zx_pcie_device_info_t {
    pub vendor_id: u16,
    pub device_id: u16,
    pub base_class: u8,
    pub sub_class: u8,
    pub program_interface: u8,
    pub revision_id: u8,
    pub bus_id: u8,
    pub dev_id: u8,
    pub func_id: u8,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct zx_pci_resource_t {
    pub type_: u32,
    pub size: usize,
    // TODO: Actually a union
    pub pio_addr: usize,
}

// TODO: Actually a union
pub type zx_rrec_t = [u8; 64];

// Ports V2
#[repr(u32)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum zx_packet_type_t {
    ZX_PKT_TYPE_USER = 0,
    ZX_PKT_TYPE_SIGNAL_ONE = 1,
    ZX_PKT_TYPE_SIGNAL_REP = 2,
}

impl Default for zx_packet_type_t {
    fn default() -> Self {
        zx_packet_type_t::ZX_PKT_TYPE_USER
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct zx_packet_signal_t {
    pub trigger: zx_signals_t,
    pub observed: zx_signals_t,
    pub count: u64,
}

pub const ZX_WAIT_ASYNC_ONCE: u32 = 0;
pub const ZX_WAIT_ASYNC_REPEATING: u32 = 1;

// Actually a union of different integer types, but this should be good enough.
pub type zx_packet_user_t = [u8; 32];

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct zx_port_packet_t {
    pub key: u64,
    pub packet_type: zx_packet_type_t,
    pub status: i32,
    pub union: [u8; 32],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct zx_guest_io_t {
    port: u16,
    access_size: u8,
    input: bool,
    // TODO: Actually a union
    data: [u8; 4],
}

#[cfg(target_arch="aarch64")]
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct zx_guest_memory_t {
    addr: zx_vaddr_t,
    inst: u32,
}

pub const X86_MAX_INST_LEN: usize = 15;

#[cfg(target_arch="x86_64")]
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct zx_guest_memory_t {
    addr: zx_vaddr_t,
    inst_len: u8,
    inst_buf: [u8; X86_MAX_INST_LEN],
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum zx_guest_packet_t_type {
    ZX_GUEST_PKT_MEMORY = 1,
    ZX_GUEST_PKT_IO = 2,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union zx_guest_packet_t_union {
    // ZX_GUEST_PKT_MEMORY
    memory: zx_guest_memory_t,
    // ZX_GUEST_PKT_IO
    io: zx_guest_io_t,
}

// Note: values of this type must maintain the invariant that
// `packet_type` correctly indicates the type of `contents`.
// Failure to do so will result in unsafety.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct zx_guest_packet_t {
    packet_type: zx_guest_packet_t_type,
    contents: zx_guest_packet_t_union,
}

impl fmt::Debug for zx_guest_packet_t {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "zx_guest_packet_t {{ packet_type: {:?}, contents: ", self.packet_type)?;
        match self.packet_type {
            zx_guest_packet_t_type::ZX_GUEST_PKT_MEMORY =>
                write!(f, "zx_guest_packet_t_union {{ memory: {:?} }} }}",
                    unsafe { self.contents.memory }
                ),
            zx_guest_packet_t_type::ZX_GUEST_PKT_IO =>
                write!(f, "zx_guest_packet_t_union {{ io: {:?} }} }}",
                    unsafe { self.contents.io }
                ),
        }
    }
}

impl cmp::PartialEq for zx_guest_packet_t {
    fn eq(&self, other: &Self) -> bool {
        (self.packet_type == other.packet_type) &&
        match self.packet_type {
            zx_guest_packet_t_type::ZX_GUEST_PKT_MEMORY =>
                unsafe { self.contents.memory == other.contents.memory },
            zx_guest_packet_t_type::ZX_GUEST_PKT_IO =>
                unsafe { self.contents.io == other.contents.io },
        }
    }
}

impl cmp::Eq for zx_guest_packet_t {}

#[cfg(target_arch="x86_64")]
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct zx_vcpu_create_args_t {
    pub ip: zx_vaddr_t,
    pub cr3: zx_vaddr_t,
    pub apic_vmo: zx_handle_t,
}

#[cfg(not(target_arch="x86_64"))]
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct zx_vcpu_create_args_t {
    pub ip: zx_vaddr_t,
}

include!("definitions.rs");
