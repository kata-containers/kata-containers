//! Types which operate over [`Stream`](futures_core::stream::Stream)`<Item =
//! `[`io::Result`](std::io::Result)`<`[`Bytes`](bytes_05::Bytes)`>>` streams, both encoders and
//! decoders for various formats.
//!
//! The `Stream` is treated as a single byte-stream to be compressed/decompressed, each item is a
//! chunk of data from this byte-stream. There is not guaranteed to be a one-to-one relationship
//! between chunks of data from the underlying stream and the resulting compressed/decompressed
//! stream, the encoders and decoders will buffer the incoming data and choose their own boundaries
//! at which to yield a new item.
//!
//! # Deprecation Migration
//!
//! This feature and module was deprecated because it's choosing one point in a large solution
//! space of "stream of byte chunks" to represent an IO data stream, and the conversion between
//! these solutions and standard IO data streams like `futures::io::AsyncBufRead` /
//! `tokio::io::AsyncBufRead` should be zero-cost.
//!
//! ```rust
//! use bytes_05::Bytes;
//! use futures::{stream::Stream, TryStreamExt};
//! use std::io::Result;
//!
//! /// For code that looks like this, choose one of the options below to replace it
//! fn from(
//!     input: impl Stream<Item = Result<bytes_05::Bytes>>,
//! ) -> impl Stream<Item = Result<bytes_05::Bytes>> {
//!     #[allow(deprecated)]
//!     async_compression::stream::GzipEncoder::new(input)
//! }
//!
//! /// Direct replacement with `tokio` v0.2 and `bytes` v0.5 using `tokio-util` v0.3
//! fn tokio_02_bytes_05(
//!     input: impl Stream<Item = Result<bytes_05::Bytes>>,
//! ) -> impl Stream<Item = Result<bytes_05::Bytes>> {
//!     tokio_util_03::codec::FramedRead::new(
//!         async_compression::tokio_02::bufread::GzipEncoder::new(
//!             tokio_02::io::stream_reader(input),
//!         ),
//!         tokio_util_03::codec::BytesCodec::new(),
//!     ).map_ok(|bytes| bytes.freeze())
//! }
//!
//! /// Upgrade replacement with `tokio` v0.3 and `bytes` v0.5 using `tokio-util` v0.4
//! fn tokio_03_bytes_05(
//!     input: impl Stream<Item = Result<bytes_05::Bytes>>,
//! ) -> impl Stream<Item = Result<bytes_05::Bytes>> {
//!     tokio_util_04::io::ReaderStream::new(
//!         async_compression::tokio_03::bufread::GzipEncoder::new(
//!             tokio_util_04::io::StreamReader::new(input),
//!         ),
//!     )
//! }
//!
//! /// Upgrade replacement with `tokio` v0.3 and `bytes` v0.6 using `tokio-util` v0.5
//! fn tokio_03_bytes_06(
//!     input: impl Stream<Item = Result<bytes_06::Bytes>>,
//! ) -> impl Stream<Item = Result<bytes_06::Bytes>> {
//!     tokio_util_05::io::ReaderStream::new(
//!         async_compression::tokio_03::bufread::GzipEncoder::new(
//!             tokio_util_05::io::StreamReader::new(input),
//!         ),
//!     )
//! }
//!
//! /// Upgrade replacement with `tokio` v1.0 and `bytes` v1.0 using `tokio-util` v0.6
//! fn tokio_bytes(
//!     input: impl Stream<Item = Result<bytes::Bytes>>,
//! ) -> impl Stream<Item = Result<bytes::Bytes>> {
//!     tokio_util_06::io::ReaderStream::new(
//!         async_compression::tokio::bufread::GzipEncoder::new(
//!             tokio_util_06::io::StreamReader::new(input),
//!         ),
//!     )
//! }
//!
//! /// What if you didn't want anything to do with `bytes`, but just a `Vec<u8>` instead?
//! fn futures_vec(
//!     input: impl Stream<Item = Result<Vec<u8>>> + Unpin,
//! ) -> impl Stream<Item = Result<Vec<u8>>> {
//!     use futures::io::AsyncReadExt;
//!
//!     futures::stream::try_unfold(
//!         async_compression::futures::bufread::GzipEncoder::new(input.into_async_read()),
//!         |mut encoder| async move {
//!             let mut chunk = vec![0; 8 * 1024];
//!             let len = encoder.read(&mut chunk).await?;
//!             if len == 0 {
//!                 Ok(None)
//!             } else {
//!                 chunk.truncate(len);
//!                 Ok(Some((chunk, encoder)))
//!             }
//!         })
//! }
//! #
//! # futures::executor::block_on(async {
//! #     let data = || futures::stream::iter(vec![Ok(vec![1, 2, 3]), Ok(vec![4, 5, 6])]);
//! #     let expected: Vec<Vec<u8>> = from(data().map_ok(bytes_05::Bytes::from))
//! #         .map_ok(|bytes| bytes.as_ref().into())
//! #         .try_collect()
//! #         .await?;
//! #
//! #     assert_eq!(
//! #         expected,
//! #         tokio_02_bytes_05(data().map_ok(bytes_05::Bytes::from))
//! #             .map_ok(|bytes| bytes.as_ref().into())
//! #             .try_collect::<Vec<Vec<u8>>>()
//! #             .await?,
//! #     );
//! #     assert_eq!(
//! #         expected,
//! #         tokio_03_bytes_05(data().map_ok(bytes_05::Bytes::from))
//! #             .map_ok(|bytes| bytes.as_ref().into())
//! #             .try_collect::<Vec<Vec<u8>>>()
//! #             .await?,
//! #     );
//! #     assert_eq!(
//! #         expected,
//! #         tokio_03_bytes_06(data().map_ok(bytes_06::Bytes::from))
//! #             .map_ok(|bytes| bytes.as_ref().into())
//! #             .try_collect::<Vec<Vec<u8>>>()
//! #             .await?,
//! #     );
//! #     assert_eq!(
//! #         expected,
//! #         tokio_bytes(data().map_ok(bytes::Bytes::from))
//! #             .map_ok(|bytes| bytes.as_ref().into())
//! #             .try_collect::<Vec<Vec<u8>>>()
//! #             .await?,
//! #     );
//! #     assert_eq!(
//! #         expected,
//! #         futures_vec(data())
//! #             .try_collect::<Vec<Vec<u8>>>()
//! #             .await?
//! #     );
//! #     Ok::<_, std::io::Error>(())
//! # })?; Ok::<_, std::io::Error>(())
//! ```

#![deprecated(
    since = "0.3.8",
    note = "See `async-compression::stream` docs for migration"
)]

#[macro_use]
mod macros;
mod generic;

pub(crate) use self::generic::{Decoder, Encoder};

algos!(stream<S>);
