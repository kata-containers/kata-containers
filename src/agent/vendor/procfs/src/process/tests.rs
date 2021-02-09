use super::*;

fn check_unwrap<T>(prc: &Process, val: ProcResult<T>) {
    match val {
        Ok(_t) => {}
        Err(ProcError::PermissionDenied(_)) if unsafe { libc::geteuid() } != 0 => {
            // we are not root, and so a permission denied error is OK
        }
        Err(ProcError::NotFound(path)) => {
            // a common reason for this error is that the process isn't running anymore
            if prc.is_alive() {
                panic!("{:?} not found", path)
            }
        }
        Err(err) => panic!("{:?}", err),
    }
}

#[allow(clippy::cognitive_complexity)]
#[test]
fn test_self_proc() {
    let myself = Process::myself().unwrap();
    println!("{:#?}", myself);
    println!("state: {:?}", myself.stat.state());
    println!("tty: {:?}", myself.stat.tty_nr());
    println!("flags: {:?}", myself.stat.flags());

    #[cfg(feature = "chrono")]
    println!("starttime: {:#?}", myself.stat.starttime());

    let kernel = KernelVersion::current().unwrap();

    if kernel >= KernelVersion::new(2, 1, 22) {
        assert!(myself.stat.exit_signal.is_some());
    } else {
        assert!(myself.stat.exit_signal.is_none());
    }

    if kernel >= KernelVersion::new(2, 2, 8) {
        assert!(myself.stat.processor.is_some());
    } else {
        assert!(myself.stat.processor.is_none());
    }

    if kernel >= KernelVersion::new(2, 5, 19) {
        assert!(myself.stat.rt_priority.is_some());
    } else {
        assert!(myself.stat.rt_priority.is_none());
    }

    if kernel >= KernelVersion::new(2, 5, 19) {
        assert!(myself.stat.rt_priority.is_some());
        assert!(myself.stat.policy.is_some());
    } else {
        assert!(myself.stat.rt_priority.is_none());
        assert!(myself.stat.policy.is_none());
    }

    if kernel >= KernelVersion::new(2, 6, 18) {
        assert!(myself.stat.delayacct_blkio_ticks.is_some());
    } else {
        assert!(myself.stat.delayacct_blkio_ticks.is_none());
    }

    if kernel >= KernelVersion::new(2, 6, 24) {
        assert!(myself.stat.guest_time.is_some());
        assert!(myself.stat.cguest_time.is_some());
    } else {
        assert!(myself.stat.guest_time.is_none());
        assert!(myself.stat.cguest_time.is_none());
    }

    if kernel >= KernelVersion::new(3, 3, 0) {
        assert!(myself.stat.start_data.is_some());
        assert!(myself.stat.end_data.is_some());
        assert!(myself.stat.start_brk.is_some());
    } else {
        assert!(myself.stat.start_data.is_none());
        assert!(myself.stat.end_data.is_none());
        assert!(myself.stat.start_brk.is_none());
    }

    if kernel >= KernelVersion::new(3, 5, 0) {
        assert!(myself.stat.arg_start.is_some());
        assert!(myself.stat.arg_end.is_some());
        assert!(myself.stat.env_start.is_some());
        assert!(myself.stat.env_end.is_some());
        assert!(myself.stat.exit_code.is_some());
    } else {
        assert!(myself.stat.arg_start.is_none());
        assert!(myself.stat.arg_end.is_none());
        assert!(myself.stat.env_start.is_none());
        assert!(myself.stat.env_end.is_none());
        assert!(myself.stat.exit_code.is_none());
    }
}

#[test]
fn test_all() {
    for prc in all_processes().unwrap() {
        // note: this test doesn't unwrap, since some of this data requires root to access
        // so permission denied errors are common.  The check_unwrap helper function handles
        // this.

        println!("{} {}", prc.pid(), prc.stat.comm);
        prc.stat.flags().unwrap();
        prc.stat.state().unwrap();
        #[cfg(feature = "chrono")]
        prc.stat.starttime().unwrap();

        // if this process is defunct/zombie, don't try to read any of the below data
        // (some might be successful, but not all)
        if prc.stat.state().unwrap() == ProcState::Zombie {
            continue;
        }

        check_unwrap(&prc, prc.cmdline());
        check_unwrap(&prc, prc.environ());
        check_unwrap(&prc, prc.fd());
        check_unwrap(&prc, prc.io());
        check_unwrap(&prc, prc.maps());
        check_unwrap(&prc, prc.coredump_filter());
        check_unwrap(&prc, prc.autogroup());
        check_unwrap(&prc, prc.auxv());
        check_unwrap(&prc, prc.cgroups());
        check_unwrap(&prc, prc.wchan());
        check_unwrap(&prc, prc.status());
        check_unwrap(&prc, prc.mountinfo());
        check_unwrap(&prc, prc.mountstats());
        check_unwrap(&prc, prc.oom_score());
    }
}

#[test]
fn test_proc_alive() {
    let myself = Process::myself().unwrap();
    assert!(myself.is_alive());
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

    let i_am_root = unsafe { libc::geteuid() } == 0;

    if !i_am_root {
        // but accessing data should result in an error (unless we are running as root!)
        assert!(!init.cwd().is_ok());
        assert!(!init.environ().is_ok());
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
fn test_mmap_path() {
    assert_eq!(MMapPath::from("[stack]").unwrap(), MMapPath::Stack);
    assert_eq!(
        MMapPath::from("[foo]").unwrap(),
        MMapPath::Other("foo".to_owned())
    );
    assert_eq!(MMapPath::from("").unwrap(), MMapPath::Anonymous);
    assert_eq!(
        MMapPath::from("[stack:154]").unwrap(),
        MMapPath::TStack(154)
    );
    assert_eq!(
        MMapPath::from("/lib/libfoo.so").unwrap(),
        MMapPath::Path(PathBuf::from("/lib/libfoo.so"))
    );
}
#[test]
fn test_proc_fd() {
    let myself = Process::myself().unwrap();
    for fd in myself.fd().unwrap() {
        println!("{:?} {:?}", fd, fd.mode());
    }
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
    let me_stat = me.stat;

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
