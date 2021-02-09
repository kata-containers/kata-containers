use caps::runtime;

#[test]
fn test_ambient_supported() {
    runtime::ambient_set_supported().unwrap();
}

#[test]
fn test_thread_all_supported() {
    assert!(runtime::thread_all_supported().len() > 0);
    assert!(runtime::thread_all_supported().len() <= caps::all().len());
}

#[test]
fn test_procfs_all_supported() {
    use std::path::PathBuf;

    let p1 = runtime::procfs_all_supported(None).unwrap();
    let p2 = runtime::procfs_all_supported(Some(PathBuf::from("/proc"))).unwrap();
    let thread = runtime::thread_all_supported();
    let all = caps::all();

    assert!(thread.len() > 0);
    assert!(thread.len() <= all.len());
    assert_eq!(
        p1,
        p2,
        "{:?}",
        p1.symmetric_difference(&p2).collect::<Vec<_>>()
    );
    assert_eq!(
        p1,
        thread,
        "{:?}",
        p1.symmetric_difference(&thread).collect::<Vec<_>>()
    );
}
