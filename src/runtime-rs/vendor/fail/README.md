# fail-rs

[![CI](https://github.com/tikv/fail-rs/workflows/CI/badge.svg)](https://github.com/tikv/fail-rs/actions)
[![Crates.io](https://img.shields.io/crates/v/fail.svg?maxAge=2592000)](https://crates.io/crates/fail)

[Documentation](https://docs.rs/fail).

A fail point implementation for Rust.

Fail points are code instrumentations that allow errors and other behavior to be injected dynamically at runtime, primarily for testing purposes. Fail points are flexible and can be configured to exhibit a variety of behavior, including panics, early returns, and sleeping. They can be controlled both programmatically and via the environment, and can be triggered conditionally and probabilistically.

This crate is inspired by FreeBSD's [failpoints](https://freebsd.org/cgi/man.cgi?query=fail).

## Usage

First, add this to your `Cargo.toml`:

```toml
[dependencies]
fail = "0.4"
```

Now you can import the `fail_point!` macro from the `fail` crate and use it to inject dynamic failures.
Fail points generation by this macro is disabled by default, and can be enabled where relevant with the `failpoints` Cargo feature.

As an example, here's a simple program that uses a fail point to simulate an I/O panic:

```rust
use fail::{fail_point, FailScenario};

fn do_fallible_work() {
    fail_point!("read-dir");
    let _dir: Vec<_> = std::fs::read_dir(".").unwrap().collect();
    // ... do some work on the directory ...
}

fn main() {
    let scenario = FailScenario::setup();
    do_fallible_work();
    scenario.teardown();
    println!("done");
}
```

Here, the program calls `unwrap` on the result of `read_dir`, a function that returns a `Result`. In other words, this particular program expects this call to `read_dir` to always succeed. And in practice it almost always will, which makes the behavior of this program when `read_dir` fails difficult to test. By instrumenting the program with a fail point we can pretend that `read_dir` failed, causing the subsequent `unwrap` to panic, and allowing us to observe the program's behavior under failure conditions.

When the program is run normally it just prints "done":

```sh
$ cargo run --features fail/failpoints
    Finished dev [unoptimized + debuginfo] target(s) in 0.01s
     Running `target/debug/failpointtest`
done
```

But now, by setting the `FAILPOINTS` variable we can see what happens if the `read_dir` fails:

```
FAILPOINTS=read-dir=panic cargo run --features fail/failpoints
    Finished dev [unoptimized + debuginfo] target(s) in 0.01s
     Running `target/debug/failpointtest`
thread 'main' panicked at 'failpoint read-dir panic', /home/ubuntu/.cargo/registry/src/github.com-1ecc6299db9ec823/fail-0.2.0/src/lib.rs:286:25
note: Run with `RUST_BACKTRACE=1` for a backtrace.
```

For further information see the [API documentation](https://docs.rs/fail).


## TODO

Triggering a fail point via the HTTP API is planned but not implemented yet.
