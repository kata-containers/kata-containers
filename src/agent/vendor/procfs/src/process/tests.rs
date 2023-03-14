use super::*;

fn check_unwrap<T>(prc: &Process, val: ProcResult<T>) -> Option<T> {
    match val {
        Ok(t) => Some(t),
        Err(ProcError::PermissionDenied(_)) if !rustix::process::geteuid().is_root() => {
            // we are not root, and so a permission denied error is OK
            None
        }
        Err(ProcError::NotFound(path)) => {
            // a common reason for this error is that the process isn't running anymore
            if prc.is_alive() {
                panic!("{:?} not found", path)
            }
            None
        }
        Err(err) => panic!("check_unwrap error for {} {:?}", prc.pid, err),
    }
}

fn check_unwrap_task<T>(prc: &Process, val: ProcResult<T>) -> Option<T> {
    match val {
        Ok(t) => Some(t),
        Err(ProcError::PermissionDenied(_)) if !rustix::process::geteuid().is_root() => {
            // we are not root, and so a permission denied error is OK
            None
        }
        Err(ProcError::NotFound(_path)) => {
            // tasks can be more short-lived thanks processes, and it seems that accessing
            // the /status and /stat files for tasks is quite unreliable
            None
        }
        Err(err) => panic!("check_unwrap error for {} {:?}", prc.pid, err),
    }
}

#[test]
fn test_main_thread_task() {
    let myself = Process::myself().unwrap();
    let task = myself.task_main_thread().unwrap();
    check_unwrap(&myself, task.stat());
}

#[allow(clippy::cognitive_complexity)]
#[test]
fn test_self_proc() {
    let myself = Process::myself().unwrap().stat().unwrap();
    println!("{:#?}", myself);
    println!("state: {:?}", myself.state());
    println!("tty: {:?}", myself.tty_nr());
    println!("flags: {:?}", myself.flags());

    #[cfg(feature = "chrono")]
    println!("starttime: {:#?}", myself.starttime());

    let kernel = KernelVersion::current().unwrap();

    if kernel >= KernelVersion::new(2, 1, 22) {
        assert!(myself.exit_signal.is_some());
    } else {
        assert!(myself.exit_signal.is_none());
    }

    if kernel >= KernelVersion::new(2, 2, 8) {
        assert!(myself.processor.is_some());
    } else {
        assert!(myself.processor.is_none());
    }

    if kernel >= KernelVersion::new(2, 5, 19) {
        assert!(myself.rt_priority.is_some());
    } else {
        assert!(myself.rt_priority.is_none());
    }

    if kernel >= KernelVersion::new(2, 5, 19) {
        assert!(myself.rt_priority.is_some());
        assert!(myself.policy.is_some());
    } else {
        assert!(myself.rt_priority.is_none());
        assert!(myself.policy.is_none());
    }

    if kernel >= KernelVersion::new(2, 6, 18) {
        assert!(myself.delayacct_blkio_ticks.is_some());
    } else {
        assert!(myself.delayacct_blkio_ticks.is_none());
    }

    if kernel >= KernelVersion::new(2, 6, 24) {
        assert!(myself.guest_time.is_some());
        assert!(myself.cguest_time.is_some());
    } else {
        assert!(myself.guest_time.is_none());
        assert!(myself.cguest_time.is_none());
    }

    if kernel >= KernelVersion::new(3, 3, 0) {
        assert!(myself.start_data.is_some());
        assert!(myself.end_data.is_some());
        assert!(myself.start_brk.is_some());
    } else {
        assert!(myself.start_data.is_none());
        assert!(myself.end_data.is_none());
        assert!(myself.start_brk.is_none());
    }

    if kernel >= KernelVersion::new(3, 5, 0) {
        assert!(myself.arg_start.is_some());
        assert!(myself.arg_end.is_some());
        assert!(myself.env_start.is_some());
        assert!(myself.env_end.is_some());
        assert!(myself.exit_code.is_some());
    } else {
        assert!(myself.arg_start.is_none());
        assert!(myself.arg_end.is_none());
        assert!(myself.env_start.is_none());
        assert!(myself.env_end.is_none());
        assert!(myself.exit_code.is_none());
    }
}

