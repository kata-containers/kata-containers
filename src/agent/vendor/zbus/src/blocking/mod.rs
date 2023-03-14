//! The blocking API.
//!
//! This module hosts all our blocking API. All the types under this module are thin wrappers
//! around the corresponding asynchronous types. Most of the method calls are simply calling their
//! asynchronous counterparts on the underlying types and use [`async_io::block_on`] to turn them
//! into blocking calls.
//!
//! # Caveats
//!
//! Since methods provided by these types run their own little runtime (`block_on`), you must not
//! call by them from async contexts because of the infamous [async sandwich footgun][asf]. This is
//! is an especially important fact to keep in mind for [`crate::dbus_interface`]. While
//! `dbus_interface` allows non-async methods for convenience, these methods are called from an
//! async context. The [`blocking` crate] provides an easy way around this problem though.
//!
//! [asf]: https://rust-lang.github.io/wg-async-foundations/vision/shiny_future/users_manual.html#caveat-beware-the-async-sandwich
//! [`blocking` crate]: https://docs.rs/blocking/

mod connection;
pub use connection::*;
mod connection_builder;
pub use connection_builder::*;
mod message_iterator;
pub use message_iterator::*;
mod object_server;
pub use object_server::*;
mod proxy;
pub use proxy::*;
mod proxy_builder;
pub use proxy_builder::*;
pub mod fdo;
