// SPDX-License-Identifier: Apache-2.0 or MIT
//
// Copyright 2021 Sony Group Corporation
//

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

//! Raw FFI bindings for libseccomp library

use std::os::raw::*;

pub const SECCOMP_MODE_DISABLED: u64 = 0;
pub const SECCOMP_MODE_STRICT: u64 = 1;
pub const SECCOMP_MODE_FILTER: u64 = 2;

pub const SECCOMP_SET_MODE_STRICT: u32 = 0;
pub const SECCOMP_SET_MODE_FILTER: u32 = 1;
pub const SECCOMP_GET_ACTION_AVAIL: u32 = 2;
pub const SECCOMP_GET_NOTIF_SIZES: u32 = 3;

pub const SECCOMP_FILTER_FLAG_TSYNC: u32 = 1;
pub const SECCOMP_FILTER_FLAG_LOG: u32 = 2;
pub const SECCOMP_FILTER_FLAG_SPEC_ALLOW: u32 = 4;
pub const SECCOMP_FILTER_FLAG_NEW_LISTENER: u32 = 8;
pub const SECCOMP_FILTER_FLAG_TSYNC_ESRCH: u32 = 16;

pub const SECCOMP_RET_KILL_PROCESS: u32 = 0x80000000;
pub const SECCOMP_RET_KILL_THREAD: u32 = 0x00000000;
pub const SECCOMP_RET_KILL: u32 = SECCOMP_RET_KILL_THREAD;
pub const SECCOMP_RET_TRAP: u32 = 0x00030000;
pub const SECCOMP_RET_ERRNO: u32 = 0x00050000;
pub const SECCOMP_RET_USER_NOTIF: u32 = 0x7fc00000;
pub const SECCOMP_RET_TRACE: u32 = 0x7ff00000;
pub const SECCOMP_RET_LOG: u32 = 0x7ffc0000;
pub const SECCOMP_RET_ALLOW: u32 = 0x7fff0000;

