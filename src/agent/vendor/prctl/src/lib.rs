#![cfg(target_os="linux")]

//! Module provides safe abstraction over the prctl interface.
//! Provided functions map to a single `prctl()` call, although some of them
//! may be usable only on a specific architecture or only with root
//! privileges. All known enums that may be used as parameters are
//! provided in this crate.
//!
//! Each function provides result which will be `Err(errno)` in case the
//! prctl() call fails.
//!
//! To run tests requiring root privileges, enable feature "root_test".

extern crate libc;
extern crate nix;
use libc::{c_int, c_ulong};
use std::ffi::CString;
use std::mem;
use nix::errno::errno;

macro_rules! handle_errno {
    ($res:ident) => ({
        if $res == -1 {
            return Err(errno());
        }
    })
}

#[allow(non_camel_case_types)]
enum PrctlOption {
    PR_SET_PDEATHSIG = 1,
    PR_GET_PDEATHSIG = 2,
    PR_GET_DUMPABLE = 3,
    PR_SET_DUMPABLE = 4,
    PR_GET_UNALIGN = 5,
    PR_SET_UNALIGN = 6,
    PR_GET_KEEPCAPS = 7,
    PR_SET_KEEPCAPS = 8,
    PR_GET_FPEMU = 9,
    PR_SET_FPEMU = 10,
    PR_GET_TIMING = 13,
    PR_SET_TIMING = 14,
    PR_SET_NAME = 15,
    PR_GET_NAME = 16,
    PR_GET_ENDIAN = 19,
    PR_SET_ENDIAN = 20,
    PR_GET_SECCOMP = 21,
    PR_SET_SECCOMP = 22,
    PR_CAPBSET_READ = 23,
    PR_CAPBSET_DROP = 24,
    PR_GET_TSC = 25,
    PR_SET_TSC = 26,
    PR_GET_SECUREBITS = 27,
    PR_SET_SECUREBITS = 28,
    PR_SET_TIMERSLACK = 29,
    PR_GET_TIMERSLACK = 30,
    PR_TASK_PERF_EVENTS_DISABLE = 31,
    PR_TASK_PERF_EVENTS_ENABLE = 32,
    PR_MCE_KILL = 33,
    PR_MCE_KILL_GET = 34,
    PR_SET_MM = 35,
    PR_SET_CHILD_SUBREAPER = 36,
    PR_GET_CHILD_SUBREAPER = 37,
    PR_SET_NO_NEW_PRIVS = 38,
    PR_GET_NO_NEW_PRIVS = 39,
    PR_SET_THP_DISABLE = 41,
    PR_GET_THP_DISABLE = 42,

// Can't figure out a nice interface - missing unless someone is interested
//  PR_GET_FPEXC = 11,
//  PR_SET_FPEXC = 12,
}

#[allow(non_camel_case_types)]
#[derive(PartialEq,Debug,Copy,Clone)]
#[repr(i32)]
pub enum PrctlUnalign {
    PR_UNALIGN_NOPRINT = 1,
    PR_UNALIGN_SIGBUS = 2,
}

#[allow(non_camel_case_types)]
#[derive(PartialEq,Debug,Copy,Clone)]
#[repr(i32)]
pub enum PrctlFpemu {
    PR_FPEMU_NOPRINT = 1,
    PR_FPEMU_SIGFPE = 2,
}

#[allow(non_camel_case_types)]
#[derive(PartialEq,Debug,Copy,Clone)]
#[repr(i32)]
pub enum PrctlTiming {
    PR_TIMING_STATISTICAL = 0,
    PR_TIMING_TIMESTAMP = 1,
}

#[allow(non_camel_case_types)]
#[derive(PartialEq,Debug,Copy,Clone)]
#[repr(i32)]
pub enum PrctlEndian {
    PR_ENDIAN_BIG = 0,
    PR_ENDIAN_LITTLE = 1,
    PR_ENDIAN_PPC_LITTLE = 2,
}

#[allow(non_camel_case_types)]
#[derive(PartialEq,Debug,Copy,Clone)]
#[repr(i32)]
pub enum PrctlSeccomp {
    SECCOMP_MODE_DISABLED = 0,
    SECCOMP_MODE_STRICT = 1,
    SECCOMP_MODE_FILTER = 2,
}

#[allow(non_camel_case_types)]
#[derive(PartialEq,Debug,Copy,Clone)]
#[repr(i32)]
pub enum PrctlTsc {
    PR_TSC_ENABLE = 1,
    PR_TSC_SIGSEGV = 2,
}

