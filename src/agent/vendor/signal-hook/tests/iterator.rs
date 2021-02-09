#![cfg(not(windows))]

extern crate signal_hook;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use signal_hook::iterator::Signals;
use signal_hook::{SIGUSR1, SIGUSR2};

#[test]
fn signals_close_forever() {
    // The cloned instances are connected to each other. Closing one closes all.
    // Closing it terminates the forever that waits for stuff. Well, it terminates all of them.
    let signals = Signals::new(&[SIGUSR1]).unwrap();
    // Detect early terminations.
    let stopped = Arc::new(AtomicBool::new(false));
    let threads = (0..5).map(|_| {
        let signals_bg = signals.clone();
        let stopped_bg = Arc::clone(&stopped);
        thread::spawn(move || {
            // Eat all the signals there are (might come from a concurrent test, in theory).
            // Would wait forever, but it should be terminated by the close below.
            for _sig in &signals_bg {}

            stopped_bg.store(true, Ordering::SeqCst);
        })
    });

    // Wait a bit… if some thread terminates by itself.
    thread::sleep(Duration::from_millis(100));
    assert!(!stopped.load(Ordering::SeqCst));

    signals.close();

    // If they don't terminate correctly, the test just keeps running. Not the best way to do
    // tests, but whatever…
    for thread in threads {
        thread.join().unwrap();
    }
}

// A reproducer for #16: if we had the mio-support enabled (which is enabled also by the
// tokio-support feature), blocking no longer works. The .wait() would return immediately (an empty
// iterator, possibly), .forever() would do a busy loop.
// flag)
#[test]
fn signals_block_wait() {
    let signals = Signals::new(&[SIGUSR2]).unwrap();
    let (s, r) = mpsc::channel();
    thread::spawn(move || {
        // Technically, it may spuriously return early. But it shouldn't be doing it too much, so
        // we just try to wait multiple times ‒ if they *all* return right away, it is broken.
        for _ in 0..10 {
            for _ in signals.wait() {
                panic!("Someone really did send us SIGUSR2, which breaks the test");
            }
        }
        let _ = s.send(());
    });

    let err = r
        .recv_timeout(Duration::from_millis(100))
        .expect_err("Wait didn't wait properly");
    assert_eq!(err, RecvTimeoutError::Timeout);
}