pub const SECCOMP_RET_ACTION_FULL: u32 = 0xffff0000;
pub const SECCOMP_RET_ACTION: u32 = 0x7fff0000;
pub const SECCOMP_RET_DATA: u32 = 0x0000ffff;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct seccomp_data {
    pub nr: c_int,
    pub arch: u32,
    pub instruction_pointer: u64,
    pub args: [u64; 6],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct seccomp_notif_sizes {
    pub seccomp_notif: u16,
    pub seccomp_notif_resp: u16,
    pub seccomp_data: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct seccomp_notif {
    pub id: u64,
    pub pid: u32,
    pub flags: u32,
    pub data: seccomp_data,
}

/// Tell the kernel to execute the target's system call
///
/// `linux/seccomp.h`:
///
/// > Note, the `SECCOMP_USER_NOTIF_FLAG_CONTINUE` flag must be used with caution!
/// > If set by the process supervising the syscalls of another process the
/// > syscall will continue. This is problematic because of an inherent TOCTOU.
/// > An attacker can exploit the time while the supervised process is waiting on
/// > a response from the supervising process to rewrite syscall arguments which
/// > are passed as pointers of the intercepted syscall.
/// > It should be absolutely clear that this means that the seccomp notifier
/// > _cannot_ be used to implement a security policy! It should only ever be used
/// > in scenarios where a more privileged process supervises the syscalls of a
/// > lesser privileged process to get around kernel-enforced security
/// > restrictions when the privileged process deems this safe. In other words,
/// > in order to continue a syscall the supervising process should be sure that
/// > another security mechanism or the kernel itself will sufficiently block
/// > syscalls if arguments are rewritten to something unsafe.
/// >
/// > Similar precautions should be applied when stacking `SECCOMP_RET_USER_NOTIF`
/// > or `SECCOMP_RET_TRACE`. For `SECCOMP_RET_USER_NOTIF` filters acting on the
/// > same syscall, the most recently added filter takes precedence. This means
/// > that the new `SECCOMP_RET_USER_NOTIF` filter can override any
/// > `SECCOMP_IOCTL_NOTIF_SEND` from earlier filters, essentially allowing all
/// > such filtered syscalls to be executed by sending the response
/// > `SECCOMP_USER_NOTIF_FLAG_CONTINUE`. Note that `SECCOMP_RET_TRACE` can equally
/// > be overriden by `SECCOMP_USER_NOTIF_FLAG_CONTINUE`.
pub const SECCOMP_USER_NOTIF_FLAG_CONTINUE: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct seccomp_notif_resp {
    pub id: u64,
    pub val: i64,
    pub error: i32,
    pub flags: u32,
}

pub const SECCOMP_ADDFD_FLAG_SETFD: u32 = 1;
pub const SECCOMP_ADDFD_FLAG_SEND: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct seccomp_notif_addfd {
    pub id: u64,
    pub flags: u32,
    pub srcfd: u32,
    pub newfd: u32,
    pub newfd_flags: u32,
}

/// version information
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct scmp_version {
    pub major: c_uint,
    pub minor: c_uint,
    pub micro: c_uint,
}

/// Filter context/handle (`*mut`)
pub type scmp_filter_ctx = *mut c_void;
/// Filter context/handle (`*const`)
pub type const_scmp_filter_ctx = *const c_void;

/// Filter attributes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub enum scmp_filter_attr {
    _SCMP_FLTATR_MIN = 0,
    /// default filter action
    SCMP_FLTATR_ACT_DEFAULT = 1,
    /// bad architecture action
    SCMP_FLTATR_ACT_BADARCH = 2,
    /// set `NO_NEW_PRIVS` on filter load
    SCMP_FLTATR_CTL_NNP = 3,
    /// sync threads on filter load
    SCMP_FLTATR_CTL_TSYNC = 4,
    /// allow rules with a -1 syscall
    SCMP_FLTATR_API_TSKIP = 5,
    /// log not-allowed actions
    SCMP_FLTATR_CTL_LOG = 6,
    /// disable SSB mitigation
    SCMP_FLTATR_CTL_SSB = 7,
    /// filter optimization level:
    /// - 0: currently unused
    /// - 1: rules weighted by priority and complexity (DEFAULT)
    /// - 2: binary tree sorted by syscall number
    SCMP_FLTATR_CTL_OPTIMIZE = 8,
    /// return the system return codes
    SCMP_FLTATR_API_SYSRAWRC = 9,
    _SCMP_FLTATR_MAX,
}

/// Comparison operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub enum scmp_compare {
    _SCMP_CMP_MIN = 0,
    /// not equal
    SCMP_CMP_NE = 1,
    /// less than
    SCMP_CMP_LT = 2,
    /// less than or equal
    SCMP_CMP_LE = 3,
    /// equal
    SCMP_CMP_EQ = 4,
    /// greater than or equal
    SCMP_CMP_GE = 5,
    /// greater than
    SCMP_CMP_GT = 6,
    /// masked equality
    SCMP_CMP_MASKED_EQ = 7,
    _SCMP_CMP_MAX,
}

/// Argument datum
pub type scmp_datum_t = u64;

/// Argument / Value comparison definition
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct scmp_arg_cmp {
    /// argument number, starting at 0
    pub arg: c_uint,
    /// the comparison op, e.g. `SCMP_CMP_*`
    pub op: scmp_compare,
    pub datum_a: scmp_datum_t,
    pub datum_b: scmp_datum_t,
}