#[allow(non_camel_case_types)]
#[derive(PartialEq,Debug,Copy,Clone)]
#[repr(i32)]
pub enum PrctlMceKill {
    PR_MCE_KILL_LATE    = 0,
    PR_MCE_KILL_EARLY   = 1,
    PR_MCE_KILL_DEFAULT = 2,
}

#[allow(non_camel_case_types)]
#[derive(Copy,Clone)]
pub enum PrctlCap {
    CAP_CHOWN            = 0,
    CAP_DAC_OVERRIDE     = 1,
    CAP_DAC_READ_SEARCH  = 2,
    CAP_FOWNER           = 3,
    CAP_FSETID           = 4,
    CAP_KILL             = 5,
    CAP_SETGID           = 6,
    CAP_SETUID           = 7,
    CAP_SETPCAP          = 8,
    CAP_LINUX_IMMUTABLE  = 9,
    CAP_NET_BIND_SERVICE = 10,
    CAP_NET_BROADCAST    = 11,
    CAP_NET_ADMIN        = 12,
    CAP_NET_RAW          = 13,
    CAP_IPC_LOCK         = 14,
    CAP_IPC_OWNER        = 15,
    CAP_SYS_MODULE       = 16,
    CAP_SYS_RAWIO        = 17,
    CAP_SYS_CHROOT       = 18,
    CAP_SYS_PTRACE       = 19,
    CAP_SYS_PACCT        = 20,
    CAP_SYS_ADMIN        = 21,
    CAP_SYS_BOOT         = 22,
    CAP_SYS_NICE         = 23,
    CAP_SYS_RESOURCE     = 24,
    CAP_SYS_TIME         = 25,
    CAP_SYS_TTY_CONFIG   = 26,
    CAP_MKNOD            = 27,
    CAP_LEASE            = 28,
    CAP_AUDIT_WRITE      = 29,
    CAP_AUDIT_CONTROL    = 30,
    CAP_SETFCAP          = 31,
    CAP_MAC_OVERRIDE     = 32,
    CAP_MAC_ADMIN        = 33,
    CAP_SYSLOG           = 34,
    CAP_WAKE_ALARM       = 35,
    CAP_BLOCK_SUSPEND    = 36,
    CAP_AUDIT_READ       = 37,
}

#[allow(non_camel_case_types)]
#[derive(PartialEq,Debug,Copy,Clone)]
pub enum PrctlSecurebits {
    SECBIT_NOROOT                 = 0x01,
    SECBIT_NOROOT_LOCKED          = 0x02,
    SECBIT_NO_SETUID_FIXUP        = 0x04,
    SECBIT_NO_SETUID_FIXUP_LOCKED = 0x08,
    SECBIT_KEEP_CAPS              = 0x10,
    SECBIT_KEEP_CAPS_LOCKED       = 0x20,
}

#[allow(non_camel_case_types)]
#[derive(PartialEq,Debug,Copy,Clone)]
pub enum PrctlMM {
    PR_SET_MM_START_CODE  = 1,
    PR_SET_MM_END_CODE    = 2,
    PR_SET_MM_START_DATA  = 3,
    PR_SET_MM_END_DATA    = 4,
    PR_SET_MM_START_STACK = 5,
    PR_SET_MM_START_BRK   = 6,
    PR_SET_MM_BRK         = 7,
    PR_SET_MM_ARG_START   = 8,
    PR_SET_MM_ARG_END     = 9,
    PR_SET_MM_ENV_START   = 10,
    PR_SET_MM_ENV_END     = 11,
    PR_SET_MM_AUXV        = 12,
    PR_SET_MM_EXE_FILE    = 13,
}

trait FromCInt {
    fn from_c_int(c_int) -> Self;
}
// Transmuting should be safe here since the layout is well defined (i32). This
// will of course work only on linux/glibc since it assumes that rust's type
// c_int == i32. Which is ok at the moment.
macro_rules! impl_from_c_int {
    ($en:ty) => (
        impl FromCInt for $en {
            fn from_c_int(val: c_int) -> $en {
                unsafe { mem::transmute(val as i32) }
            }
        }
    )
}
impl_from_c_int!(PrctlEndian);
impl_from_c_int!(PrctlFpemu);
impl_from_c_int!(PrctlMceKill);
impl_from_c_int!(PrctlSeccomp);
impl_from_c_int!(PrctlTiming);
impl_from_c_int!(PrctlTsc);
impl_from_c_int!(PrctlUnalign);

