// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

//
// Core Scheduling landed in linux 5.14, this enables that -
// ONLY the processes have the same cookie value can share an SMT core for security
// reasons, since SMT siblings share their cpu caches and many other things. This can
// prevent some malicious processes steal others' private information.
//
// This is enabled by containerd, see https://github.com/containerd/containerd/blob/main/docs/man/containerd-config.toml.5.md#format
//
// This is done by using system call prctl(), for core scheduling purpose, it is defined as
// int prctl(PR_SCHED_CORE, int cs_command, pid_t pid, enum pid_type type,
//           unsigned long *cookie);
//
// You may go to https://lwn.net/Articles/861251/, https://lore.kernel.org/lkml/20210422123309.039845339@infradead.org/
// and kernel.org/doc/html/latest/admin-guide/hw-vuln/core-scheduling.html for more info.
//

use anyhow::Result;
use nix::{self, errno::Errno};

#[allow(dead_code)]
pub const PID_GROUP: usize = 0;
#[allow(dead_code)]
pub const THREAD_GROUP: usize = 1;
pub const PROCESS_GROUP: usize = 2;

#[allow(dead_code)]
pub const PR_SCHED_CORE: i32 = 62;
pub const PR_SCHED_CORE_CREATE: usize = 1;
pub const PR_SCHED_CORE_SHARE_FROM: usize = 3;

// create a new core sched domain, this will NOT succeed if kernel version < 5.14
pub fn core_sched_create(pidtype: usize) -> Result<(), Errno> {
    let ret =
        unsafe { nix::libc::prctl(PR_SCHED_CORE, PR_SCHED_CORE_CREATE, 0usize, pidtype, 0usize) };
    if ret != 0 {
        Err(Errno::last())
    } else {
        Ok(())
    }
}

// shares the domain with *pid*
#[allow(dead_code)]
pub fn core_sched_share_from(pid: usize, pidtype: usize) -> Result<(), Errno> {
    let ret = unsafe {
        nix::libc::prctl(
            PR_SCHED_CORE,
            PR_SCHED_CORE_SHARE_FROM,
            pid,
            pidtype,
            0usize,
        )
    };
    if ret != 0 {
        Err(Errno::last())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{os::unix::process::CommandExt, process::Command};

    use super::*;

    fn cookie() -> Result<u64, Errno> {
        let mut cookie = 0u64;
        let ret = unsafe {
            nix::libc::prctl(
                PR_SCHED_CORE,
                0usize, // PR_SCHED_CORE_GET
                0usize,
                PID_GROUP,
                &mut cookie as *mut u64,
            )
        };
        if ret != 0 {
            Err(Errno::last())
        } else {
            Ok(cookie)
        }
    }

    #[test]
    fn test_core_sched_cookie_is_inherited() {
        const ROLE: &str = "KATA_CORE_SCHED_INHERITANCE_TEST";
        let mut child = Command::new(std::env::current_exe().unwrap());
        child.args([
            "--exact",
            "core_sched::tests::test_core_sched_cookie_is_inherited",
            "--nocapture",
            "--test-threads=1",
        ]);
        match std::env::var(ROLE) {
            Err(_) => {
                // PR_SCHED_CORE_CREATE applies to the process group. Isolate
                // this experiment from the test runner and its other children.
                let output = child.env(ROLE, "create").process_group(0).output().unwrap();
                assert!(output.status.success(), "{:?}", output);
                print!("{}", String::from_utf8_lossy(&output.stdout));
                eprint!("{}", String::from_utf8_lossy(&output.stderr));
            }
            Ok(role) if role == "create" => {
                match core_sched_create(PROCESS_GROUP) {
                    Ok(()) => {}
                    Err(Errno::EINVAL | Errno::ENODEV) => {
                        eprintln!(
                            "SKIP cookie inheritance: kernel/core scheduling or SMT unavailable"
                        );
                        return;
                    }
                    Err(error) => panic!("create core scheduling cookie: {}", error),
                }
                let expected = cookie().unwrap();
                assert_ne!(expected, 0);
                let output = child.env(ROLE, expected.to_string()).output().unwrap();
                assert!(output.status.success(), "{:?}", output);
                assert_eq!(cookie().unwrap(), expected);
            }
            Ok(expected) => {
                assert_eq!(cookie().unwrap(), expected.parse::<u64>().unwrap());
            }
        }
    }

    // An invalid scope fails without changing the test process's cookie.
    // Kernels without SMT can return ENODEV before checking the scope. Both
    // errors must survive libc returning -1 rather than a negative errno.
    #[test]
    fn test_core_sched_create_errno() {
        assert!(matches!(
            core_sched_create(usize::MAX),
            Err(Errno::EINVAL | Errno::ENODEV)
        ));
    }

    #[test]
    fn test_core_sched_share_from_errno() {
        assert!(matches!(
            core_sched_share_from(0, usize::MAX),
            Err(Errno::EINVAL | Errno::ENODEV)
        ));
    }
}
