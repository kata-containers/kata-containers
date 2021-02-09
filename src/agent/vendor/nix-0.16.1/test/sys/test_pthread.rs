use nix::sys::pthread::*;

#[cfg(target_env = "musl")]
#[test]
fn test_pthread_self() {
    let tid = pthread_self();
    assert!(tid != ::std::ptr::null_mut());
}

#[cfg(not(target_env = "musl"))]
#[test]
fn test_pthread_self() {
    let tid = pthread_self();
    assert!(tid != 0);
}