#[link(name="c")]
extern {
    fn prctl(option: c_int, arg2: c_ulong, arg3: c_ulong, arg4: c_ulong, arg5: c_ulong) -> c_int;
}

fn prctl_call(option: PrctlOption) -> Result<(), i32> {
    let res = unsafe {
        prctl(option as c_int, 0, 0, 0, 0)
    };
    handle_errno!(res);
    Ok(())
}

fn prctl_get_flag_arg2(option: PrctlOption) -> Result<bool, i32> {
    let (res, mode) = unsafe {
        let mut mode: c_int = 0;
        let mode_ptr: *mut c_int = &mut mode;
        let r = prctl(option as c_int, mode_ptr as c_ulong, 0, 0, 0);
        (r, mode)
    };
    handle_errno!(res);
    Ok(mode > 0)
}

fn prctl_get_result(option: PrctlOption) -> Result<c_int, i32> {
    let res = unsafe {
        prctl(option as c_int, 0, 0, 0, 0)
    };
    handle_errno!(res);
    Ok(res)
}

fn prctl_get_flag(option: PrctlOption) -> Result<bool, i32> {
    let res = try!(prctl_get_result(option));
    Ok(res > 0)
}

fn prctl_send_arg2(option: PrctlOption, arg2: c_ulong) -> Result<(), i32> {
    let res = unsafe {
        prctl(option as c_int, arg2, 0, 0, 0)
    };
    handle_errno!(res);
    Ok(())
}

fn prctl_send_arg3(option: PrctlOption, arg2: c_ulong, arg3: c_ulong) -> Result<(), i32> {
    let res = unsafe {
        prctl(option as c_int, arg2, arg3, 0, 0)
    };
    handle_errno!(res);
    Ok(())
}

fn prctl_set_flag(option: PrctlOption, val: bool) -> Result<(), i32> {
    prctl_send_arg2(option, val as c_ulong)
}

fn prctl_get_enum<E: FromCInt>(option: PrctlOption) -> Result<E, i32> {
    let (res, mode) = unsafe {
        let mut mode: c_int = 0;
        let mode_ptr: *mut c_int = &mut mode;
        let r = prctl(option as c_int, mode_ptr as c_ulong, 0, 0, 0);
        (r, mode)
    };
    handle_errno!(res);
    let mode_enum = FromCInt::from_c_int(mode);
    Ok(mode_enum)
}

fn prctl_get_enum_value<E: FromCInt>(option: PrctlOption) -> Result<E, i32> {
    let res = try!(prctl_get_result(option));
    let mode_enum = FromCInt::from_c_int(res);
    Ok(mode_enum)
}

fn prctl_set_enum(option: PrctlOption, mode: c_ulong) -> Result<(), i32> {
    let res = unsafe {
        prctl(option as c_int, mode, 0, 0, 0)
    };
    handle_errno!(res);
    Ok(())
}

pub fn get_death_signal() -> Result<isize, i32> {
    let (res, sig) = unsafe {
        let mut sig: c_int = 0;
        let sig_ptr: *mut c_int = &mut sig;
        let r = prctl(PrctlOption::PR_GET_PDEATHSIG as c_int, sig_ptr as c_ulong, 0, 0, 0);
        (r, sig)
    };
    handle_errno!(res);
    Ok(sig as isize)
}

pub fn set_death_signal(signal: isize) -> Result<(), i32> {
    let res = unsafe {
        prctl(PrctlOption::PR_SET_PDEATHSIG as c_int, signal as c_ulong, 0, 0, 0)
    };
    handle_errno!(res);
    Ok(())
}

pub fn get_dumpable() -> Result<bool, i32> {
    prctl_get_flag(PrctlOption::PR_GET_DUMPABLE)
}

pub fn set_dumpable(dumpable: bool) -> Result<(), i32> {
    prctl_set_flag(PrctlOption::PR_SET_DUMPABLE, dumpable)
}

pub fn get_name() -> Result<CString, i32> {
    let (res, name) = unsafe {
        // 16 characters in the name, but null terminate just in case
        // only 15 characters can be set anyway, contrary to prctl doc
        // https://bugzilla.kernel.org/show_bug.cgi?id=11764
        let mut name = [0 as u8; 17];
        let res = prctl(PrctlOption::PR_GET_NAME as c_int, name.as_mut_ptr() as c_ulong, 0, 0, 0);
        // buffer is one bigger than result - there will be one
        let nul_pos = name.iter().position(|x| *x == 0).unwrap();
        let name_slice = &name[0 .. nul_pos];
        (res, CString::new(name_slice).unwrap())
    };
    handle_errno!(res);
    Ok(name)
}

