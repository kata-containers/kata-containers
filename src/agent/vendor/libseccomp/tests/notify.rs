#![cfg(libseccomp_v2_5)]

use libc::{dup3, O_CLOEXEC};
use libseccomp::*;
use std::thread;

macro_rules! skip_if_not_supported {
    () => {
        if !check_api(6, ScmpVersion::from((2, 5, 0))).unwrap() {
            println!("Skip tests for userspace notification");
            return;
        }
    };
}

#[derive(Debug)]
struct TestData {
    syscall: ScmpSyscall,
    args: Vec<u64>,
    arch: ScmpArch,
    resp_val: i64,
    resp_err: i32,
    resp_flags: u32,
    expected_val: i64,
}

#[test]
fn test_user_notification() {
    skip_if_not_supported!();

    let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow).unwrap();
    let syscall = ScmpSyscall::from_name("dup3").unwrap();
    let arch = ScmpArch::native();

    ctx.add_arch(arch).unwrap();
    ctx.add_rule(ScmpAction::Notify, syscall).unwrap();

    let tests = &[
        TestData {
            syscall,
            args: vec![0, 100, O_CLOEXEC as u64],
            arch,
            resp_val: 10,
            resp_err: 0,
            resp_flags: 0,
            expected_val: 10,
        },
        TestData {
            syscall,
            args: vec![0, 100, O_CLOEXEC as u64],
            arch,
            resp_val: 0,
            resp_err: -1,
            resp_flags: 0,
            expected_val: -1,
        },
        TestData {
            syscall,
            args: vec![0, 100, O_CLOEXEC as u64],
            arch,
            resp_val: 0,
            resp_err: 0,
            resp_flags: ScmpNotifRespFlags::CONTINUE.bits(),
            expected_val: 100,
        },
    ];

    ctx.load().unwrap();

    let fd = ctx.get_notify_fd().unwrap();

    let mut handlers = vec![];

    for test in tests.iter() {
        let args: (i32, i32, i32) = (
            test.args[0] as i32,
            test.args[1] as i32,
            test.args[2] as i32,
        );

        handlers.push(thread::spawn(move || unsafe {
            dup3(args.0, args.1, args.2)
        }));

        let req = ScmpNotifReq::receive(fd).unwrap();

        // Checks architecture
        assert_eq!(req.data.arch, test.arch);

        // Checks the number of syscall
        assert_eq!(req.data.syscall, test.syscall);

        // Checks syscall arguments
        for (i, test_val) in test.args.iter().enumerate() {
            assert_eq!(&req.data.args[i], test_val);
        }

        // Checks TOCTOU
        assert!(notify_id_valid(fd, req.id).is_ok());

        let resp = ScmpNotifResp::new(req.id, test.resp_val, test.resp_err, test.resp_flags);
        resp.respond(fd).unwrap();
    }

    // Checks return value
    for (i, handler) in handlers.into_iter().enumerate() {
        let ret_val = handler.join().unwrap();
        assert_eq!(tests[i].expected_val as i32, ret_val);
    }
}

#[test]
fn test_resp_new() {
    assert_eq!(
        ScmpNotifResp::new_val(1234, 1, ScmpNotifRespFlags::empty()),
        ScmpNotifResp::new(1234, 1, 0, 0),
    );
    assert_eq!(
        ScmpNotifResp::new_error(1234, -2, ScmpNotifRespFlags::empty()),
        ScmpNotifResp::new(1234, 0, -2, 0),
    );
    assert_eq!(
        ScmpNotifResp::new_continue(1234, ScmpNotifRespFlags::empty()),
        ScmpNotifResp::new(1234, 0, 0, ScmpNotifRespFlags::CONTINUE.bits()),
    );
}

#[test]
fn test_error() {
    skip_if_not_supported!();

    let ctx = ScmpFilterContext::new_filter(ScmpAction::Allow).unwrap();
    let resp = ScmpNotifResp::new(0, 0, 0, 0);

    assert!(ctx.get_notify_fd().is_err());
    assert!(ScmpNotifReq::receive(0).is_err());
    assert!(resp.respond(0).is_err());
    assert!(notify_id_valid(0, 0).is_err());
}

#[test]
fn resp_flags_from_bits_preserve() {
    assert_eq!(
        ScmpNotifRespFlags::from_bits_preserve(u32::MAX).bits(),
        u32::MAX
    );
}
