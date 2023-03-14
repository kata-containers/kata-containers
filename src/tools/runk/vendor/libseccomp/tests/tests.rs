use libseccomp::*;
use std::io::{stdout, Error};

#[cfg(feature = "const-syscall")]
mod known_syscall_names;

macro_rules! syscall_assert {
    ($e1: expr, $e2: expr) => {
        let mut errno: i32 = 0;
        if $e1 < 0 {
            errno = -Error::last_os_error().raw_os_error().unwrap()
        }
        assert_eq!(errno, $e2);
    };
}

#[test]
fn test_check_version() {
    assert!(check_version(ScmpVersion::from((2, 4, 0))).unwrap());
    assert!(!check_version(ScmpVersion::from((100, 100, 100))).unwrap());
}

#[test]
fn test_check_api() {
    assert!(check_api(3, ScmpVersion::from((2, 4, 0))).unwrap());
    assert!(!check_api(100, ScmpVersion::from((2, 4, 0))).unwrap());
}

#[test]
#[allow(deprecated)]
fn test_get_library_version() {
    let ret = ScmpVersion::current().unwrap();
    assert_eq!(ret, get_library_version().unwrap());
    println!(
        "test_get_library_version: {}.{}.{}",
        ret.major, ret.minor, ret.micro
    );
}

#[test]
fn test_scmparch_native() {
    let ret = ScmpArch::native();
    println!("test_get_native_arch: native arch is {:?}", ret);
}

#[test]
fn test_get_api() {
    let ret = get_api();
    println!("test_get_api: Got API level of {}", ret);
}

#[test]
fn test_set_api() {
    set_api(1).unwrap();
    assert_eq!(get_api(), 1);

    assert!(set_api(1000).is_err());
}

#[test]
fn test_set_syscall_priority() {
    let mut ctx = ScmpFilterContext::new_filter(ScmpAction::KillThread).unwrap();
    let syscall = ScmpSyscall::from_name("open").unwrap();
    let priority = 100;

    assert!(ctx.set_syscall_priority(syscall, priority).is_ok());
    assert!(ctx.set_syscall_priority(-1, priority).is_err());
}

#[test]
#[allow(deprecated)]
fn test_filter_attributes() {
    let mut ctx = ScmpFilterContext::new_filter(ScmpAction::KillThread).unwrap();

    // Test for CtlNnp
    ctx.set_ctl_nnp(false).unwrap();
    let ret = ctx.get_ctl_nnp().unwrap();
    assert!(!ret);

    ctx.set_no_new_privs_bit(true).unwrap();
    assert!(ctx.get_no_new_privs_bit().unwrap());

    // Test for ActBadArch
    let test_actions = [
        ScmpAction::Trap,
        ScmpAction::Errno(libc::EACCES),
        ScmpAction::Trace(10),
    ];
    for action in test_actions {
        ctx.set_act_badarch(action).unwrap();
        let ret = ctx.get_act_badarch().unwrap();
        assert_eq!(ret, action);
    }

    // Test for ActDefault
    let ret = ctx.get_act_default().unwrap();
    assert_eq!(ret, ScmpAction::KillThread);

    // Test for CtlLog
    if check_api(3, ScmpVersion::from((2, 4, 0))).unwrap() {
        ctx.set_ctl_log(true).unwrap();
        let ret = ctx.get_ctl_log().unwrap();
        assert!(ret);
    } else {
        assert!(ctx.set_ctl_log(true).is_err());
        assert!(ctx.get_ctl_log().is_err());
    }

    // Test for CtlSsb
    if check_api(4, ScmpVersion::from((2, 5, 0))).unwrap() {
        ctx.set_ctl_ssb(true).unwrap();
        let ret = ctx.get_ctl_ssb().unwrap();
        assert!(ret);
    } else {
        assert!(ctx.set_ctl_ssb(true).is_err());
        assert!(ctx.get_ctl_ssb().is_err());
    }

    // Test for CtlOptimize
    let opt_level = 2;
    if check_api(4, ScmpVersion::from((2, 5, 0))).unwrap() {
        ctx.set_ctl_optimize(opt_level).unwrap();
        let ret = ctx.get_ctl_optimize().unwrap();
        assert_eq!(ret, opt_level);
    } else {
        assert!(ctx.set_ctl_optimize(opt_level).is_err());
        assert!(ctx.get_ctl_optimize().is_err());
    }

    // Test for ApiSysRawRc
    if check_api(4, ScmpVersion::from((2, 5, 0))).unwrap() {
        ctx.set_api_sysrawrc(true).unwrap();
        let ret = ctx.get_api_sysrawrc().unwrap();
        assert!(ret);
    } else {
        assert!(ctx.set_api_sysrawrc(true).is_err());
        assert!(ctx.get_api_sysrawrc().is_err());
    }

    // Test for CtlTsync
    if check_api(2, ScmpVersion::from((2, 2, 0))).unwrap() {
        ctx.set_ctl_tsync(true).unwrap();
        let ret = ctx.get_ctl_tsync().unwrap();
        assert!(ret);
    } else {
        assert!(ctx.set_ctl_tsync(true).is_err());
        assert!(ctx.get_ctl_tsync().is_err());
    }
}

