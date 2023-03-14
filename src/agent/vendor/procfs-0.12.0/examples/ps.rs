#![allow(clippy::print_literal)]

extern crate procfs;

/// A very basic clone of `ps` on Linux, in the simple no-argument mode.
/// It shows all the processes that share the same tty as our self

fn main() {
    let me = procfs::process::Process::myself().unwrap();
    let tps = procfs::ticks_per_second().unwrap();

    println!("{: >5} {: <8} {: >8} {}", "PID", "TTY", "TIME", "CMD");

    let tty = format!("pty/{}", me.stat.tty_nr().1);
    for prc in procfs::process::all_processes().unwrap() {
        if prc.stat.tty_nr == me.stat.tty_nr {
            // total_time is in seconds
            let total_time = (prc.stat.utime + prc.stat.stime) as f32 / (tps as f32);
            println!("{: >5} {: <8} {: >8} {}", prc.stat.pid, tty, total_time, prc.stat.comm);
        }
    }
}
