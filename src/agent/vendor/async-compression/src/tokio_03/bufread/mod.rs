//! Types which operate over [`AsyncBufRead`](::tokio_03::io::AsyncBufRead) streams, both encoders and
//! decoders for various formats.

#[macro_use]
mod macros;
mod generic;

pub(crate) use generic::{Decoder, Encoder};

algos!(tokio_03::bufread<R>);