#[test]
fn test_filter_reset() {
    let mut ctx = ScmpFilterContext::new_filter(ScmpAction::KillThread).unwrap();
    ctx.reset(ScmpAction::Allow).unwrap();

    let action = ctx.get_act_default().unwrap();

    let expected_action = ScmpAction::Allow;

    assert_eq!(expected_action, action);
}

#[test]
fn test_syscall_i32() {
    assert_eq!(4_i32, i32::from(ScmpSyscall::from(4)));
}

#[test]
fn test_syscall_eq_i32() {
    assert_eq!(ScmpSyscall::from(4), 4);
    assert_eq!(4, ScmpSyscall::from(4));
    assert_ne!(ScmpSyscall::from(4), 5);
    assert_ne!(4, ScmpSyscall::from(5));
}

#[test]
#[allow(deprecated)]
fn test_get_syscall_name_from_arch() {
    assert_eq!(
        "openat",
        ScmpSyscall::from(56)
            .get_name_by_arch(ScmpArch::Aarch64)
            .unwrap()
    );

    assert!(ScmpSyscall::from(10_000).get_name().is_err());
    assert!(ScmpSyscall::from(-1).get_name().is_err());

    assert_eq!(
        get_syscall_name_from_arch(ScmpArch::Native, 5).unwrap(),
        ScmpSyscall::from(5).get_name().unwrap(),
    );

    assert_eq!(
        get_syscall_name_from_arch(ScmpArch::Aarch64, 5).unwrap(),
        ScmpSyscall::from(5)
            .get_name_by_arch(ScmpArch::Aarch64)
            .unwrap(),
    );
}

#[test]
#[allow(deprecated)]
fn test_get_syscall_from_name() {
    assert_eq!(
        ScmpSyscall::from_name_by_arch("openat", ScmpArch::Aarch64).unwrap(),
        56,
    );

    assert_eq!(
        ScmpSyscall::from_name_by_arch_rewrite("socket", ScmpArch::X86).unwrap(),
        102,
    );

    assert!(ScmpSyscall::from_name("no_syscall").is_err());
    assert!(ScmpSyscall::from_name("null\0byte").is_err());
    assert!(ScmpSyscall::from_name_by_arch_rewrite("no_syscall", ScmpArch::X32).is_err());
    assert!(ScmpSyscall::from_name_by_arch_rewrite("null\0byte", ScmpArch::X32).is_err());

    assert_eq!(
        get_syscall_from_name("openat", None).unwrap(),
        ScmpSyscall::from_name("openat").unwrap(),
    );
    assert_eq!(
        get_syscall_from_name("openat", Some(ScmpArch::Aarch64)).unwrap(),
        ScmpSyscall::from_name_by_arch("openat", ScmpArch::Aarch64).unwrap(),
    );
}

#[test]
#[cfg(feature = "const-syscall")]
fn test_syscall_new() {
    for name in known_syscall_names::KNOWN_SYSCALL_NAMES {
        if let Ok(nr) = ScmpSyscall::from_name(name) {
            assert_eq!(ScmpSyscall::new(name), nr);
        }
    }
    assert_eq!(ScmpSyscall::new(""), libseccomp_sys::__NR_SCMP_ERROR);
    assert_eq!(
        ScmpSyscall::new("unkown_syscall"),
        libseccomp_sys::__NR_SCMP_ERROR,
    );
}

#[test]
fn test_display_syscall() {
    assert_eq!(format!("{}", ScmpSyscall::from(4)), "4");
}

#[test]
fn test_arch_functions() {
    let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow).unwrap();
    ctx.add_arch(ScmpArch::X86).unwrap();
    let ret = ctx.is_arch_present(ScmpArch::X86).unwrap();
    assert!(ret);

    ctx.remove_arch(ScmpArch::X86).unwrap();
    let ret = ctx.is_arch_present(ScmpArch::X86).unwrap();
    assert!(!ret);
}

