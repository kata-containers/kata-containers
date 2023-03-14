/// Test that `Timespec` and `Secs` support a 64-bit number of seconds,
/// avoiding the y2038 bug.
#[cfg(not(libc))] // The libc crate does not support a 64-bit time_t.
#[test]
fn test_y2038() {
    use rustix::time::{Secs, Timespec};

    let tv_sec: i64 = 0;
    let _ = Timespec { tv_sec, tv_nsec: 0 };
    let _: Secs = tv_sec;
}
