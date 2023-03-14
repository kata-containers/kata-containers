use nix::sys::pthread::*;

#[cfg(any(target_env = "musl", target_os = "redox"))]
#[test]
fn test_pthread_self() {
    let tid = pthread_self();
    assert!(tid != ::std::ptr::null_mut());
}

#[cfg(not(any(target_env = "musl", target_os = "redox")))]
#[test]
fn test_pthread_self() {
    let tid = pthread_self();
    assert!(tid != 0);
}