#[test]
fn test_merge_filters() {
    let mut ctx1 = ScmpFilterContext::new_filter(ScmpAction::Allow).unwrap();
    let mut ctx2 = ScmpFilterContext::new_filter(ScmpAction::Allow).unwrap();
    let native_arch = ScmpArch::native();
    let mut prospective_arch = ScmpArch::Aarch64;

    if native_arch == ScmpArch::Aarch64 {
        prospective_arch = ScmpArch::X8664;
    }

    ctx2.add_arch(prospective_arch).unwrap();

    // In order to merge two filters, both filters must have no
    // overlapping architectures.
    // Therefore, need to remove the native arch.
    ctx2.remove_arch(native_arch).unwrap();

    ctx1.merge(ctx2).unwrap();

    let ret = ctx1.is_arch_present(prospective_arch).unwrap();
    assert!(ret);
}

#[test]
fn test_export_functions() {
    let ctx = ScmpFilterContext::new_filter(ScmpAction::Allow).unwrap();

    assert!(ctx.export_bpf(&mut stdout()).is_ok());
    assert!(ctx.export_bpf(&mut -1).is_err());

    assert!(ctx.export_pfc(&mut stdout()).is_ok());
    assert!(ctx.export_pfc(&mut -1).is_err());
}

#[test]
fn test_rule_add_load() {
    let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow).unwrap();
    ctx.add_arch(ScmpArch::Native).unwrap();

    let syscall = ScmpSyscall::from_name("dup3").unwrap();

    ctx.add_rule(ScmpAction::Errno(10), syscall).unwrap();
    ctx.load().unwrap();

    syscall_assert!(unsafe { libc::dup3(0, 100, libc::O_CLOEXEC) }, -10);
}

#[test]
fn test_rule_add_array_load() {
    let mut cmps: Vec<ScmpArgCompare> = Vec::new();
    let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow).unwrap();
    ctx.add_arch(ScmpArch::Native).unwrap();

    let syscall = ScmpSyscall::from_name("process_vm_readv").unwrap();

    let cmp1 = ScmpArgCompare::new(0, ScmpCompareOp::Equal, 10);
    let cmp2 = ScmpArgCompare::new(2, ScmpCompareOp::Equal, 20);

    cmps.push(cmp1);
    cmps.push(cmp2);

    ctx.add_rule_conditional(ScmpAction::Errno(111), syscall, &cmps)
        .unwrap();

    ctx.load().unwrap();

    syscall_assert!(
        unsafe { libc::process_vm_readv(10, std::ptr::null(), 0, std::ptr::null(), 0, 0) },
        0
    );
    syscall_assert!(
        unsafe { libc::process_vm_readv(10, std::ptr::null(), 20, std::ptr::null(), 0, 0) },
        -111
    );
}

#[test]
fn test_rule_add_exact_load() {
    let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow).unwrap();
    ctx.add_arch(ScmpArch::Native).unwrap();

    let syscall = ScmpSyscall::from_name("dup3").unwrap();

    ctx.add_rule_exact(ScmpAction::Errno(10), syscall).unwrap();
    ctx.load().unwrap();

    syscall_assert!(unsafe { libc::dup3(0, 100, libc::O_CLOEXEC) }, -10);
}

#[test]
fn test_rule_add_exact_array_load() {
    let mut cmps: Vec<ScmpArgCompare> = Vec::new();
    let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow).unwrap();
    ctx.add_arch(ScmpArch::Native).unwrap();

    let syscall = ScmpSyscall::from_name("process_vm_readv").unwrap();

    let cmp1 = ScmpArgCompare::new(0, ScmpCompareOp::Equal, 10);
    let cmp2 = ScmpArgCompare::new(2, ScmpCompareOp::Equal, 20);

    cmps.push(cmp1);
    cmps.push(cmp2);

    ctx.add_rule_conditional_exact(ScmpAction::Errno(111), syscall, &cmps)
        .unwrap();

    ctx.load().unwrap();

    syscall_assert!(
        unsafe { libc::process_vm_readv(10, std::ptr::null(), 0, std::ptr::null(), 0, 0) },
        0
    );
    syscall_assert!(
        unsafe { libc::process_vm_readv(10, std::ptr::null(), 20, std::ptr::null(), 0, 0) },
        -111
    );
}
