#![deny(
    missing_debug_implementations,
    unreachable_pub,
    rust_2018_idioms,
    missing_docs
)]

//! `hyperlocal` provides [Hyper](http://github.com/hyperium/hyper) bindings
//! for [Unix domain sockets](https://github.com/tokio-rs/tokio/tree/master/tokio-net/src/uds/).
//!
//! See the [`UnixClientExt`] docs for
//! how to configure clients.
//!
//! See the
//! [`UnixServerExt`] docs for how to
//! configure servers.
//!
//! The [`UnixConnector`] can be used in the [`hyper::Client`] builder
//! interface, if required.
//!
//! # Features
//!
//! - Client- enables the client extension trait and connector. *Enabled by
//!   default*.
//!
//! - Server- enables the server extension trait. *Enabled by default*.

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "client")]
pub use client::{UnixClientExt, UnixConnector};

#[cfg(feature = "server")]
mod server;
#[cfg(feature = "server")]
pub use server::UnixServerExt;

mod uri;
pub use uri::Uri;

#[cfg(feature = "server")]
pub use crate::server::conn::SocketIncoming;
