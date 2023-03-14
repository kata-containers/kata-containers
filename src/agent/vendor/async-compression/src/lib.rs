//! Adaptors between compression crates and Rust's modern asynchronous IO types.
//!

//! # Feature Organization
//!
//! This crate is divided up along two axes, which can each be individually selected via Cargo
//! features.
//!
//! All features are disabled by default, you should enable just the ones you need from the lists
//! below.
//!
//! If you want to pull in everything there are three group features defined:
//!

//!  Feature | Does
//! ---------|------
//!  `all`   | Activates all implementations and algorithms.
//!  `all-implementations` | Activates all implementations, needs to be paired with a selection of algorithms
//!  `all-algorithms` | Activates all algorithms, needs to be paired with a selection of implementations
//!

//! ## IO implementation
//!
//! The first division is which underlying asynchronous IO trait will be wrapped, these are
//! available as separate features that have corresponding top-level modules:
//!

//!  Feature | Type
//! ---------|------
// TODO: Kill rustfmt on this section, `#![rustfmt::skip::attributes(cfg_attr)]` should do it, but
// that's unstable
#![cfg_attr(
    feature = "futures-io",
    doc = "[`futures-io`](crate::futures) | [`futures::io::AsyncBufRead`](futures_io::AsyncBufRead), [`futures::io::AsyncWrite`](futures_io::AsyncWrite)"
)]
#![cfg_attr(
    not(feature = "futures-io"),
    doc = "`futures-io` (*inactive*) | `futures::io::AsyncBufRead`, `futures::io::AsyncWrite`"
)]
#![cfg_attr(
    feature = "futures-bufread",
    doc = "`futures-bufread` | (*deprecated*, use `futures-io`)"
)]
#![cfg_attr(
    feature = "futures-write",
    doc = "`futures-write` | (*deprecated*, use `futures-io`)"
)]
#![cfg_attr(
    feature = "stream",
    doc = "[`stream`] | (*deprecated*, see [`async-compression:stream`](crate::stream) docs for migration)"
)]
#![cfg_attr(
    not(feature = "stream"),
    doc = "`stream` (*inactive*) | (*deprecated*, see `async-compression::stream` docs for migration)"
)]
#![cfg_attr(
    feature = "tokio-02",
    doc = "[`tokio-02`](crate::tokio_02) | [`tokio::io::AsyncBufRead`](::tokio_02::io::AsyncBufRead), [`tokio::io::AsyncWrite`](::tokio_02::io::AsyncWrite)"
)]
#![cfg_attr(
    not(feature = "tokio-02"),
    doc = "`tokio-02` (*inactive*) | `tokio::io::AsyncBufRead`, `tokio::io::AsyncWrite`"
)]
#![cfg_attr(
    feature = "tokio-03",
    doc = "[`tokio-03`](crate::tokio_03) | [`tokio::io::AsyncBufRead`](::tokio_03::io::AsyncBufRead), [`tokio::io::AsyncWrite`](::tokio_03::io::AsyncWrite)"
)]
#![cfg_attr(
    not(feature = "tokio-03"),
    doc = "`tokio-03` (*inactive*) | `tokio::io::AsyncBufRead`, `tokio::io::AsyncWrite`"
)]
#![cfg_attr(
    feature = "tokio",
    doc = "[`tokio`](crate::tokio) | [`tokio::io::AsyncBufRead`](::tokio::io::AsyncBufRead), [`tokio::io::AsyncWrite`](::tokio::io::AsyncWrite)"
)]
#![cfg_attr(
    not(feature = "tokio"),
    doc = "`tokio` (*inactive*) | `tokio::io::AsyncBufRead`, `tokio::io::AsyncWrite`"
)]
//!

//! ## Compression algorithm
//!
//! The second division is which compression schemes to support, there are currently a few
//! available choices, these determine which types will be available inside the above modules:
//!

