//! Prints the elapsed time every 1 second and quits on Ctrl+C.

#[macro_use]
extern crate crossbeam_channel;
extern crate signal_hook;

use std::io;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, tick, Receiver};
use signal_hook::iterator::Signals;
use signal_hook::SIGINT;

// Creates a channel that gets a message every time `SIGINT` is signalled.
fn sigint_notifier() -> io::Result<Receiver<()>> {
    let (s, r) = bounded(100);
    let signals = Signals::new(&[SIGINT])?;

    thread::spawn(move || {
        for _ in signals.forever() {
            if s.send(()).is_err() {
                break;
            }
        }
    });

    Ok(r)
}

// Prints the elapsed time.
fn show(dur: Duration) {
    println!(
        "Elapsed: {}.{:03} sec",
        dur.as_secs(),
        dur.subsec_nanos() / 1_000_000
    );
}

fn main() {
    let start = Instant::now();
    let update = tick(Duration::from_secs(1));
    let ctrl_c = sigint_notifier().unwrap();

    loop {
        select! {
            recv(update) -> _ => {
                show(start.elapsed());
            }
            recv(ctrl_c) -> _ => {
                println!();
                println!("Goodbye!");
                show(start.elapsed());
                break;
            }
        }
    }
}