pub fn set_name(name: &str) -> Result<(), i32> {
    let res = unsafe {
        let cname = CString::new(name).unwrap();
        prctl(PrctlOption::PR_SET_NAME as c_int, cname.as_ptr() as c_ulong, 0, 0, 0)
    };
    handle_errno!(res);
    Ok(())
}

pub fn set_no_new_privileges(new_privileges: bool) -> Result<(), i32> {
    prctl_set_flag(PrctlOption::PR_SET_NO_NEW_PRIVS, new_privileges)
}

pub fn get_no_new_privileges() -> Result<bool, i32> {
    prctl_get_flag(PrctlOption::PR_GET_NO_NEW_PRIVS)
}

pub fn set_unaligned_access(mode: PrctlUnalign) -> Result<(), i32> {
    prctl_set_enum(PrctlOption::PR_SET_UNALIGN, mode as c_ulong)
}

pub fn get_unaligned_access() -> Result<PrctlUnalign, i32> {
    prctl_get_enum(PrctlOption::PR_GET_UNALIGN)
}

pub fn set_keep_capabilities(keep_capabilities: bool) -> Result<(), i32> {
    prctl_set_flag(PrctlOption::PR_SET_KEEPCAPS, keep_capabilities)
}

pub fn get_keep_capabilities() -> Result<bool, i32> {
    prctl_get_flag(PrctlOption::PR_GET_KEEPCAPS)
}

pub fn get_fpemu() -> Result<PrctlFpemu, i32> {
    prctl_get_enum(PrctlOption::PR_GET_FPEMU)
}

pub fn set_fpemu(mode: PrctlFpemu) -> Result<(), i32> {
    prctl_set_enum(PrctlOption::PR_SET_FPEMU, mode as c_ulong)
}

pub fn get_timing() -> Result<PrctlTiming, i32> {
    prctl_get_enum_value(PrctlOption::PR_GET_TIMING)
}

pub fn set_timing(timing: PrctlTiming) -> Result<(), i32> {
    prctl_set_enum(PrctlOption::PR_SET_TIMING, timing as c_ulong)
}

pub fn get_endian() -> Result<PrctlEndian, i32> {
    prctl_get_enum(PrctlOption::PR_GET_ENDIAN)
}

pub fn set_endian(endian: PrctlEndian) -> Result<(), i32> {
    prctl_set_enum(PrctlOption::PR_SET_ENDIAN, endian as c_ulong)
}

pub fn get_seccomp() -> Result<PrctlSeccomp, i32> {
    prctl_get_enum_value(PrctlOption::PR_GET_SECCOMP)
}

pub fn set_seccomp_strict() -> Result<(), i32> {
    prctl_set_enum(PrctlOption::PR_SET_SECCOMP, PrctlSeccomp::SECCOMP_MODE_STRICT as c_ulong)
}

// TODO
//pub fn set_seccomp_filter(...) -> Result<(), i32> {
// ...
//}

pub fn get_tsc() -> Result<PrctlTsc, i32> {
    prctl_get_enum(PrctlOption::PR_GET_TSC)
}

pub fn set_tsc(mode: PrctlTsc) -> Result<(), i32> {
    prctl_set_enum(PrctlOption::PR_SET_TSC, mode as c_ulong)
}

pub fn disable_perf_events() -> Result<(), i32> {
    prctl_call(PrctlOption::PR_TASK_PERF_EVENTS_DISABLE)
}

pub fn enable_perf_events() -> Result<(), i32> {
    prctl_call(PrctlOption::PR_TASK_PERF_EVENTS_ENABLE)
}

pub fn get_child_subreaper() -> Result<bool, i32> {
    prctl_get_flag_arg2(PrctlOption::PR_GET_CHILD_SUBREAPER)
}

pub fn set_child_subreaper(subreaper: bool) -> Result<(), i32> {
    prctl_set_flag(PrctlOption::PR_SET_CHILD_SUBREAPER, subreaper)
}

pub fn get_thp_disable() -> Result<bool, i32> {
    prctl_get_flag(PrctlOption::PR_GET_THP_DISABLE)
}

