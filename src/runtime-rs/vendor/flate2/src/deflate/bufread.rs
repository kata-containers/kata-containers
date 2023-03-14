use std::io;
use std::io::prelude::*;
use std::mem;

use crate::zio;
use crate::{Compress, Decompress};

/// A DEFLATE encoder, or compressor.
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
/// use std::io;
/// use flate2::Compression;
/// use flate2::bufread::DeflateEncoder;
/// use std::fs::File;
/// use std::io::BufReader;
///
/// # fn main() {
/// #    println!("{:?}", open_hello_world().unwrap());
/// # }
/// #
/// // Opens sample file, compresses the contents and returns a Vector
/// fn open_hello_world() -> io::Result<Vec<u8>> {
///    let f = File::open("examples/hello_world.txt")?;
///    let b = BufReader::new(f);
///    let mut deflater = DeflateEncoder::new(b, Compression::fast());
///    let mut buffer = Vec::new();
///    deflater.read_to_end(&mut buffer)?;
///    Ok(buffer)
/// }
/// ```
#[derive(Debug)]
pub struct DeflateEncoder<R> {
    obj: R,
    data: Compress,
}

impl<R: BufRead> DeflateEncoder<R> {
    /// Creates a new encoder which will read uncompressed data from the given
    /// stream and emit the compressed stream.
    pub fn new(r: R, level: crate::Compression) -> DeflateEncoder<R> {
        DeflateEncoder {
            obj: r,
            data: Compress::new(level, false),
        }
    }
}

pub fn reset_encoder_data<R>(zlib: &mut DeflateEncoder<R>) {
    zlib.data.reset();
}

impl<R> DeflateEncoder<R> {
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

impl<R: BufRead> Read for DeflateEncoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        zio::read(&mut self.obj, &mut self.data, buf)
    }
}

impl<W: BufRead + Write> Write for DeflateEncoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.get_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.get_mut().flush()
    }
}

/// A DEFLATE decoder, or decompressor.
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
/// # use flate2::write::DeflateEncoder;
/// use flate2::bufread::DeflateDecoder;
///
/// # fn main() {
/// #    let mut e = DeflateEncoder::new(Vec::new(), Compression::default());
/// #    e.write_all(b"Hello World").unwrap();
/// #    let bytes = e.finish().unwrap();
/// #    println!("{}", decode_reader(bytes).unwrap());
/// # }
/// // Uncompresses a Deflate Encoded vector of bytes and returns a string or error
/// // Here &[u8] implements Read
/// fn decode_reader(bytes: Vec<u8>) -> io::Result<String> {
///    let mut deflater = DeflateDecoder::new(&bytes[..]);
///    let mut s = String::new();
///    deflater.read_to_string(&mut s)?;
///    Ok(s)
/// }
/// ```
#[derive(Debug)]
pub struct DeflateDecoder<R> {
    obj: R,
    data: Decompress,
}

pub fn reset_decoder_data<R>(zlib: &mut DeflateDecoder<R>) {
    zlib.data = Decompress::new(false);
}

impl<R: BufRead> DeflateDecoder<R> {
    /// Creates a new decoder which will decompress data read from the given
    /// stream.
    pub fn new(r: R) -> DeflateDecoder<R> {
        DeflateDecoder {
            obj: r,
            data: Decompress::new(false),
        }
    }
}

impl<R> DeflateDecoder<R> {
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

    /// Resets the state of this decoder's data
    ///
    /// This will reset the internal state of this decoder. It will continue
    /// reading from the same stream.
    pub fn reset_data(&mut self) {
        reset_decoder_data(self);
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

impl<R: BufRead> Read for DeflateDecoder<R> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        zio::read(&mut self.obj, &mut self.data, into)
    }
}

impl<W: BufRead + Write> Write for DeflateDecoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.get_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.get_mut().flush()
    }
}
