//! One indirect test for cleanup.
//!
//! Unlike the ones in cleanup.rs, this one is usable on windows too. But because the library can't
//! recover from cleanup, we have just one here.

extern crate libc;
extern crate signal_hook;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use libc::{c_int, raise, sighandler_t, signal};
use signal_hook::SIGTERM;

extern "C" fn handler(_: c_int) {}

#[test]
fn cleanup_indirect() {
    // Read what the default is.
    let orig = unsafe { signal(SIGTERM, handler as sighandler_t) };
    signal_hook::flag::register(SIGTERM, Arc::new(AtomicBool::new(false))).unwrap();
    signal_hook::cleanup::register(SIGTERM, vec![SIGTERM]).unwrap();
    // By now, it is switched to something else.
    unsafe {
        // This'll change it back to the default due to the cleanup.
        raise(SIGTERM);
        // Check it really did.
        assert_eq!(orig, signal(SIGTERM, handler as sighandler_t));
    }
}