pub fn set_thp_disable(disable: bool) -> Result<(), i32> {
    prctl_set_flag(PrctlOption::PR_SET_THP_DISABLE, disable)
}

pub fn read_capability(cap: PrctlCap) -> Result<bool, i32> {
    let res = unsafe {
        prctl(PrctlOption::PR_CAPBSET_READ as c_int, cap as c_ulong, 0, 0, 0)
    };
    handle_errno!(res);
    Ok(res > 0)
}

pub fn drop_capability(cap: PrctlCap) -> Result<(), i32> {
    prctl_send_arg2(PrctlOption::PR_CAPBSET_DROP, cap as c_ulong)
}

pub fn set_securebits(bits: Vec<PrctlSecurebits>) -> Result<(), i32> {
    let mut mask = 0 as c_ulong;
    for bit in bits.iter() {
        mask |= *bit as c_ulong;
    }

    prctl_send_arg2(PrctlOption::PR_SET_SECUREBITS, mask as c_ulong)
}

pub fn get_securebits() -> Result<Vec<PrctlSecurebits>, i32> {
    let res = try!(prctl_get_result(PrctlOption::PR_GET_SECUREBITS));
    let mut bits: Vec<PrctlSecurebits> = Vec::new();
    for bit in vec!(
            PrctlSecurebits::SECBIT_NOROOT, PrctlSecurebits::SECBIT_NOROOT_LOCKED,
            PrctlSecurebits::SECBIT_NO_SETUID_FIXUP, PrctlSecurebits::SECBIT_NO_SETUID_FIXUP_LOCKED,
            PrctlSecurebits::SECBIT_KEEP_CAPS, PrctlSecurebits::SECBIT_KEEP_CAPS_LOCKED).iter() {

        if res & *bit as c_int != 0 {
            bits.push(*bit);
        }
    }
    Ok(bits)
}

pub fn get_timer_slack() -> Result<usize, i32> {
    let res = try!(prctl_get_result(PrctlOption::PR_GET_TIMERSLACK));
    // res is already not negative - erros were negative
    Ok(res as usize)
}

pub fn set_timer_slack(slack: usize) -> Result<(), i32> {
    prctl_send_arg2(PrctlOption::PR_SET_TIMERSLACK, slack as c_ulong)
}

pub fn clear_mce_kill() -> Result<(), i32> {
    prctl_send_arg2(PrctlOption::PR_MCE_KILL, 0)
}

pub fn set_mce_kill(policy: PrctlMceKill) -> Result<(), i32> {
    prctl_send_arg3(PrctlOption::PR_MCE_KILL, 1, policy as c_ulong)
}

pub fn get_mce_kill() -> Result<PrctlMceKill, i32> {
    prctl_get_enum_value(PrctlOption::PR_MCE_KILL_GET)
}

pub fn set_mm(setting: PrctlMM, value: c_ulong) -> Result<(), i32> {
    prctl_send_arg3(PrctlOption::PR_SET_MM, setting as c_ulong, value)
}

#[test]
fn check_dumpable() {
    let old = get_dumpable().unwrap();
    assert_eq!(Ok(()), set_dumpable(true));
    assert_eq!(Ok(true), get_dumpable());
    assert_eq!(Ok(()), set_dumpable(false));
    assert_eq!(Ok(false), get_dumpable());
    assert_eq!(Ok(()), set_dumpable(old));
}

#[test]
fn check_death_signal() {
    let old = get_death_signal().unwrap();
    assert_eq!(Ok(()), set_death_signal(1));
    assert_eq!(Ok(1), get_death_signal());
    assert_eq!(Ok(()), set_death_signal(2));
    assert_eq!(Ok(2), get_death_signal());
    assert_eq!(Ok(()), set_death_signal(old));
}

#[test]
fn check_name() {
    let old = get_name().unwrap();
    assert_eq!(Ok(()), set_name("fake"));
    assert_eq!("fake", String::from_utf8_lossy(get_name().unwrap().to_bytes()));
    assert_eq!(Ok(()), set_name("veryveryverylon"));
    assert_eq!("veryveryverylon", String::from_utf8_lossy(get_name().unwrap().to_bytes()));
    assert_eq!(Ok(()), set_name(&String::from_utf8_lossy(old.to_bytes())));
}

#[test]
#[cfg(feature = "not_travis")]
fn check_no_new_privileges() {
    assert_eq!(Ok(false), get_no_new_privileges());
}

