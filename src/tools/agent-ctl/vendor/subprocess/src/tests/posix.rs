use std::ffi::OsString;

use crate::unix::PopenExt;
use crate::{ExitStatus, Popen, PopenConfig, Redirection};

use libc;

#[test]
fn err_terminate() {
    let mut p = Popen::create(&["sleep", "5"], PopenConfig::default()).unwrap();
    assert!(p.poll().is_none());
    p.terminate().unwrap();
    assert_eq!(p.wait().unwrap(), ExitStatus::Signaled(libc::SIGTERM as u8));
}

#[test]
fn waitpid_echild() {
    let mut p = Popen::create(&["true"], PopenConfig::default()).unwrap();
    let pid = p.pid().unwrap() as i32;
    let mut status = 0 as libc::c_int;
    let wpid = unsafe { libc::waitpid(pid, &mut status, 0) };
    assert_eq!(wpid, pid);
    assert_eq!(status, 0);
    assert_eq!(p.wait().unwrap(), ExitStatus::Undetermined);
}

#[test]
fn send_signal() {
    let mut p = Popen::create(&["sleep", "5"], PopenConfig::default()).unwrap();
    p.send_signal(libc::SIGUSR1).unwrap();
    assert_eq!(p.wait().unwrap(), ExitStatus::Signaled(libc::SIGUSR1 as u8));
}

#[test]
fn env_set_all_1() {
    let mut p = Popen::create(
        &["env"],
        PopenConfig {
            stdout: Redirection::Pipe,
            env: Some(Vec::new()),
            ..Default::default()
        },
    )
    .unwrap();
    let (out, _err) = p.communicate(None).unwrap();
    assert_eq!(out.unwrap(), "");
}

#[test]
fn env_set_all_2() {
    let mut p = Popen::create(
        &["env"],
        PopenConfig {
            stdout: Redirection::Pipe,
            env: Some(vec![(OsString::from("FOO"), OsString::from("bar"))]),
            ..Default::default()
        },
    )
    .unwrap();
    let (out, _err) = p.communicate(None).unwrap();
    assert_eq!(out.unwrap().trim_end(), "FOO=bar");
}
