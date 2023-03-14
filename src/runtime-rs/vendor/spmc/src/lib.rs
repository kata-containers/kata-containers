#![deny(warnings)]
#![deny(missing_docs)]
//! # SPMC
//!
//! A single producer, multiple consumers. Commonly used to implement
//! work-stealing.
//!
//! ## Example
//!
//! ```
//! # use std::thread;
//! let (mut tx, rx) = spmc::channel();
//!
//! let mut handles = Vec::new();
//! for n in 0..5 {
//!     let rx = rx.clone();
//!     handles.push(thread::spawn(move || {
//!         let msg = rx.recv().unwrap();
//!         println!("worker {} recvd: {}", n, msg);
//!     }));
//! }
//!
//! for i in 0..5 {
//!     tx.send(i * 2).unwrap();
//! }
//!
//! for handle in handles {
//!   handle.join().unwrap();
//! }
//! ```

mod channel;
mod loom;

pub use self::channel::{channel, Sender, Receiver};
pub use std::sync::mpsc::{SendError, RecvError, TryRecvError};


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanity() {
        let (mut tx, rx) = channel();
        tx.send(5).unwrap();
        tx.send(12).unwrap();
        tx.send(1).unwrap();

        assert_eq!(rx.try_recv(), Ok(5));
        assert_eq!(rx.try_recv(), Ok(12));
        assert_eq!(rx.try_recv(), Ok(1));
        assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
    }

    #[test]
    fn test_multiple_consumers() {
        let (mut tx, rx) = channel();
        let rx2 = rx.clone();
        tx.send(5).unwrap();
        tx.send(12).unwrap();
        tx.send(1).unwrap();

        assert_eq!(rx.try_recv(), Ok(5));
        assert_eq!(rx2.try_recv(), Ok(12));
        assert_eq!(rx2.try_recv(), Ok(1));
        assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
        assert_eq!(rx2.try_recv(), Err(TryRecvError::Empty));
    }

    #[test]
    fn test_send_on_dropped_chan() {
        let (mut tx, rx) = channel();
        drop(rx);
        assert_eq!(tx.send(5), Err(SendError(5)));
    }

    #[test]
    fn test_try_recv_on_dropped_chan() {
        let (mut tx, rx) = channel();
        tx.send(2).unwrap();
        drop(tx);

        assert_eq!(rx.try_recv(), Ok(2));
        assert_eq!(rx.try_recv(), Err(TryRecvError::Disconnected));
        assert_eq!(rx.recv(), Err(RecvError));
    }

    #[test]
    fn test_recv_blocks() {
        use std::thread;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        let (mut tx, rx) = channel();
        let toggle = Arc::new(AtomicBool::new(false));
        let toggle_clone = toggle.clone();
        thread::spawn(move || {
            toggle_clone.store(true, Ordering::Relaxed);
            tx.send(11).unwrap();
        });

        assert_eq!(rx.recv(), Ok(11));
        assert!(toggle.load(Ordering::Relaxed))
    }

    #[test]
    fn test_recv_unblocks_on_dropped_chan() {
        use std::thread;

        let (tx, rx) = channel::<i32>();
        thread::spawn(move || {
            let _tx = tx;
        });

        assert_eq!(rx.recv(), Err(RecvError));
    }

    #[test]
    fn test_send_sleep() {
        use std::thread;
        use std::time::Duration;

        let (mut tx, rx) = channel();

        let mut handles = Vec::new();
        for _ in 0..5 {
            let rx = rx.clone();
            handles.push(thread::spawn(move || {
                rx.recv().unwrap();
            }));
        }

        for i in 0..5 {
            tx.send(i * 2).unwrap();
            thread::sleep(Duration::from_millis(100));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_tx_dropped_rxs_drain() {
        for l in 0..10 {
            println!("loop {}", l);

            let (mut tx, rx) = channel();

            let mut handles = Vec::new();
            for _ in 0..5 {
                let rx = rx.clone();
                handles.push(::std::thread::spawn(move || {
                    loop {
                        match rx.recv() {
                            Ok(_) => continue,
                            Err(_) => break,
                        }
                    }
                }));
            }

            for i in 0..10 {
                tx.send(format!("Sending value {} {}", l, i)).unwrap();
            }
            drop(tx);

            for handle in handles {
                handle.join().unwrap();
            }
        }
    }

    #[test]
    fn msg_dropped() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        struct Dropped(Arc<AtomicBool>);

        impl Drop for Dropped {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Relaxed);
            }
        }

        let sentinel = Arc::new(AtomicBool::new(false));
        assert!(!sentinel.load(Ordering::Relaxed));


        let (mut tx, rx) = channel();

        tx.send(Dropped(sentinel.clone())).unwrap();
        assert!(!sentinel.load(Ordering::Relaxed));

        rx.recv().unwrap();
        assert!(sentinel.load(Ordering::Relaxed));
    }


    #[test]
    fn msgs_dropped() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct Dropped(Arc<AtomicUsize>);

        impl Drop for Dropped {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }

        let sentinel = Arc::new(AtomicUsize::new(0));
        assert_eq!(0, sentinel.load(Ordering::Relaxed));


        let (mut tx, rx) = channel();

        tx.send(Dropped(sentinel.clone())).unwrap();
        tx.send(Dropped(sentinel.clone())).unwrap();
        tx.send(Dropped(sentinel.clone())).unwrap();
        tx.send(Dropped(sentinel.clone())).unwrap();
        assert_eq!(0, sentinel.load(Ordering::Relaxed));

        rx.recv().unwrap();
        assert_eq!(1, sentinel.load(Ordering::Relaxed));
        rx.recv().unwrap();
        rx.recv().unwrap();
        rx.recv().unwrap();
        assert_eq!(4, sentinel.load(Ordering::Relaxed));
    }
}
