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
    let errno = unsafe { nix::libc::prctl(PR_SCHED_CORE, PR_SCHED_CORE_CREATE, 0, pidtype, 0) };
    if errno != 0 {
        Err(nix::errno::Errno::from_i32(-errno))
    } else {
        Ok(())
    }
}

// shares the domain with *pid*
#[allow(dead_code)]
pub fn core_sched_share_from(pid: usize, pidtype: usize) -> Result<(), Errno> {
    let errno =
        unsafe { nix::libc::prctl(PR_SCHED_CORE, PR_SCHED_CORE_SHARE_FROM, pid, pidtype, 0) };
    if errno != 0 {
        Err(nix::errno::Errno::from_i32(-errno))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::errno::Errno::{EINVAL, ENODEV, ENOMEM, EPERM, ESRCH};

    const RELEASE_MAJOR_VERSION: u8 = 5;
    const RELEASE_MINOR_VERSION: u8 = 14;

    // since this feature only lands in linux 5.14, we run the test when version is higher
    fn core_sched_landed() -> bool {
        let vinfo = std::fs::read_to_string("/proc/sys/kernel/osrelease");
        if let Ok(info) = vinfo {
            let vnum: Vec<&str> = info.as_str().split('.').collect();
            if vnum.len() >= 2 {
                let major: u8 = vnum[0].parse().unwrap();
                let minor: u8 = vnum[1].parse().unwrap();
                return major >= RELEASE_MAJOR_VERSION && minor >= RELEASE_MINOR_VERSION;
            }
        }
        false
    }

    #[test]
    fn test_core_sched() {
        std::env::set_var("SCHED_CORE", "1");
        assert_eq!(std::env::var("SCHED_CORE").unwrap(), "1");
        if core_sched_landed() {
            // it is possible that the machine running this test does not support SMT,
            // therefore it does not make sense to assert a successful prctl call
            // but we can still make sure that the return value is a possible value
            let e = core_sched_create(PROCESS_GROUP);
            if let Err(errno) = e {
                if errno != EINVAL
                    && errno != ENODEV
                    && errno != ENOMEM
                    && errno != EPERM
                    && errno != ESRCH
                {
                    panic!("impossible return value {:?}", errno);
                }
            }
        }
    }
}
