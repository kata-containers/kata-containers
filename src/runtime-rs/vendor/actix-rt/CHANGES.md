# Changes

## Unreleased - 2022-xx-xx


## 2.7.0 - 2022-03-08
- Update `tokio-uring` dependency to `0.3.0`. [#448]
- Minimum supported Rust version (MSRV) is now 1.49.

[#448]: https://github.com/actix/actix-net/pull/448


## 2.6.0 - 2022-01-12
- Update `tokio-uring` dependency to `0.2.0`. [#436]

[#436]: https://github.com/actix/actix-net/pull/436


## 2.5.1 - 2021-12-31
- Expose `System::with_tokio_rt` and `Arbiter::with_tokio_rt`. [#430]

[#430]: https://github.com/actix/actix-net/pull/430


## 2.5.0 - 2021-11-22
- Add `System::run_with_code` to allow retrieving the exit code on stop. [#411]

[#411]: https://github.com/actix/actix-net/pull/411


## 2.4.0 - 2021-11-05
- Add `Arbiter::try_current` for situations where thread may or may not have Arbiter context. [#408]
- Start io-uring with `System::new` when feature is enabled. [#395]

[#395]: https://github.com/actix/actix-net/pull/395
[#408]: https://github.com/actix/actix-net/pull/408


## 2.3.0 - 2021-10-11
- The `spawn` method can now resolve with non-unit outputs. [#369]
- Add experimental (semver-exempt) `io-uring` feature for enabling async file I/O on linux. [#374]

[#369]: https://github.com/actix/actix-net/pull/369
[#374]: https://github.com/actix/actix-net/pull/374


## 2.2.0 - 2021-03-29
- **BREAKING** `ActixStream::{poll_read_ready, poll_write_ready}` methods now return
  `Ready` object in ok variant. [#293]
  * Breakage is acceptable since `ActixStream` was not intended to be public.

[#293]: https://github.com/actix/actix-net/pull/293


## 2.1.0 - 2021-02-24
- Add `ActixStream` extension trait to include readiness methods. [#276]
- Re-export `tokio::net::TcpSocket` in `net` module [#282]

[#276]: https://github.com/actix/actix-net/pull/276
[#282]: https://github.com/actix/actix-net/pull/282


## 2.0.2 - 2021-02-06
- Add `Arbiter::handle` to get a handle of an owned Arbiter. [#274]
- Add `System::try_current` for situations where actix may or may not be running a System. [#275]

[#274]: https://github.com/actix/actix-net/pull/274
[#275]: https://github.com/actix/actix-net/pull/275


## 2.0.1 - 2021-02-06
- Expose `JoinError` from Tokio. [#271]

[#271]: https://github.com/actix/actix-net/pull/271


## 2.0.0 - 2021-02-02
- Remove all Arbiter-local storage methods. [#262]
- Re-export `tokio::pin`. [#262]

[#262]: https://github.com/actix/actix-net/pull/262


## 2.0.0-beta.3 - 2021-01-31
- Remove `run_in_tokio`, `attach_to_tokio` and `AsyncSystemRunner`. [#253]
- Return `JoinHandle` from `actix_rt::spawn`. [#253]
- Remove old `Arbiter::spawn`. Implementation is now inlined into `actix_rt::spawn`. [#253]
- Rename `Arbiter::{send => spawn}` and `Arbiter::{exec_fn => spawn_fn}`. [#253]
- Remove `Arbiter::exec`. [#253]
- Remove deprecated `Arbiter::local_join` and `Arbiter::is_running`. [#253]
- `Arbiter::spawn` now accepts !Unpin futures. [#256]
- `System::new` no longer takes arguments. [#257]
- Remove `System::with_current`. [#257]
- Remove `Builder`. [#257]
- Add `System::with_init` as replacement for `Builder::run`. [#257]
- Rename `System::{is_set => is_registered}`. [#257]
- Add `ArbiterHandle` for sending messages to non-current-thread arbiters. [#257].
- `System::arbiter` now returns an `&ArbiterHandle`. [#257]
- `Arbiter::current` now returns an `ArbiterHandle` instead. [#257]
- `Arbiter::join` now takes self by value. [#257]

[#253]: https://github.com/actix/actix-net/pull/253
[#254]: https://github.com/actix/actix-net/pull/254
[#256]: https://github.com/actix/actix-net/pull/256
[#257]: https://github.com/actix/actix-net/pull/257


## 2.0.0-beta.2 - 2021-01-09
- Add `task` mod with re-export of `tokio::task::{spawn_blocking, yield_now, JoinHandle}` [#245]
- Add default "macros" feature to allow faster compile times when using `default-features=false`.

[#245]: https://github.com/actix/actix-net/pull/245


## 2.0.0-beta.1 - 2020-12-28
- Add `System::attach_to_tokio` method. [#173]
- Update `tokio` dependency to `1.0`. [#236]
- Rename `time` module `delay_for` to `sleep`, `delay_until` to `sleep_until`, `Delay` to `Sleep`
  to stay aligned with Tokio's naming. [#236]
- Remove `'static` lifetime requirement for `Runtime::block_on` and `SystemRunner::block_on`.
  * These methods now accept `&self` when calling. [#236]
- Remove `'static` lifetime requirement for `System::run` and `Builder::run`. [#236]
- `Arbiter::spawn` now panics when `System` is not in scope. [#207]
- Fix work load issue by removing `PENDING` thread local. [#207]

[#207]: https://github.com/actix/actix-net/pull/207
[#236]: https://github.com/actix/actix-net/pull/236


## 1.1.1 - 2020-04-30
- Fix memory leak due to [#94] (see [#129] for more detail)

[#129]: https://github.com/actix/actix-net/issues/129


## 1.1.0 - 2020-04-08 _(YANKED)_
- Expose `System::is_set` to check if current system has ben started [#99]
- Add `Arbiter::is_running` to check if event loop is running [#124]
- Add `Arbiter::local_join` associated function
  to get be able to `await` for spawned futures [#94]

[#94]: https://github.com/actix/actix-net/pull/94
[#99]: https://github.com/actix/actix-net/pull/99
[#124]: https://github.com/actix/actix-net/pull/124


## 1.0.0 - 2019-12-11
- Update dependencies


## 1.0.0-alpha.3 - 2019-12-07
- Migrate to tokio 0.2
- Fix compilation on non-unix platforms


## 1.0.0-alpha.2 - 2019-12-02
- Export `main` and `test` attribute macros
- Export `time` module (re-export of tokio-timer)
- Export `net` module (re-export of tokio-net)


## 1.0.0-alpha.1 - 2019-11-22
- Migrate to std::future and tokio 0.2


## 0.2.6 - 2019-11-14
- Allow to join arbiter's thread. #60
- Fix arbiter's thread panic message.


## 0.2.5 - 2019-09-02
- Add arbiter specific storage


## 0.2.4 - 2019-07-17
- Avoid a copy of the Future when initializing the Box. #29


## 0.2.3 - 2019-06-22
- Allow to start System using existing CurrentThread Handle #22


## 0.2.2 - 2019-03-28
- Moved `blocking` module to `actix-threadpool` crate


## 0.2.1 - 2019-03-11
- Added `blocking` module
- Added `Arbiter::exec_fn` - execute fn on the arbiter's thread
- Added `Arbiter::exec` - execute fn on the arbiter's thread and wait result


## 0.2.0 - 2019-03-06
- `run` method returns `io::Result<()>`
- Removed `Handle`


## 0.1.0 - 2018-12-09
- Initial release
