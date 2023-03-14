use std::io;
use std::io::prelude::*;
use std::mem;

use crate::zio;
use crate::{Compress, Decompress};

/// A ZLIB encoder, or compressor.
///
/// This structure consumes a [`BufRead`] interface, reading uncompressed data
/// from the underlying reader, and emitting compressed data.
///
/// [`BufRead`]: https://doc.rust-lang.org/std/io/trait.BufRead.html
///
/// # Examples
///
/// ```
/// use std::io::prelude::*;
/// use flate2::Compression;
/// use flate2::bufread::ZlibEncoder;
/// use std::fs::File;
/// use std::io::BufReader;
///
/// // Use a buffered file to compress contents into a Vec<u8>
///
/// # fn open_hello_world() -> std::io::Result<Vec<u8>> {
/// let f = File::open("examples/hello_world.txt")?;
/// let b = BufReader::new(f);
/// let mut z = ZlibEncoder::new(b, Compression::fast());
/// let mut buffer = Vec::new();
/// z.read_to_end(&mut buffer)?;
/// # Ok(buffer)
/// # }
/// ```
#[derive(Debug)]
pub struct ZlibEncoder<R> {
    obj: R,
    data: Compress,
}

impl<R: BufRead> ZlibEncoder<R> {
    /// Creates a new encoder which will read uncompressed data from the given
    /// stream and emit the compressed stream.
    pub fn new(r: R, level: crate::Compression) -> ZlibEncoder<R> {
        ZlibEncoder {
            obj: r,
            data: Compress::new(level, true),
        }
    }
}

pub fn reset_encoder_data<R>(zlib: &mut ZlibEncoder<R>) {
    zlib.data.reset()
}

impl<R> ZlibEncoder<R> {
    /// Resets the state of this encoder entirely, swapping out the input
    /// stream for another.
    ///
    /// This function will reset the internal state of this encoder and replace
    /// the input stream with the one provided, returning the previous input
    /// stream. Future data read from this encoder will be the compressed
    /// version of `r`'s data.
    pub fn reset(&mut self, r: R) -> R {
        reset_encoder_data(self);
        mem::replace(&mut self.obj, r)
    }

    /// Acquires a reference to the underlying reader
    pub fn get_ref(&self) -> &R {
        &self.obj
    }

    /// Acquires a mutable reference to the underlying stream
    ///
    /// Note that mutation of the stream may result in surprising results if
    /// this encoder is continued to be used.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.obj
    }

    /// Consumes this encoder, returning the underlying reader.
    pub fn into_inner(self) -> R {
        self.obj
    }

    /// Returns the number of bytes that have been read into this compressor.
    ///
    /// Note that not all bytes read from the underlying object may be accounted
    /// for, there may still be some active buffering.
    pub fn total_in(&self) -> u64 {
        self.data.total_in()
    }

    /// Returns the number of bytes that the compressor has produced.
    ///
    /// Note that not all bytes may have been read yet, some may still be
    /// buffered.
    pub fn total_out(&self) -> u64 {
        self.data.total_out()
    }
}

impl<R: BufRead> Read for ZlibEncoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        zio::read(&mut self.obj, &mut self.data, buf)
    }
}

impl<R: BufRead + Write> Write for ZlibEncoder<R> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.get_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.get_mut().flush()
    }
}

/// A ZLIB decoder, or decompressor.
///
/// This structure consumes a [`BufRead`] interface, reading compressed data
/// from the underlying reader, and emitting uncompressed data.
///
/// [`BufRead`]: https://doc.rust-lang.org/std/io/trait.BufRead.html
///
/// # Examples
///
/// ```
/// use std::io::prelude::*;
/// use std::io;
/// # use flate2::Compression;
/// # use flate2::write::ZlibEncoder;
/// use flate2::bufread::ZlibDecoder;
///
/// # fn main() {
/// # let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
/// # e.write_all(b"Hello World").unwrap();
/// # let bytes = e.finish().unwrap();
/// # println!("{}", decode_bufreader(bytes).unwrap());
/// # }
/// #
/// // Uncompresses a Zlib Encoded vector of bytes and returns a string or error
/// // Here &[u8] implements BufRead
///
/// fn decode_bufreader(bytes: Vec<u8>) -> io::Result<String> {
///     let mut z = ZlibDecoder::new(&bytes[..]);
///     let mut s = String::new();
///     z.read_to_string(&mut s)?;
///     Ok(s)
/// }
/// ```
#[derive(Debug)]
pub struct ZlibDecoder<R> {
    obj: R,
    data: Decompress,
}

impl<R: BufRead> ZlibDecoder<R> {
    /// Creates a new decoder which will decompress data read from the given
    /// stream.
    pub fn new(r: R) -> ZlibDecoder<R> {
        ZlibDecoder {
            obj: r,
            data: Decompress::new(true),
        }
    }
}

pub fn reset_decoder_data<R>(zlib: &mut ZlibDecoder<R>) {
    zlib.data = Decompress::new(true);
}

impl<R> ZlibDecoder<R> {
    /// Resets the state of this decoder entirely, swapping out the input
    /// stream for another.
    ///
    /// This will reset the internal state of this decoder and replace the
    /// input stream with the one provided, returning the previous input
    /// stream. Future data read from this decoder will be the decompressed
    /// version of `r`'s data.
    pub fn reset(&mut self, r: R) -> R {
        reset_decoder_data(self);
        mem::replace(&mut self.obj, r)
    }

    /// Acquires a reference to the underlying stream
    pub fn get_ref(&self) -> &R {
        &self.obj
    }

    /// Acquires a mutable reference to the underlying stream
    ///
    /// Note that mutation of the stream may result in surprising results if
    /// this encoder is continued to be used.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.obj
    }

    /// Consumes this decoder, returning the underlying reader.
    pub fn into_inner(self) -> R {
        self.obj
    }

    /// Returns the number of bytes that the decompressor has consumed.
    ///
    /// Note that this will likely be smaller than what the decompressor
    /// actually read from the underlying stream due to buffering.
    pub fn total_in(&self) -> u64 {
        self.data.total_in()
    }

    /// Returns the number of bytes that the decompressor has produced.
    pub fn total_out(&self) -> u64 {
        self.data.total_out()
    }
}

impl<R: BufRead> Read for ZlibDecoder<R> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        zio::read(&mut self.obj, &mut self.data, into)
    }
}

impl<R: BufRead + Write> Write for ZlibDecoder<R> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.get_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.get_mut().flush()
    }
}