#[test]
fn test_all() {
    let is_wsl2 = kernel_config()
        .ok()
        .and_then(|cfg| {
            cfg.get("CONFIG_LOCALVERSION").and_then(|ver| {
                if let ConfigSetting::Value(s) = ver {
                    Some(s == "\"-microsoft-standard\"")
                } else {
                    None
                }
            })
        })
        .unwrap_or(false);
    for p in all_processes().unwrap() {
        // note: this test doesn't unwrap, since some of this data requires root to access
        // so permission denied errors are common.  The check_unwrap helper function handles
        // this.

        let prc = p.unwrap();
        let stat = prc.stat().unwrap();
        println!("{} {}", prc.pid(), stat.comm);
        stat.flags().unwrap();
        stat.state().unwrap();
        #[cfg(feature = "chrono")]
        stat.starttime().unwrap();

        // if this process is defunct/zombie, don't try to read any of the below data
        // (some might be successful, but not all)
        if stat.state().unwrap() == ProcState::Zombie {
            continue;
        }

        check_unwrap(&prc, prc.cmdline());
        check_unwrap(&prc, prc.environ());
        check_unwrap(&prc, prc.fd());
        check_unwrap(&prc, prc.io());
        check_unwrap(&prc, prc.maps());
        check_unwrap(&prc, prc.coredump_filter());
        // The WSL2 kernel doesn't have autogroup, even though this should be present since linux
        // 2.6.36
        if is_wsl2 {
            assert!(prc.autogroup().is_err());
        } else {
            check_unwrap(&prc, prc.autogroup());
        }
        check_unwrap(&prc, prc.auxv());
        check_unwrap(&prc, prc.cgroups());
        check_unwrap(&prc, prc.wchan());
        check_unwrap(&prc, prc.status());
        check_unwrap(&prc, prc.mountinfo());
        check_unwrap(&prc, prc.mountstats());
        check_unwrap(&prc, prc.oom_score());

        if let Some(tasks) = check_unwrap(&prc, prc.tasks()) {
            for task in tasks {
                let task = task.unwrap();
                check_unwrap_task(&prc, task.stat());
                check_unwrap_task(&prc, task.status());
                check_unwrap_task(&prc, task.io());
                check_unwrap_task(&prc, task.schedstat());
            }
        }
    }
}

#[test]
fn test_smaps() {
    let me = Process::myself().unwrap();
    let smaps = match me.smaps() {
        Ok(x) => x,
        Err(ProcError::NotFound(_)) => {
            // ignored because not all kernerls have smaps
            return;
        }
        Err(e) => panic!("{}", e),
    };
    println!("{:#?}", smaps);
}

#[test]
fn test_smaps_rollup() {
    let me = Process::myself().unwrap();
    let smaps_rollup = match me.smaps_rollup() {
        Ok(x) => x,
        Err(ProcError::NotFound(_)) => {
            // ignored because not all kernerls have smaps_rollup
            return;
        }
        Err(e) => panic!("{}", e),
    };
    println!("{:#?}", smaps_rollup);
}

#[test]
fn test_proc_alive() {
    let myself = Process::myself().unwrap();
    assert!(myself.is_alive());

    // zombies should not be considered alive
    let mut command = std::process::Command::new("sleep");
    command
        .arg("0")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let mut child = command.spawn().unwrap();
    let child_pid = child.id() as i32;

    // sleep very briefly to allow the child to start and then exit
    std::thread::sleep(std::time::Duration::from_millis(30));

    let child_proc = Process::new(child_pid).unwrap();
    assert!(!child_proc.is_alive(), "Child state is: {:?}", child_proc.stat());
    assert!(child_proc.stat().unwrap().state().unwrap() == ProcState::Zombie);
    child.wait().unwrap();
    assert!(Process::new(child_pid).is_err());
    assert!(!child_proc.is_alive(), "Child state is: {:?}", child_proc.stat());
}

#[test]
fn test_proc_environ() {
    let myself = Process::myself().unwrap();
    let proc_environ = myself.environ().unwrap();

    let std_environ: HashMap<_, _> = std::env::vars_os().collect();
    assert_eq!(proc_environ, std_environ);
}