//!  Feature | Types
//! ---------|------
#![cfg_attr(
    feature = "brotli",
    doc = "`brotli` | [`BrotliEncoder`](?search=BrotliEncoder), [`BrotliDecoder`](?search=BrotliDecoder)"
)]
#![cfg_attr(
    not(feature = "brotli"),
    doc = "`brotli` (*inactive*) | `BrotliEncoder`, `BrotliDecoder`"
)]
#![cfg_attr(
    feature = "bzip2",
    doc = "`bzip2` | [`BzEncoder`](?search=BzEncoder), [`BzDecoder`](?search=BzDecoder)"
)]
#![cfg_attr(
    not(feature = "bzip2"),
    doc = "`bzip2` (*inactive*) | `BzEncoder`, `BzDecoder`"
)]
#![cfg_attr(
    feature = "deflate",
    doc = "`deflate` | [`DeflateEncoder`](?search=DeflateEncoder), [`DeflateDecoder`](?search=DeflateDecoder)"
)]
#![cfg_attr(
    not(feature = "deflate"),
    doc = "`deflate` (*inactive*) | `DeflateEncoder`, `DeflateDecoder`"
)]
#![cfg_attr(
    feature = "gzip",
    doc = "`gzip` | [`GzipEncoder`](?search=GzipEncoder), [`GzipDecoder`](?search=GzipDecoder)"
)]
#![cfg_attr(
    not(feature = "gzip"),
    doc = "`gzip` (*inactive*) | `GzipEncoder`, `GzipDecoder`"
)]
#![cfg_attr(
    feature = "lzma",
    doc = "`lzma` | [`LzmaEncoder`](?search=LzmaEncoder), [`LzmaDecoder`](?search=LzmaDecoder)"
)]
#![cfg_attr(
    not(feature = "lzma"),
    doc = "`lzma` (*inactive*) | `LzmaEncoder`, `LzmaDecoder`"
)]
#![cfg_attr(
    feature = "xz",
    doc = "`xz` | [`XzEncoder`](?search=XzEncoder), [`XzDecoder`](?search=XzDecoder)"
)]
#![cfg_attr(
    not(feature = "xz"),
    doc = "`xz` (*inactive*) | `XzEncoder`, `XzDecoder`"
)]
#![cfg_attr(
    feature = "zlib",
    doc = "`zlib` | [`ZlibEncoder`](?search=ZlibEncoder), [`ZlibDecoder`](?search=ZlibDecoder)"
)]
#![cfg_attr(
    not(feature = "zlib"),
    doc = "`zlib` (*inactive*) | `ZlibEncoder`, `ZlibDecoder`"
)]
#![cfg_attr(
    feature = "zstd",
    doc = "`zstd` | [`ZstdEncoder`](?search=ZstdEncoder), [`ZstdDecoder`](?search=ZstdDecoder)"
)]
#![cfg_attr(
    not(feature = "zstd"),
    doc = "`zstd` (*inactive*) | `ZstdEncoder`, `ZstdDecoder`"
)]
//!

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(
    missing_docs,
    rust_2018_idioms,
    missing_copy_implementations,
    missing_debug_implementations
)]
#![cfg_attr(not(all), allow(unused))]

#[macro_use]
mod macros;
mod codec;

#[cfg(feature = "futures-io")]
#[cfg_attr(docsrs, doc(cfg(feature = "futures-io")))]
pub mod futures;
#[cfg(feature = "stream")]
#[cfg_attr(docsrs, doc(cfg(feature = "stream")))]
pub mod stream;
#[cfg(feature = "tokio")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio")))]
pub mod tokio;
#[cfg(feature = "tokio-02")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio-02")))]
pub mod tokio_02;
#[cfg(feature = "tokio-03")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio-03")))]
pub mod tokio_03;

mod unshared;
mod util;

#[cfg(feature = "brotli")]
use brotli::enc::backward_references::BrotliEncoderParams;

/// Level of compression data should be compressed with.
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub enum Level {
    /// Fastest quality of compression, usually produces bigger size.
    Fastest,
    /// Best quality of compression, usually produces the smallest size.
    Best,
    /// Default quality of compression defined by the selected compression algorithm.
    Default,
    /// Precise quality based on the underlying compression algorithms'
    /// qualities. The interpretation of this depends on the algorithm chosen
    /// and the specific implementation backing it.
    /// Qualities are implicitly clamped to the algorithm's maximum.
    Precise(u32),
}

impl Level {
    #[cfg(feature = "brotli")]
    fn into_brotli(self, mut params: BrotliEncoderParams) -> BrotliEncoderParams {
        match self {
            Self::Fastest => params.quality = 0,
            Self::Best => params.quality = 11,
            Self::Precise(quality) => params.quality = quality.min(11) as i32,
            Self::Default => (),
        }

        params
    }

    #[cfg(feature = "bzip2")]
    fn into_bzip2(self) -> bzip2::Compression {
        match self {
            Self::Fastest => bzip2::Compression::fast(),
            Self::Best => bzip2::Compression::best(),
            Self::Precise(quality) => bzip2::Compression::new(quality.max(1).min(9)),
            Self::Default => bzip2::Compression::default(),
        }
    }

    #[cfg(feature = "flate2")]
    fn into_flate2(self) -> flate2::Compression {
        match self {
            Self::Fastest => flate2::Compression::fast(),
            Self::Best => flate2::Compression::best(),
            Self::Precise(quality) => flate2::Compression::new(quality.min(10)),
            Self::Default => flate2::Compression::default(),
        }
    }

    #[cfg(feature = "zstd")]
    fn into_zstd(self) -> i32 {
        match self {
            Self::Fastest => 1,
            Self::Best => 21,
            Self::Precise(quality) => quality.min(21) as i32,
            Self::Default => libzstd::DEFAULT_COMPRESSION_LEVEL,
        }
    }

    #[cfg(feature = "xz2")]
    fn into_xz2(self) -> u32 {
        match self {
            Self::Fastest => 0,
            Self::Best => 9,
            Self::Precise(quality) => quality.min(9),
            Self::Default => 5,
        }
    }
}
