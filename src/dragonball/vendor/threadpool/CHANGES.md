# Changes

## 1.8.1

* [Fix a typo](https://github.com/rust-threadpool/rust-threadpool/pull/107)

## 1.8.0

* [Raise minimal rustc version to 1.13.0](https://github.com/rust-threadpool/rust-threadpool/pull/99)
* [Update num_cpus to 1.13.0](https://github.com/rust-threadpool/rust-threadpool/pull/105)

## 1.7.1

* [Join waves](https://github.com/rust-threadpool/rust-threadpool/pull/89)

## 1.7.0

* [Introduce `threadpool::Builder`](https://github.com/rust-threadpool/rust-threadpool/pull/83)
* [Add more hyperlinks to documentation](https://github.com/rust-threadpool/rust-threadpool/pull/87)
* [Add keywords and categories to Cargo.toml](https://github.com/rust-threadpool/rust-threadpool/pull/88)

## 1.6.0

* [Implement `PartialEq` and `Eq` for `ThreadPool`](https://github.com/rust-threadpool/rust-threadpool/pull/81)

## 1.5.0

* [Implement `Default` for `ThreadPool` use 'num_cpus' crate.](https://github.com/rust-threadpool/rust-threadpool/pull/72)

## 1.4.1

* [Introduce `with_name`, deprecate `new_with_name`](https://github.com/rust-threadpool/rust-threadpool/pull/73)
* [Documentation improvements](https://github.com/rust-threadpool/rust-threadpool/pull/71)

## 1.4.0

* [Implementation of the `join` operation](https://github.com/rust-threadpool/rust-threadpool/pull/63)

## 1.3.2

* [Enable `#[deprecated]` doc, requires Rust 1.9](https://github.com/rust-threadpool/rust-threadpool/pull/38)

## 1.3.1

* [Implement std::fmt::Debug for ThreadPool](https://github.com/rust-threadpool/rust-threadpool/pull/50)

## 1.3.0

* [Add barrier sync example](https://github.com/rust-threadpool/rust-threadpool/pull/35)
* [Rename `threads` method/params to `num_threads`, deprecate old usage](https://github.com/rust-threadpool/rust-threadpool/pull/34)
* [Stop using deprecated `sleep_ms` function in tests](https://github.com/rust-threadpool/rust-threadpool/pull/33)

## 1.2.0

* [New method to determine number of panicked threads](https://github.com/rust-threadpool/rust-threadpool/pull/31)

## 1.1.1

* [Silence warning related to unused result](https://github.com/rust-threadpool/rust-threadpool/pull/30)
* [Minor doc improvements](https://github.com/rust-threadpool/rust-threadpool/pull/30)

## 1.1.0

* [New constructor for specifying thread names for a thread pool](https://github.com/rust-threadpool/rust-threadpool/pull/28)

## 1.0.2

* [Use atomic counters](https://github.com/rust-threadpool/rust-threadpool/pull/25)

## 1.0.1

* [Switch active_count from Mutex to RwLock for more performance](https://github.com/rust-threadpool/rust-threadpool/pull/23)