/// The native architecture token
pub const SCMP_ARCH_NATIVE: u32 = 0x0;
/// The x86 (32-bit) architecture token
pub const SCMP_ARCH_X86: u32 = 0x40000003;
/// The x86-64 (64-bit) architecture token
pub const SCMP_ARCH_X86_64: u32 = 0xc000003e;
/// The x32 (32-bit x86_64) architecture token
///
/// NOTE: this is different from the value used by the kernel because libseccomp need to
/// be able to distinguish between x32 and x86_64
pub const SCMP_ARCH_X32: u32 = 0x4000003e;
pub const SCMP_ARCH_ARM: u32 = 0x40000028;
pub const SCMP_ARCH_AARCH64: u32 = 0xc00000b7;
pub const SCMP_ARCH_MIPS: u32 = 0x8;
pub const SCMP_ARCH_MIPS64: u32 = 0x80000008;
pub const SCMP_ARCH_MIPS64N32: u32 = 0xa0000008;
pub const SCMP_ARCH_MIPSEL: u32 = 0x40000008;
pub const SCMP_ARCH_MIPSEL64: u32 = 0xc0000008;
pub const SCMP_ARCH_MIPSEL64N32: u32 = 0xe0000008;
pub const SCMP_ARCH_PPC: u32 = 0x14;
pub const SCMP_ARCH_PPC64: u32 = 0x80000015;
pub const SCMP_ARCH_PPC64LE: u32 = 0xc0000015;
pub const SCMP_ARCH_S390: u32 = 0x16;
pub const SCMP_ARCH_S390X: u32 = 0x80000016;
pub const SCMP_ARCH_PARISC: u32 = 0xf;
pub const SCMP_ARCH_PARISC64: u32 = 0x8000000f;
pub const SCMP_ARCH_RISCV64: u32 = 0xc00000f3;

pub const SCMP_ACT_MASK: u32 = SECCOMP_RET_ACTION_FULL;
/// Kill the process
pub const SCMP_ACT_KILL_PROCESS: u32 = 0x80000000;
/// Kill the thread
pub const SCMP_ACT_KILL_THREAD: u32 = 0x00000000;
/// Kill the thread, defined for backward compatibility
pub const SCMP_ACT_KILL: u32 = SCMP_ACT_KILL_THREAD;
/// Throw a `SIGSYS` signal
pub const SCMP_ACT_TRAP: u32 = 0x00030000;
/// Notifies userspace
pub const SCMP_ACT_NOTIFY: u32 = 0x7fc00000;
pub const SCMP_ACT_ERRNO_MASK: u32 = 0x00050000;
/// Return the specified error code
#[must_use]
pub const fn SCMP_ACT_ERRNO(x: u16) -> u32 {
    SCMP_ACT_ERRNO_MASK | x as u32
}
pub const SCMP_ACT_TRACE_MASK: u32 = 0x7ff00000;
/// Notify a tracing process with the specified value
#[must_use]
pub const fn SCMP_ACT_TRACE(x: u16) -> u32 {
    SCMP_ACT_TRACE_MASK | x as u32
}
/// Allow the syscall to be executed after the action has been logged
pub const SCMP_ACT_LOG: u32 = 0x7ffc0000;
/// Allow the syscall to be executed
pub const SCMP_ACT_ALLOW: u32 = 0x7fff0000;

