#![cfg(not(windows))]

#[cfg(feature = "tokio-support")]
mod tests {
    extern crate libc;
    extern crate signal_hook;
    extern crate tokio;

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use self::signal_hook::iterator::Signals;
    use self::tokio::prelude::*;
    use self::tokio::timer::Interval;

    fn send_sig(sig: libc::c_int) {
        unsafe { libc::raise(sig) };
    }

    #[test]
    fn repeated() {
        let signals = Signals::new(&[signal_hook::SIGUSR1])
            .unwrap()
            .into_async()
            .unwrap()
            .take(20)
            .map_err(|e| panic!("{}", e))
            .for_each(|sig| {
                assert_eq!(sig, signal_hook::SIGUSR1);
                send_sig(signal_hook::SIGUSR1);
                Ok(())
            });
        send_sig(signal_hook::SIGUSR1);
        tokio::run(signals);
    }

    /// A test where we actually wait for something ‒ the stream/reactor goes to sleep.
    #[test]
    fn delayed() {
        const CNT: usize = 10;
        let cnt = Arc::new(AtomicUsize::new(0));
        let inc_cnt = Arc::clone(&cnt);
        let signals = Signals::new(&[signal_hook::SIGUSR1, signal_hook::SIGUSR2])
            .unwrap()
            .into_async()
            .unwrap()
            .filter(|sig| *sig == signal_hook::SIGUSR2)
            .take(CNT as u64)
            .map_err(|e| panic!("{}", e))
            .for_each(move |_| {
                inc_cnt.fetch_add(1, Ordering::Relaxed);
                Ok(())
            });
        let senders = Interval::new(Instant::now(), Duration::from_millis(250))
            .map_err(|e| panic!("{}", e))
            .for_each(|_| {
                send_sig(signal_hook::SIGUSR2);
                Ok(())
            });
        let both = signals.select(senders).map(|_| ()).map_err(|_| ());
        tokio::run(both);
        // Just make sure it didn't terminate prematurely
        assert_eq!(CNT, cnt.load(Ordering::Relaxed));
    }
}