#[test]
fn test_error_handling() {
    // getting the proc struct should be OK
    let init = Process::new(1).unwrap();

    let i_have_access = rustix::process::geteuid().as_raw() == init.uid().unwrap();

    if !i_have_access {
        // but accessing data should result in an error (unless we are running as root!)
        assert!(init.cwd().is_err());
        assert!(init.environ().is_err());
    }
}

#[test]
fn test_proc_exe() {
    let myself = Process::myself().unwrap();
    let proc_exe = myself.exe().unwrap();
    let std_exe = std::env::current_exe().unwrap();
    assert_eq!(proc_exe, std_exe);
}

#[test]
fn test_proc_io() {
    let myself = Process::myself().unwrap();
    let kernel = KernelVersion::current().unwrap();
    let io = myself.io();
    println!("{:?}", io);
    if io.is_ok() {
        assert!(kernel >= KernelVersion::new(2, 6, 20));
    }
}

#[test]
fn test_proc_maps() {
    let myself = Process::myself().unwrap();
    let maps = myself.maps().unwrap();
    for map in maps {
        println!("{:?}", map);
    }
}

#[test]
fn test_proc_pagemap() {
    let myself = Process::myself().unwrap();
    let maps = myself.maps().unwrap();

    let stack_map = maps.iter().find(|m| matches!(m.pathname, MMapPath::Stack)).unwrap();
    let page_size = crate::page_size().unwrap() as usize;
    let start_page = stack_map.address.0 as usize / page_size;
    let end_page = stack_map.address.1 as usize / page_size;

    let mut pagemap = myself.pagemap().unwrap();
    let page_infos = pagemap.get_range_info(start_page..end_page).unwrap();

    let present_pages = page_infos.iter().filter(|info| {
        if let PageInfo::MemoryPage(flags) = info {
            flags.contains(MemoryPageFlags::PRESENT)
        } else {
            false
        }
    });

    for present_page in present_pages {
        println!("{:?}", present_page);
    }
}

#[test]
fn test_mmap_path() {
    assert_eq!(MMapPath::from("[stack]").unwrap(), MMapPath::Stack);
    assert_eq!(MMapPath::from("[foo]").unwrap(), MMapPath::Other("foo".to_owned()));
    assert_eq!(MMapPath::from("").unwrap(), MMapPath::Anonymous);
    assert_eq!(MMapPath::from("[stack:154]").unwrap(), MMapPath::TStack(154));
    assert_eq!(
        MMapPath::from("/lib/libfoo.so").unwrap(),
        MMapPath::Path(PathBuf::from("/lib/libfoo.so"))
    );
}
#[test]
fn test_proc_fds() {
    let myself = Process::myself().unwrap();
    for f in myself.fd().unwrap() {
        let fd = f.unwrap();
        println!("{:?} {:?}", fd, fd.mode());
    }
}

#[test]
fn test_proc_fd() {
    let myself = Process::myself().unwrap();
    let raw_fd = myself.fd().unwrap().next().unwrap().unwrap().fd as i32;
    let fd = FDInfo::from_raw_fd(myself.pid, raw_fd).unwrap();
    println!("{:?} {:?}", fd, fd.mode());
}

#[test]
fn test_proc_coredump() {
    let myself = Process::myself().unwrap();
    let flags = myself.coredump_filter();
    println!("{:?}", flags);
}

