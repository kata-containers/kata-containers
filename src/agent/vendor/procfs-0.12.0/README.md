procfs
======

[![Crate](https://img.shields.io/crates/v/procfs.svg)](https://crates.io/crates/procfs)
[![Docs](https://docs.rs/procfs/badge.svg)](https://docs.rs/procfs)
[![Minimum rustc version](https://img.shields.io/badge/rustc-1.34+-lightgray.svg)](https://github.com/eminence/procfs#minimum-rust-version)


This crate is an interface to the `proc` pseudo-filesystem on linux, which is normally mounted as `/proc`.
Long-term, this crate aims to be fairly feature complete, but at the moment not all files are exposed.
See the docs for info on what's supported, or view the [support.md](https://github.com/eminence/procfs/blob/master/support.md)
file in the code repository.

## Examples
There are several examples in the docs and in the [examples folder](https://github.com/eminence/procfs/tree/master/examples)
of the code repository.

Here's a small example that prints out all processes that are running on the same tty as the calling
process.  This is very similar to what "ps" does in its default mode:

```rust
fn main() {
    let me = procfs::process::Process::myself().unwrap();
    let tps = procfs::ticks_per_second().unwrap();

    println!("{: >5} {: <8} {: >8} {}", "PID", "TTY", "TIME", "CMD");

    let tty = format!("pty/{}", me.stat.tty_nr().1);
    for prc in procfs::process::all_processes().unwrap() {
        if prc.stat.tty_nr == me.stat.tty_nr {
            // total_time is in seconds
            let total_time =
                (prc.stat.utime + prc.stat.stime) as f32 / (tps as f32);
            println!(
                "{: >5} {: <8} {: >8} {}",
                prc.stat.pid, tty, total_time, prc.stat.comm
            );
        }
    }
}
```

Here's another example that shows how to get the current memory usage of the current process:

```rust
use procfs::process::Process;

fn main() {
    let me = Process::myself().unwrap();
    println!("PID: {}", me.pid);

    let page_size = procfs::page_size().unwrap() as u64;
    println!("Memory page size: {}", page_size);

    println!("== Data from /proc/self/stat:");
    println!("Total virtual memory used: {} bytes", me.stat.vsize);
    println!("Total resident set: {} pages ({} bytes)", me.stat.rss, me.stat.rss as u64 * page_size);
}
```

There are a few ways to get this data, so also checkout the longer
[self_memory](https://github.com/eminence/procfs/blob/master/examples/self_memory.rs) example for more
details.

## Cargo features

The following cargo features are available:

* `chrono` -- Default.  Optional.  This feature enables a few methods that return values as `DateTime` objects.
* `flate2` -- Default.  Optional.  This feature enables parsing gzip compressed `/proc/config.gz` file via the `procfs::kernel_config` method.
* `backtrace` -- Optional.  This feature lets you get a stack trace whenever an `InternalError` is raised.

## Minimum Rust Version

This crate requires a minimum rust version of 1.34.0 (2019-04-11), though if you use the optional `backtrace` feature,
rust 1.42.0 is required (2020-03-12).  However, three dependencies of this crate (`hex`, `bitflags`, and `flate2`) require a newer
version of rust, and must be manually pinned to an older version in order to use rust 1.34.  You can do this by running:

    cargo update -p hex --precise 0.4.2
    cargo update -p bitflags --precise 1.2.1
    cargo update -p flate2 --precise 1.0.21

## License

The procfs library is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

For additional copyright information regarding documentation, please also see the COPYRIGHT.txt file.

### Contribution

Contributions are welcome, especially in the areas of documentation and testing on older kernels.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

