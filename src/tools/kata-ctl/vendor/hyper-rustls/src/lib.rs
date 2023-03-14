//! # hyper-rustls
//!
//! A pure-Rust HTTPS connector for [hyper](https://hyper.rs), based on
//! [Rustls](https://github.com/ctz/rustls).
//!
//! ## Example
//!
//! ```no_run
//! # #[cfg(all(feature = "rustls-native-certs", feature = "tokio-runtime", feature = "http1"))]
//! # fn main() {
//! use hyper::{Body, Client, StatusCode, Uri};
//!
//! let mut rt = tokio::runtime::Runtime::new().unwrap();
//! let url = ("https://hyper.rs").parse().unwrap();
//! let https = hyper_rustls::HttpsConnectorBuilder::new()
//!     .with_native_roots()
//!     .https_only()
//!     .enable_http1()
//!     .build();
//!
//! let client: Client<_, hyper::Body> = Client::builder().build(https);
//!
//! let res = rt.block_on(client.get(url)).unwrap();
//! assert_eq!(res.status(), StatusCode::OK);
//! # }
//! # #[cfg(not(all(feature = "rustls-native-certs", feature = "tokio-runtime", feature = "http1")))]
//! # fn main() {}
//! ```

#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod config;
mod connector;
mod stream;

#[cfg(feature = "logging")]
mod log {
    pub use log::{debug, trace};
}

#[cfg(not(feature = "logging"))]
mod log {
    macro_rules! trace    ( ($($tt:tt)*) => {{}} );
    macro_rules! debug    ( ($($tt:tt)*) => {{}} );
    pub(crate) use {debug, trace};
}

pub use crate::config::ConfigBuilderExt;
pub use crate::connector::builder::ConnectorBuilder as HttpsConnectorBuilder;
pub use crate::connector::HttpsConnector;
pub use crate::stream::MaybeHttpsStream;

/// The various states of the [`HttpsConnectorBuilder`]
pub mod builderstates {
    #[cfg(feature = "http2")]
    pub use crate::connector::builder::WantsProtocols3;
    pub use crate::connector::builder::{
        WantsProtocols1, WantsProtocols2, WantsSchemes, WantsTlsConfig,
    };
}