#[test]
fn test_proc_auxv() {
    let myself = Process::myself().unwrap();
    let auxv = myself.auxv().unwrap();
    println!("{:?}", auxv);
    for (k, v) in auxv {
        // See bits/auxv.h
        match k {
            2 => println!("File descriptor of program: {}", v),
            3 => println!("Address of the program headers of the executable: 0x{:x}", v),
            4 => println!("Size of program header entry: {}", v),
            5 => println!("Number of program headers: {}", v),
            6 => {
                println!("System page size: {}", v);
                assert!(v > 0);
            }
            7 => {
                println!("Base address: 0x{:x}", v);
                assert!(v > 0);
            }
            8 => println!("Flags: 0x{:x}", v),
            9 => {
                println!("Entry address of the executable: 0x{:x}", v);
                assert!(v > 0);
            }
            11 => {
                println!("Real UID: {}", v);
                assert_eq!(v as u32, rustix::process::getuid().as_raw());
            }
            12 => {
                println!("Effective UID: {}", v);
                assert!(v > 0);
            }
            13 => {
                println!("Real GID: {}", v);
                assert!(v > 0);
            }
            14 => {
                println!("Effective GID: {}", v);
                assert!(v > 0);
            }
            15 => {
                println!("Platform string address: 0x{:x}", v);
                let platform = unsafe { std::ffi::CStr::from_ptr(v as *const _) };
                println!("Platform string: {:?}", platform);
            }
            16 => println!("HW Cap: 0x{:x}", v),
            17 => {
                println!("Clock ticks per second: {}", v);
                assert_eq!(v, crate::ticks_per_second().unwrap());
            }
            19 => println!("Data cache block size: {}", v),
            23 => println!("Run as setuid?: {}", v),
            25 => println!("Address of 16 random bytes: 0x{:x}", v),
            26 => println!("HW Cap2: 0x{:x}", v),
            31 => {
                println!("argv[0] address: 0x{:x}", v);
                let argv0 = unsafe { std::ffi::CStr::from_ptr(v as *const _) };
                println!("argv[0]: {:?}", argv0);
            }
            k => println!("Unknown key {}: {:x}", k, v),
        }
    }
}

#[test]
fn test_proc_wchan() {
    let myself = Process::myself().unwrap();
    let wchan = myself.wchan().unwrap();
    println!("{:?}", wchan);
}

#[test]
fn test_proc_loginuid() {
    if !Path::new("/proc/self/loginuid").exists() {
        return;
    }

    let myself = Process::myself().unwrap();
    let loginuid = myself.loginuid().unwrap();
    println!("{:?}", loginuid);
}

#[test]
fn test_nopanic() {
    fn inner() -> ProcResult<u8> {
        let a = vec!["xyz"];
        from_iter(a)
    }
    assert!(inner().is_err());
}

#[test]
fn test_procinfo() {
    // test to see that this crate and procinfo give mostly the same results

    fn diff_mem(a: f32, b: f32) {
        let diff = (a - b).abs();
        assert!(diff < 20000.0, "diff:{}", diff);
    }

    // take a pause to let things "settle" before getting data.  By default, cargo will run
    // tests in parallel, which can cause disturbences
    std::thread::sleep(std::time::Duration::from_secs(1));

    let procinfo_stat = procinfo::pid::stat_self().unwrap();
    let me = Process::myself().unwrap();
    let me_stat = me.stat().unwrap();

    diff_mem(procinfo_stat.vsize as f32, me_stat.vsize as f32);

    assert_eq!(me_stat.priority, procinfo_stat.priority as i64);
    assert_eq!(me_stat.nice, procinfo_stat.nice as i64);
    // flags seem to change during runtime, with PF_FREEZER_SKIP coming and going...
    //assert_eq!(me_stat.flags, procinfo_stat.flags, "procfs:{:?} procinfo:{:?}", crate::StatFlags::from_bits(me_stat.flags), crate::StatFlags::from_bits(procinfo_stat.flags));
    assert_eq!(me_stat.pid, procinfo_stat.pid);
    assert_eq!(me_stat.ppid, procinfo_stat.ppid);
}

#[test]
fn test_statm() {
    let me = Process::myself().unwrap();
    let statm = me.statm().unwrap();
    println!("{:#?}", statm);
}

#[test]
fn test_schedstat() {
    let me = Process::myself().unwrap();
    let schedstat = me.schedstat().unwrap();
    println!("{:#?}", schedstat);
}

#[test]
fn test_fdtarget() {
    // none of these values are valid, but were found by a fuzzer to crash procfs.  this
    // test ensures that the crashes have been fixed

    let _ = FDTarget::from_str(":");
    let _ = FDTarget::from_str("n:ǟF");
    let _ = FDTarget::from_str("pipe:");
}

#[test]
fn test_fdtarget_memfd() {
    let memfd = FDTarget::from_str("/memfd:test").unwrap();
    assert!(matches!(memfd, FDTarget::MemFD(s) if s == "test"));
}