#[test]
fn check_unalign() {
    // if someone owns an alpha/ppc they can update this...
    assert_eq!(Err(22), get_unaligned_access());
    assert_eq!(Err(22), set_unaligned_access(PrctlUnalign::PR_UNALIGN_SIGBUS));
}

#[test]
fn check_keep_capabilities() {
    assert_eq!(Ok(()), set_keep_capabilities(true));
    assert_eq!(Ok(true), get_keep_capabilities());
    assert_eq!(Ok(()), set_keep_capabilities(false));
    assert_eq!(Ok(false), get_keep_capabilities());
}

#[test]
fn check_fpemu() {
    assert_eq!(Err(22), get_fpemu());
    assert_eq!(Err(22), set_fpemu(PrctlFpemu::PR_FPEMU_SIGFPE));
}

#[test]
fn check_timing() {
    let old = get_timing().unwrap();
    assert_eq!(Ok(()), set_timing(PrctlTiming::PR_TIMING_STATISTICAL));
    assert_eq!(Ok(PrctlTiming::PR_TIMING_STATISTICAL), get_timing());
    assert_eq!(Ok(()), set_timing(old));
}

#[test]
fn check_endian() {
    assert_eq!(Err(22), get_endian());
    assert_eq!(Err(22), set_endian(PrctlEndian::PR_ENDIAN_PPC_LITTLE));
}

#[test]
#[cfg(feature = "not_travis")]
fn check_seccomp() {
    assert_eq!(Ok(PrctlSeccomp::SECCOMP_MODE_DISABLED), get_seccomp());
}

#[test]
fn check_tsc() {
    let old = get_tsc().unwrap();
    assert_eq!(Ok(()), set_tsc(PrctlTsc::PR_TSC_SIGSEGV));
    assert_eq!(Ok(PrctlTsc::PR_TSC_SIGSEGV), get_tsc());
    assert_eq!(Ok(()), set_tsc(PrctlTsc::PR_TSC_ENABLE));
    assert_eq!(Ok(PrctlTsc::PR_TSC_ENABLE), get_tsc());
    assert_eq!(Ok(()), set_tsc(old));
}

#[test]
fn check_perf_events() {
    assert_eq!(Ok(()), disable_perf_events());
}

#[test]
#[cfg(feature = "not_travis")]
fn check_subreaper() {
    assert_eq!(Ok(()), set_child_subreaper(true));
    assert_eq!(Ok(true), get_child_subreaper());
    assert_eq!(Ok(()), set_child_subreaper(false));
    assert_eq!(Ok(false), get_child_subreaper());
}

#[test]
#[cfg(feature = "not_travis")]
fn check_thp() {
    let old = get_thp_disable().unwrap();
    assert_eq!(Ok(()), set_thp_disable(true));
    assert_eq!(Ok(true), get_thp_disable());
    assert_eq!(Ok(()), set_thp_disable(false));
    assert_eq!(Ok(false), get_thp_disable());
    assert_eq!(Ok(()), set_thp_disable(old));
}

#[test]
fn check_capabilities() {
    assert_eq!(Ok(true), read_capability(PrctlCap::CAP_CHOWN));
}

#[test]
fn check_securebits() {
    assert_eq!(Ok(vec!()), get_securebits());
}

#[test]
#[cfg(feature = "root_test")]
fn check_securebits_root() {
    assert_eq!(Ok(()), set_securebits(vec!(PrctlSecurebits::SECBIT_NOROOT)));
    assert_eq!(Ok(vec!(PrctlSecurebits::SECBIT_NOROOT)), get_securebits());
}

#[test]
fn check_timerslack() {
    let old = get_timer_slack().unwrap();
    assert_eq!(Ok(()), set_timer_slack(10000));
    assert_eq!(Ok(10000), get_timer_slack());
    assert_eq!(Ok(()), set_timer_slack(old));
}

#[test]
fn check_mce_kill() {
    let old = get_mce_kill().unwrap();
    assert_eq!(Ok(()), clear_mce_kill());
    assert_eq!(Ok(PrctlMceKill::PR_MCE_KILL_DEFAULT), get_mce_kill());
    assert_eq!(Ok(()), set_mce_kill(PrctlMceKill::PR_MCE_KILL_LATE));
    assert_eq!(Ok(PrctlMceKill::PR_MCE_KILL_LATE), get_mce_kill());
    assert_eq!(Ok(()), set_mce_kill(old));
}