#[link(name = "seccomp")]
extern "C" {

    /// Query the library version information
    ///
    /// This function returns a pointer to a populated [`scmp_version`] struct, the
    /// caller does not need to free the structure when finished.
    pub fn seccomp_version() -> *const scmp_version;

    /// Query the library's level of API support
    ///
    /// This function returns an API level value indicating the current supported
    /// functionality.  It is important to note that this level of support is
    /// determined at runtime and therefore can change based on the running kernel
    /// and system configuration (e.g. any previously loaded seccomp filters).  This
    /// function can be called multiple times, but it only queries the system the
    /// first time it is called, the API level is cached and used in subsequent
    /// calls.
    ///
    /// The current API levels are described below:
    /// - 0
    ///   - reserved
    /// - 1
    ///   - base level
    /// - 2
    ///   - support for the [`SCMP_FLTATR_CTL_TSYNC`](scmp_filter_attr::SCMP_FLTATR_CTL_TSYNC) filter attribute
    ///   - uses the [`seccomp(2)`] syscall instead of the [`prctl(2)`] syscall
    /// - 3
    ///   - support for the [`SCMP_FLTATR_CTL_LOG`](scmp_filter_attr::SCMP_FLTATR_CTL_LOG) filter attribute
    ///   - support for the [`SCMP_ACT_LOG`] action
    ///   - support for the [`SCMP_ACT_KILL_PROCESS`] action
    /// - 4
    ///   - support for the [`SCMP_FLTATR_CTL_SSB`](scmp_filter_attr::SCMP_FLTATR_CTL_SSB) filter attrbute
    /// - 5
    ///   - support for the [`SCMP_ACT_NOTIFY`] action and notify APIs
    /// - 6
    ///   - support the simultaneous use of [`SCMP_FLTATR_CTL_TSYNC`](scmp_filter_attr::SCMP_FLTATR_CTL_TSYNC) and notify APIs
    ///
    /// [`seccomp(2)`]: https://man7.org/linux/man-pages/man2/seccomp.2.html
    /// [`prctl(2)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
    pub fn seccomp_api_get() -> c_uint;

    /// Set the library's level of API support
    ///
    /// This function forcibly sets the API level of the library at runtime.  Valid
    /// API levels are discussed in the description of the [`seccomp_api_get()`]
    /// function.  General use of this function is strongly discouraged.
    pub fn seccomp_api_set(level: c_uint) -> c_int;

    /// Initialize the filter state
    ///
    /// - `def_action`: the default filter action
    ///
    /// This function initializes the internal seccomp filter state and should
    /// be called before any other functions in this library to ensure the filter
    /// state is initialized.  Returns a filter context on success, `ptr::null()` on failure.
    pub fn seccomp_init(def_action: u32) -> scmp_filter_ctx;

    /// Reset the filter state
    ///
    /// - `ctx`: the filter context
    /// - `def_action`: the default filter action
    ///
    /// This function resets the given seccomp filter state and ensures the
    /// filter state is reinitialized.  This function does not reset any seccomp
    /// filters already loaded into the kernel.  Returns zero on success, negative
    /// values on failure.
    pub fn seccomp_reset(ctx: scmp_filter_ctx, def_action: u32) -> c_int;

    /// Destroys the filter state and releases any resources
    ///
    /// - `ctx`: the filter context
    ///
    /// This functions destroys the given seccomp filter state and releases any
    /// resources, including memory, associated with the filter state.  This
    /// function does not reset any seccomp filters already loaded into the kernel.
    /// The filter context can no longer be used after calling this function.
    pub fn seccomp_release(ctx: scmp_filter_ctx);

    /// Merge two filters
    ///
    /// - `ctx_dst`: the destination filter context
    /// - `ctx_src`: the source filter context
    ///
    /// This function merges two filter contexts into a single filter context and
    /// destroys the second filter context.  The two filter contexts must have the
    /// same attribute values and not contain any of the same architectures; if they
    /// do, the merge operation will fail.  On success, the source filter context
    /// will be destroyed and should no longer be used; it is not necessary to
    /// call [`seccomp_release()`] on the source filter context.  Returns zero on
    /// success, negative values on failure.
    pub fn seccomp_merge(ctx_dst: scmp_filter_ctx, ctx_src: scmp_filter_ctx) -> c_int;

    /// Resolve the architecture name to a architecture token
    ///
    /// - `arch_name`: the architecture name
    ///
    /// This function resolves the given architecture name to a token suitable for
    /// use with libseccomp, returns zero on failure.
    pub fn seccomp_arch_resolve_name(arch_name: *const c_char) -> u32;

    /// Return the native architecture token
    ///
    /// This function returns the native architecture token value, e.g. `SCMP_ARCH_*`.
    pub fn seccomp_arch_native() -> u32;

    /// Check to see if an existing architecture is present in the filter
    ///
    /// - `ctx`: the filter context
    /// - `arch_token`: the architecture token, e.g. `SCMP_ARCH_*`
    ///
    /// This function tests to see if a given architecture is included in the filter
    /// context.  If the architecture token is [`SCMP_ARCH_NATIVE`] then the native
    /// architecture will be assumed.  Returns zero if the architecture exists in
    /// the filter, `-libc::EEXIST` if it is not present, and other negative values on
    /// failure.
    pub fn seccomp_arch_exist(ctx: const_scmp_filter_ctx, arch_token: u32) -> c_int;

    /// Adds an architecture to the filter
    ///
    /// - `ctx`: the filter context
    /// - `arch_token`: the architecture token, e.g. `SCMP_ARCH_*`
    ///
    /// This function adds a new architecture to the given seccomp filter context.
    /// Any new rules added after this function successfully returns will be added
    /// to this architecture but existing rules will not be added to this
    /// architecture.  If the architecture token is [`SCMP_ARCH_NATIVE`] then the native
    /// architecture will be assumed.  Returns zero on success, `-libc::EEXIST` if
    /// specified architecture is already present, other negative values on failure.
    pub fn seccomp_arch_add(ctx: scmp_filter_ctx, arch_token: u32) -> c_int;

    /// Removes an architecture from the filter
    ///
    /// - `ctx`: the filter context
    /// - `arch_token`: the architecture token, e.g. `SCMP_ARCH_*`
    ///
    /// This function removes an architecture from the given seccomp filter context.
    /// If the architecture token is [`SCMP_ARCH_NATIVE`] then the native architecture
    /// will be assumed.  Returns zero on success, negative values on failure.
    pub fn seccomp_arch_remove(ctx: scmp_filter_ctx, arch_token: u32) -> c_int;

    /// Loads the filter into the kernel
    ///
    /// - `ctx`: the filter context
    ///
    /// This function loads the given seccomp filter context into the kernel.  If
    /// the filter was loaded correctly, the kernel will be enforcing the filter
    /// when this function returns.  Returns zero on success, negative values on
    /// error.
    pub fn seccomp_load(ctx: const_scmp_filter_ctx) -> c_int;

    /// Set the value of a filter attribute
    ///
    /// - `ctx`: the filter context
    /// - `attr`: the filter attribute name
    /// - `value`: the filter attribute value
    ///
    /// This function fetches the value of the given attribute name and returns it
    /// via `value`.  Returns zero on success, negative values on failure.
    pub fn seccomp_attr_get(
        ctx: const_scmp_filter_ctx,
        attr: scmp_filter_attr,
        value: *mut u32,
    ) -> c_int;

    /// Set the value of a filter attribute
    ///
    /// - `ctx`: the filter context
    /// - `attr`: the filter attribute name
    /// - `value`: the filter attribute value
    ///
    /// This function sets the value of the given attribute.  Returns zero on
    /// success, negative values on failure.
    pub fn seccomp_attr_set(ctx: scmp_filter_ctx, attr: scmp_filter_attr, value: u32) -> c_int;

    /// Resolve a syscall number to a name
    ///
    /// - `arch_token`: the architecture token, e.g. `SCMP_ARCH_*`
    /// - `num`: the syscall number
    ///
    /// Resolve the given syscall number to the syscall name for the given
    /// architecture; it is up to the caller to free the returned string.  Returns
    /// the syscall name on success, `ptr::null()` on failure
    pub fn seccomp_syscall_resolve_num_arch(arch_token: u32, num: c_int) -> *const c_char;

    /// Resolve a syscall name to a number
    ///
    /// - `arch_token`: the architecture token, e.g. `SCMP_ARCH_*`
    /// - `name`: the syscall name
    ///
    /// Resolve the given syscall name to the syscall number for the given
    /// architecture.  Returns the syscall number on success, including negative
    /// pseudo syscall numbers (e.g. `__PNR_*`); returns [`__NR_SCMP_ERROR`] on failure.
    pub fn seccomp_syscall_resolve_name_arch(arch_token: u32, name: *const c_char) -> c_int;

    /// Resolve a syscall name to a number and perform any rewriting necessary
    ///
    /// - `arch_token`: the architecture token, e.g. `SCMP_ARCH_*`
    /// - `name`: the syscall name
    ///
    /// Resolve the given syscall name to the syscall number for the given
    /// architecture and do any necessary syscall rewriting needed by the
    /// architecture.  Returns the syscall number on success, including negative
    /// pseudo syscall numbers (e.g. `__PNR_*`); returns [`__NR_SCMP_ERROR`] on failure.
    pub fn seccomp_syscall_resolve_name_rewrite(arch_token: u32, name: *const c_char) -> c_int;

    /// Resolve a syscall name to a number
    ///
    /// - `name`: the syscall name
    ///
    /// Resolve the given syscall name to the syscall number.  Returns the syscall
    /// number on success, including negative pseudo syscall numbers (e.g. `__PNR_*`);
    /// returns [`__NR_SCMP_ERROR`] on failure.
    pub fn seccomp_syscall_resolve_name(name: *const c_char) -> c_int;

    /// Set the priority of a given syscall
    ///
    /// - `ctx`: the filter context
    /// - `syscall`: the syscall number
    /// - `priority`: priority value, higher value == higher priority
    ///
    /// This function sets the priority of the given syscall; this value is used
    /// when generating the seccomp filter code such that higher priority syscalls
    /// will incur less filter code overhead than the lower priority syscalls in the
    /// filter.  Returns zero on success, negative values on failure.
    pub fn seccomp_syscall_priority(ctx: scmp_filter_ctx, syscall: c_int, priority: u8) -> c_int;

    /// Add a new rule to the filter
    ///
    /// - `ctx`: the filter context
    /// - `action`: the filter action
    /// - `syscall`: the syscall number
    /// - `arg_cnt`: the number of argument filters in the argument filter chain
    /// - `...`: [`scmp_arg_cmp`] structs
    ///
    /// This function adds a series of new argument/value checks to the seccomp
    /// filter for the given syscall; multiple argument/value checks can be
    /// specified and they will be chained together (AND'd together) in the filter.
    /// If the specified rule needs to be adjusted due to architecture specifics it
    /// will be adjusted without notification.  Returns zero on success, negative
    /// values on failure.
    pub fn seccomp_rule_add(
        ctx: scmp_filter_ctx,
        action: u32,
        syscall: c_int,
        arg_cnt: c_uint,
        ...
    ) -> c_int;

    /// Add a new rule to the filter
    ///
    /// - `ctx`: the filter context
    /// - `action`: the filter action
    /// - `syscall`: the syscall number
    /// - `arg_cnt`: the number of elements in the arg_array parameter
    /// - `arg_array`: array of [`scmp_arg_cmp`] structs
    ///
    /// This function adds a series of new argument/value checks to the seccomp
    /// filter for the given syscall; multiple argument/value checks can be
    /// specified and they will be chained together (AND'd together) in the filter.
    /// If the specified rule needs to be adjusted due to architecture specifics it
    /// will be adjusted without notification.  Returns zero on success, negative
    /// values on failure.
    pub fn seccomp_rule_add_array(
        ctx: scmp_filter_ctx,
        action: u32,
        syscall: c_int,
        arg_cnt: c_uint,
        arg_array: *const scmp_arg_cmp,
    ) -> c_int;

    /// Add a new rule to the filter
    ///
    /// - `ctx`: the filter context
    /// - `action`: the filter action
    /// - `syscall`: the syscall number
    /// - `arg_cnt`: the number of argument filters in the argument filter chain
    /// - `...`: [`scmp_arg_cmp`] structs
    ///
    /// This function adds a series of new argument/value checks to the seccomp
    /// filter for the given syscall; multiple argument/value checks can be
    /// specified and they will be chained together (AND'd together) in the filter.
    /// If the specified rule can not be represented on the architecture the
    /// function will fail.  Returns zero on success, negative values on failure.
    pub fn seccomp_rule_add_exact(
        ctx: scmp_filter_ctx,
        action: u32,
        syscall: c_int,
        arg_cnt: c_uint,
        ...
    ) -> c_int;

    /// Add a new rule to the filter
    ///
    /// - `ctx`: the filter context
    /// - `action`: the filter action
    /// - `syscall`: the syscall number
    /// - `arg_cnt`:  the number of elements in the arg_array parameter
    /// - `arg_array`: array of scmp_arg_cmp structs
    ///
    /// This function adds a series of new argument/value checks to the seccomp
    /// filter for the given syscall; multiple argument/value checks can be
    /// specified and they will be chained together (AND'd together) in the filter.
    /// If the specified rule can not be represented on the architecture the
    /// function will fail.  Returns zero on success, negative values on failure.
    pub fn seccomp_rule_add_exact_array(
        ctx: scmp_filter_ctx,
        action: u32,
        syscall: c_int,
        arg_cnt: c_uint,
        arg_array: *const scmp_arg_cmp,
    ) -> c_int;

    /// Allocate a pair of notification request/response structures
    ///
    /// - `req`: the request location
    /// - `resp`: the response location
    ///
    /// This function allocates a pair of request/response structure by computing
    /// the correct sized based on the currently running kernel. It returns zero on
    /// success, and negative values on failure.
    pub fn seccomp_notify_alloc(
        req: *mut *mut seccomp_notif,
        resp: *mut *mut seccomp_notif_resp,
    ) -> c_int;

    /// Free a pair of notification request/response structures.
    ///
    /// - `req`: the request location
    /// - `resp`: the response location
    pub fn seccomp_notify_free(req: *mut seccomp_notif, resp: *mut seccomp_notif_resp) -> c_int;

    /// Send a notification response to a seccomp notification fd
    ///
    /// - `fd`: the notification fd
    /// - `resp`: the response buffer to use
    ///
    /// Sends a notification response on this fd. This function is thread safe
    /// (synchronization is performed in the kernel). Returns zero on success,
    /// negative values on error.
    pub fn seccomp_notify_receive(fd: c_int, req: *mut seccomp_notif) -> c_int;

    /// Check if a notification id is still valid
    ///
    /// - `fd`: the notification fd
    /// - `id`: the id to test
    ///
    /// Checks to see if a notification id is still valid. Returns 0 on success, and
    /// negative values on failure.
    pub fn seccomp_notify_respond(fd: c_int, resp: *mut seccomp_notif_resp) -> c_int;

    /// Check if a notification id is still valid
    ///
    /// - `fd`: the notification fd
    /// - `id`: the id to test
    ///
    /// Checks to see if a notification id is still valid. Returns 0 on success, and
    /// negative values on failure.
    pub fn seccomp_notify_id_valid(fd: c_int, id: u64) -> c_int;

    /// Return the notification fd from a filter that has already been loaded
    ///
    /// - `ctx`: the filter context
    ///
    /// This returns the listener fd that was generated when the seccomp policy was
    /// loaded. This is only valid after [`seccomp_load()`] with a filter that makes
    /// use of [`SCMP_ACT_NOTIFY`].
    pub fn seccomp_notify_fd(ctx: const_scmp_filter_ctx) -> c_int;

    /// Generate seccomp Pseudo Filter Code (PFC) and export it to a file
    ///
    /// - `ctx`: the filter context
    /// - `fd`: the destination fd
    ///
    /// This function generates seccomp Pseudo Filter Code (PFC) and writes it to
    /// the given fd.  Returns zero on success, negative values on failure.
    pub fn seccomp_export_pfc(ctx: const_scmp_filter_ctx, fd: c_int) -> c_int;

    /// Generate seccomp Berkley Packet Filter (BPF) code and export it to a file
    ///
    /// - `ctx`: the filter context
    /// - `fd`: the destination fd
    ///
    /// This function generates seccomp Berkley Packer Filter (BPF) code and writes
    /// it to the given fd.  Returns zero on success, negative values on failure.
    pub fn seccomp_export_bpf(ctx: const_scmp_filter_ctx, fd: c_int) -> c_int;
}

/// Negative pseudo syscall number returned by some functions in case of an error
pub const __NR_SCMP_ERROR: c_int = -1;
pub const __NR_SCMP_UNDEF: c_int = -2;
