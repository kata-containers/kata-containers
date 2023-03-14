use std::io;
use std::io::{Read, Result};

use crate::fixed::FixedInt;
use crate::varint::{VarInt, MSB};

#[cfg(feature = "tokio_async")]
use tokio::{io::AsyncReadExt, prelude::*};

#[cfg(feature = "futures_async")]
use futures_util::{io::AsyncReadExt, io::AsyncRead};

/// A trait for reading VarInts from any other `Reader`.
///
/// It's recommended to use a buffered reader, as many small reads will happen.
pub trait VarIntReader {
    /// Returns either the decoded integer, or an error.
    ///
    /// In general, this always reads a whole varint. If the encoded varint's value is bigger
    /// than the valid value range of `VI`, then the value is truncated.
    ///
    /// On EOF, an io::Error with io::ErrorKind::UnexpectedEof is returned.
    fn read_varint<VI: VarInt>(&mut self) -> Result<VI>;
}

#[cfg(any(feature = "tokio_async", feature = "futures_async"))]
/// Like a VarIntReader, but returns a future.
#[async_trait::async_trait]
pub trait VarIntAsyncReader {
    async fn read_varint_async<VI: VarInt>(&mut self) -> Result<VI>;
}

/// VarIntProcessor encapsulates the logic for decoding a VarInt byte-by-byte.
#[derive(Default)]
pub struct VarIntProcessor {
    buf: [u8; 10],
    i: usize,
}

impl VarIntProcessor {
    fn push(&mut self, b: u8) -> Result<()> {
        if self.i >= 10 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unterminated varint",
            ));
        }
        self.buf[self.i] = b;
        self.i += 1;
        Ok(())
    }
    fn finished(&self) -> bool {
        (self.i > 0 && (self.buf[self.i - 1] & MSB == 0))
    }
    fn decode<VI: VarInt>(&self) -> VI {
        VI::decode_var(&self.buf[0..self.i]).0
    }
}

#[cfg(any(feature = "tokio_async", feature = "futures_async"))]
#[async_trait::async_trait]
impl<AR: AsyncRead + Unpin + Send> VarIntAsyncReader for AR {
    async fn read_varint_async<VI: VarInt>(&mut self) -> Result<VI> {
        let mut buf = [0 as u8; 1];
        let mut p = VarIntProcessor::default();

        while !p.finished() {
            let read = self.read(&mut buf).await?;

            // EOF
            if read == 0 && p.i == 0 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Reached EOF"));
            }
            if read == 0 {
                break;
            }

            p.push(buf[0])?;
        }

        Ok(p.decode())
    }
}

impl<R: Read> VarIntReader for R {
    fn read_varint<VI: VarInt>(&mut self) -> Result<VI> {
        let mut buf = [0 as u8; 1];
        let mut p = VarIntProcessor::default();

        while !p.finished() {
            let read = self.read(&mut buf)?;

            // EOF
            if read == 0 && p.i == 0 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Reached EOF"));
            }
            if read == 0 {
                break;
            }

            p.push(buf[0])?;
        }

        Ok(p.decode())
    }
}

/// A trait for reading FixedInts from any other `Reader`.
pub trait FixedIntReader {
    /// Read a fixed integer from a reader. How many bytes are read depends on `FI`.
    ///
    /// On EOF, an io::Error with io::ErrorKind::UnexpectedEof is returned.
    fn read_fixedint<FI: FixedInt>(&mut self) -> Result<FI>;
}

/// Like FixedIntReader, but returns a future.
#[cfg(any(feature = "tokio_async", feature = "futures_async"))]
#[async_trait::async_trait]
pub trait FixedIntAsyncReader {
    async fn read_fixedint_async<FI: FixedInt>(&mut self) -> Result<FI>;
}

#[cfg(any(feature = "tokio_async", feature = "futures_async"))]
#[async_trait::async_trait]
impl<AR: AsyncRead + Unpin + Send> FixedIntAsyncReader for AR {
    async fn read_fixedint_async<FI: FixedInt>(&mut self) -> Result<FI> {
        let mut buf = [0 as u8; 8];
        self.read_exact(&mut buf[0..FI::required_space()]).await?;
        Ok(FI::decode_fixed(&buf[0..FI::required_space()]))
    }
}

impl<R: Read> FixedIntReader for R {
    fn read_fixedint<FI: FixedInt>(&mut self) -> Result<FI> {
        let mut buf = [0 as u8; 8];
        self.read_exact(&mut buf[0..FI::required_space()])?;
        Ok(FI::decode_fixed(&buf[0..FI::required_space()]))
    }
}
