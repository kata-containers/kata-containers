# Linux IO Uring
[![github actions](https://github.com/tokio-rs/io-uring/workflows/ci/badge.svg)](https://github.com/tokio-rs/io-uring/actions)
[![crates](https://img.shields.io/crates/v/io-uring.svg)](https://crates.io/crates/io-uring)
[![license](https://img.shields.io/badge/License-MIT-blue.svg)](https://github.com/tokio-rs/io-uring/blob/master/LICENSE-MIT)
[![license](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://github.com/tokio-rs/io-uring/blob/master/LICENSE-APACHE)
[![docs.rs](https://docs.rs/io-uring/badge.svg)](https://docs.rs/io-uring/)

The low-level [`io_uring`](https://kernel.dk/io_uring.pdf) userspace interface for Rust.

## Usage

To use `io-uring` crate, first add this to your `Cargo.toml`:

```toml
[dependencies]
io-uring = "0.5"
```

Next we can start using `io-uring` crate.
The following is quick introduction using `Read` for file.

```rust
use io_uring::{opcode, types, IoUring};
use std::os::unix::io::AsRawFd;
use std::{fs, io};

fn main() -> io::Result<()> {
    let mut ring = IoUring::new(8)?;

    let fd = fs::File::open("README.md")?;
    let mut buf = vec![0; 1024];

    let read_e = opcode::Read::new(types::Fd(fd.as_raw_fd()), buf.as_mut_ptr(), buf.len() as _)
        .build()
        .user_data(0x42);

    // Note that the developer needs to ensure
    // that the entry pushed into submission queue is valid (e.g. fd, buffer).
    unsafe {
        ring.submission()
            .push(&read_e)
            .expect("submission queue is full");
    }

    ring.submit_and_wait(1)?;

    let cqe = ring.completion().next().expect("completion queue is empty");

    assert_eq!(cqe.user_data(), 0x42);
    assert!(cqe.result() >= 0, "read error: {}", cqe.result());

    Ok(())
}
```

Note that opcode `Read` is only available after kernel 5.6.
If you use a kernel lower than 5.6, this example will fail.

## Test and Benchmarks

You can run the test and benchmark of the library with the following commands.

```
$ cargo run --package io-uring-test
$ cargo bench --package io-uring-bench
```


### License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in io-uring by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
