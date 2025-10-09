// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// Copyright © 2020 Intel Corporation
// Copyright (c) 2025 Alibaba Cloud
// Copyright (c) 2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::convert::TryInto;

use dragonball::{ALL_THREADS, VCPU_THREAD, VMM_THREAD};
use seccompiler::{BpfProgram, SeccompAction, SeccompFilter};

pub fn get_seccomp_filter(thread_type: &str) -> BpfProgram {
    let rules = match thread_type {
        ALL_THREADS => get_process_seccomp_rules(),
        // Add rules for finer-grained restrictions as needed
        VCPU_THREAD => vec![],
        VMM_THREAD => vec![],
        _ => {
            warn!(sl!(), "Unknown thread type for seccomp: {}", thread_type);
            vec![]
        }
    };
    SeccompFilter::new(
        rules.into_iter().collect(),
        SeccompAction::Trap,
        SeccompAction::Allow,
        std::env::consts::ARCH.try_into().unwrap(),
    )
    .and_then(|f| f.try_into())
    .unwrap_or_default()
}

pub fn get_process_seccomp_rules() -> Vec<(i64, Vec<seccompiler::SeccompRule>)> {
    vec![
        (libc::SYS_read, vec![]),
        (libc::SYS_write, vec![]),
        #[cfg(target_arch = "x86_64")]
        (libc::SYS_open, vec![]),
        (libc::SYS_close, vec![]),
        #[cfg(target_arch = "x86_64")]
        (libc::SYS_stat, vec![]),
        (libc::SYS_fstat, vec![]),
        #[cfg(target_arch = "x86_64")]
        (libc::SYS_lstat, vec![]),
        #[cfg(target_arch = "x86_64")]
        (libc::SYS_poll, vec![]),
        (libc::SYS_lseek, vec![]),
        (libc::SYS_mmap, vec![]),
        (libc::SYS_mprotect, vec![]),
        (libc::SYS_munmap, vec![]),
        (libc::SYS_brk, vec![]),
        (libc::SYS_rt_sigaction, vec![]),
        (libc::SYS_rt_sigprocmask, vec![]),
        (libc::SYS_rt_sigreturn, vec![]),
        (libc::SYS_ioctl, vec![]),
        (libc::SYS_pread64, vec![]),
        (libc::SYS_pwrite64, vec![]),
        (libc::SYS_readv, vec![]),
        (libc::SYS_writev, vec![]),
        #[cfg(target_arch = "x86_64")]
        (libc::SYS_access, vec![]),
        (libc::SYS_sched_yield, vec![]),
        (libc::SYS_mremap, vec![]),
        (libc::SYS_mincore, vec![]),
        (libc::SYS_madvise, vec![]),
        (libc::SYS_dup, vec![]),
        (libc::SYS_nanosleep, vec![]),
        (libc::SYS_getpid, vec![]),
        (libc::SYS_socket, vec![]),
        (libc::SYS_connect, vec![]),
        (libc::SYS_sendto, vec![]),
        (libc::SYS_recvfrom, vec![]),
        (libc::SYS_sendmsg, vec![]),
        (libc::SYS_recvmsg, vec![]),
        (libc::SYS_shutdown, vec![]),
        (libc::SYS_bind, vec![]),
        (libc::SYS_listen, vec![]),
        (libc::SYS_socketpair, vec![]),
        (libc::SYS_setsockopt, vec![]),
        (libc::SYS_getsockopt, vec![]),
        (libc::SYS_clone, vec![]),
        (libc::SYS_exit, vec![]),
        (libc::SYS_wait4, vec![]),
        (libc::SYS_kill, vec![]),
        (libc::SYS_fcntl, vec![]),
        (libc::SYS_fsync, vec![]),
        (libc::SYS_fdatasync, vec![]),
        (libc::SYS_ftruncate, vec![]),
        (libc::SYS_getcwd, vec![]),
        #[cfg(target_arch = "x86_64")]
        (libc::SYS_mkdir, vec![]),
        #[cfg(target_arch = "x86_64")]
        (libc::SYS_rmdir, vec![]),
        #[cfg(target_arch = "x86_64")]
        (libc::SYS_unlink, vec![]),
        #[cfg(target_arch = "x86_64")]
        (libc::SYS_readlink, vec![]),
        (libc::SYS_umask, vec![]),
        (libc::SYS_gettimeofday, vec![]),
        (libc::SYS_getuid, vec![]),
        #[cfg(target_arch = "x86_64")]
        (libc::SYS_getpgrp, vec![]),
        (libc::SYS_setsid, vec![]),
        (libc::SYS_getpgid, vec![]),
        (libc::SYS_sigaltstack, vec![]),
        (libc::SYS_statfs, vec![]),
        (libc::SYS_prctl, vec![]),
        (libc::SYS_mount, vec![]),
        (libc::SYS_umount2, vec![]),
        (libc::SYS_gettid, vec![]),
        (libc::SYS_getxattr, vec![]),
        (libc::SYS_tkill, vec![]),
        (libc::SYS_futex, vec![]),
        (libc::SYS_sched_setaffinity, vec![]),
        (libc::SYS_sched_getaffinity, vec![]),
        (libc::SYS_io_setup, vec![]),
        (libc::SYS_io_destroy, vec![]),
        (libc::SYS_io_getevents, vec![]),
        (libc::SYS_io_submit, vec![]),
        (libc::SYS_io_cancel, vec![]),
        (libc::SYS_getdents64, vec![]),
        (libc::SYS_restart_syscall, vec![]),
        (libc::SYS_clock_gettime, vec![]),
        (libc::SYS_clock_nanosleep, vec![]),
        (libc::SYS_exit_group, vec![]),
        #[cfg(target_arch = "x86_64")]
        (libc::SYS_epoll_wait, vec![]),
        (libc::SYS_epoll_ctl, vec![]),
        (libc::SYS_tgkill, vec![]),
        (libc::SYS_mbind, vec![]),
        (libc::SYS_openat, vec![]),
        (libc::SYS_mkdirat, vec![]),
        (libc::SYS_newfstatat, vec![]),
        (libc::SYS_unlinkat, vec![]),
        (libc::SYS_readlinkat, vec![]),
        (libc::SYS_faccessat, vec![]),
        (libc::SYS_ppoll, vec![]),
        (libc::SYS_set_robust_list, vec![]),
        (libc::SYS_utimensat, vec![]),
        (libc::SYS_epoll_pwait, vec![]),
        (libc::SYS_timerfd_create, vec![]),
        (libc::SYS_fallocate, vec![]),
        (libc::SYS_timerfd_settime, vec![]),
        (libc::SYS_accept4, vec![]),
        (libc::SYS_eventfd2, vec![]),
        (libc::SYS_epoll_create1, vec![]),
        (libc::SYS_preadv, vec![]),
        (libc::SYS_pwritev, vec![]),
        (libc::SYS_prlimit64, vec![]),
        (libc::SYS_setns, vec![]),
        (libc::SYS_seccomp, vec![]),
        (libc::SYS_getrandom, vec![]),
        (libc::SYS_memfd_create, vec![]),
        (libc::SYS_statx, vec![]),
        #[cfg(target_arch = "x86_64")]
        (libc::SYS_rseq, vec![]),
        #[cfg(target_arch = "aarch64")]
        (293, vec![]),
        (libc::SYS_io_uring_setup, vec![]),
        (libc::SYS_io_uring_enter, vec![]),
        (libc::SYS_io_uring_register, vec![]),
        (libc::SYS_clone3, vec![]),
        (libc::SYS_close_range, vec![]),
        (libc::SYS_landlock_create_ruleset, vec![]),
        (libc::SYS_landlock_add_rule, vec![]),
        (libc::SYS_landlock_restrict_self, vec![]),
        (libc::SYS_fchownat, vec![]),
        (libc::SYS_renameat, vec![]),
        (libc::SYS_fchmodat, vec![]),
        #[cfg(target_arch = "x86_64")]
        (libc::SYS_pipe, vec![]),
        #[cfg(target_arch = "x86_64")]
        (libc::SYS_arch_prctl, vec![]),
        #[cfg(target_arch = "x86_64")]
        (libc::SYS_dup2, vec![]),
        (libc::SYS_dup3, vec![]),
        (libc::SYS_pipe2, vec![]),
        (libc::SYS_pidfd_send_signal, vec![]),
        (libc::SYS_pidfd_open, vec![]),
        (libc::SYS_getsockname, vec![]),
        (libc::SYS_getpeername, vec![]),
        (libc::SYS_faccessat2, vec![]),
        #[cfg(target_arch = "x86_64")]
        (libc::SYS_fork, vec![]),
        (libc::SYS_execve, vec![]),
        (libc::SYS_uname, vec![]),
        (libc::SYS_copy_file_range, vec![]),
        (libc::SYS_flock, vec![]),
        (libc::SYS_set_tid_address, vec![]),
        (libc::SYS_getrlimit, vec![]),
        (libc::SYS_getgid, vec![]),
        (libc::SYS_geteuid, vec![]),
        (libc::SYS_getegid, vec![]),
        (libc::SYS_getppid, vec![]),
        (libc::SYS_getresuid, vec![]),
        (libc::SYS_waitid, vec![]),
        (libc::SYS_getresgid, vec![]),
        (libc::SYS_capget, vec![]),
        (libc::SYS_linkat, vec![]),
        (libc::SYS_fstatfs, vec![]),
        (libc::SYS_symlinkat, vec![]),
        (libc::SYS_setxattr, vec![]),
        (libc::SYS_setresuid, vec![]),
        (libc::SYS_setresgid, vec![]),
        (libc::SYS_renameat2, vec![]),
        (libc::SYS_capset, vec![]),
        (libc::SYS_mknodat, vec![]),
        (libc::SYS_readahead, vec![]),
        (libc::SYS_removexattr, vec![]),
        (libc::SYS_inotify_init1, vec![]),
        (libc::SYS_inotify_add_watch, vec![]),
        (libc::SYS_unshare, vec![]),
        (libc::SYS_pivot_root, vec![]),
        (libc::SYS_chroot, vec![]),
        (libc::SYS_fchmod, vec![]),
        #[cfg(target_arch = "x86_64")]
        (libc::SYS_chmod, vec![]),
        #[cfg(target_arch = "x86_64")]
        (libc::SYS_fchmodat2, vec![]),
    ]
}
