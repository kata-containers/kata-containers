use rustix::time::{clock_gettime, ClockId};

#[test]
fn test_monotonic_clock() {
    let a = clock_gettime(ClockId::Monotonic);
    let b = clock_gettime(ClockId::Monotonic);
    if b.tv_sec == a.tv_sec {
        assert!(b.tv_nsec >= a.tv_nsec);
    } else {
        assert!(b.tv_sec > a.tv_sec);
    }
}
