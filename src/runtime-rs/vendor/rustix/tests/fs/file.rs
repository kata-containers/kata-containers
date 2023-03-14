#[cfg(not(target_os = "redox"))]
#[test]
fn test_file() {
    rustix::fs::accessat(
        &rustix::fs::cwd(),
        "Cargo.toml",
        rustix::fs::Access::READ_OK,
        rustix::fs::AtFlags::empty(),
    )
    .unwrap();

    assert_eq!(
        rustix::fs::openat(
            &rustix::fs::cwd(),
            "Cagro.motl",
            rustix::fs::OFlags::RDONLY,
            rustix::fs::Mode::empty(),
        )
        .unwrap_err(),
        rustix::io::Error::NOENT
    );

    let file = rustix::fs::openat(
        &rustix::fs::cwd(),
        "Cargo.toml",
        rustix::fs::OFlags::RDONLY,
        rustix::fs::Mode::empty(),
    )
    .unwrap();

    assert_eq!(
        rustix::fs::openat(
            &file,
            "Cargo.toml",
            rustix::fs::OFlags::RDONLY,
            rustix::fs::Mode::empty(),
        )
        .unwrap_err(),
        rustix::io::Error::NOTDIR
    );

    #[cfg(not(any(
        target_os = "ios",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd"
    )))]
    rustix::fs::fadvise(&file, 0, 10, rustix::fs::Advice::Normal).unwrap();

    assert_eq!(
        rustix::fs::fcntl_getfd(&file).unwrap(),
        rustix::fs::FdFlags::empty()
    );
    assert_eq!(
        rustix::fs::fcntl_getfl(&file).unwrap(),
        rustix::fs::OFlags::empty()
    );

    let stat = rustix::fs::fstat(&file).unwrap();
    assert!(stat.st_size > 0);
    assert!(stat.st_blocks > 0);

    #[cfg(not(any(target_os = "netbsd", target_os = "wasi")))]
    // not implemented in libc for netbsd yet
    {
        let statfs = rustix::fs::fstatfs(&file).unwrap();
        assert!(statfs.f_blocks > 0);
    }

    assert_eq!(rustix::io::is_read_write(&file).unwrap(), (true, false));

    assert_ne!(rustix::io::ioctl_fionread(&file).unwrap(), 0);
}
