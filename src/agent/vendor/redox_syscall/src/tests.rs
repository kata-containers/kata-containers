#[test]
fn brk() {
    unsafe {
        let start = dbg!(crate::brk(0)).unwrap();
        let end = start + 4 * 1024 * 1024;
        assert_eq!(dbg!(crate::brk(end)), Ok(end));
    }
}

#[test]
fn chdir() {
    //TODO: Verify CWD
    assert_eq!(dbg!(crate::chdir("file:/")), Ok(0));
    assert_eq!(dbg!(crate::chdir("file:/root")), Ok(0));
}

//TODO: chmod

#[test]
fn clone() {
    let expected_status = 42;
    let pid_res = unsafe { crate::clone(0) };
    if pid_res == Ok(0) {
        crate::exit(expected_status).unwrap();
        panic!("failed to exit");
    } else {
        let pid = dbg!(pid_res).unwrap();
        let mut status = 0;
        assert_eq!(dbg!(crate::waitpid(pid, &mut status, 0)), Ok(pid));
        assert_eq!(dbg!(crate::wifexited(status)), true);
        assert_eq!(dbg!(crate::wexitstatus(status)), expected_status);
    }
}

//TODO: close

#[test]
fn clock_gettime() {
    let mut tp = crate::TimeSpec::default();
    assert_eq!(dbg!(
        crate::clock_gettime(crate::CLOCK_MONOTONIC, &mut tp)
    ), Ok(0));
    assert_ne!(dbg!(tp), crate::TimeSpec::default());

    tp = crate::TimeSpec::default();
    assert_eq!(dbg!(
        crate::clock_gettime(crate::CLOCK_REALTIME, &mut tp)
    ), Ok(0));
    assert_ne!(dbg!(tp), crate::TimeSpec::default());
}

//TODO: dup

//TODO: dup2

//TODO: exit (handled by clone?)

//TODO: fchmod

//TODO: fcntl

#[test]
fn fexec() {
    let name = "/bin/ls";

    let fd = dbg!(
        crate::open(name, crate::O_RDONLY | crate::O_CLOEXEC)
    ).unwrap();

    let args = &[
        [name.as_ptr() as usize, name.len()]
    ];

    let vars = &[];

    let pid_res = unsafe { crate::clone(0) };
    if pid_res == Ok(0) {
        crate::fexec(fd, args, vars).unwrap();
        panic!("failed to fexec");
    } else {
        assert_eq!(dbg!(crate::close(fd)), Ok(0));

        let pid = dbg!(pid_res).unwrap();
        let mut status = 0;
        assert_eq!(dbg!(crate::waitpid(pid, &mut status, 0)), Ok(pid));
        assert_eq!(dbg!(crate::wifexited(status)), true);
        assert_eq!(dbg!(crate::wexitstatus(status)), 0);
    }
}

#[test]
fn fmap() {
    use std::slice;

    let fd = dbg!(
        crate::open(
            "/tmp/syscall-tests-fmap",
            crate::O_CREAT | crate::O_RDWR | crate::O_CLOEXEC
        )
    ).unwrap();

    let map = unsafe {
        slice::from_raw_parts_mut(
            dbg!(
                crate::fmap(fd, &crate::Map {
                    offset: 0,
                    size: 128,
                    flags: crate::PROT_READ | crate::PROT_WRITE
                })
            ).unwrap() as *mut u8,
            128
        )
    };

    // Maps should be available after closing
    assert_eq!(dbg!(crate::close(fd)), Ok(0));

    for i in 0..128 {
        map[i as usize] = i;
        assert_eq!(map[i as usize], i);
    }

    //TODO: add msync
    unsafe {
        assert_eq!(dbg!(
            crate::funmap(map.as_mut_ptr() as usize)
        ), Ok(0));
    }
}
