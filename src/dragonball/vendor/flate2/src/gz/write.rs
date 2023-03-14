use std::cmp;
use std::io;
use std::io::prelude::*;

use super::bufread::{corrupt, read_gz_header};
use super::{GzBuilder, GzHeader};
use crate::crc::{Crc, CrcWriter};
use crate::zio;
use crate::{Compress, Compression, Decompress, Status};

/// A gzip streaming encoder
///
/// This structure exposes a [`Write`] interface that will emit compressed data
/// to the underlying writer `W`.
///
/// [`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
///
/// # Examples
///
/// ```
/// use std::io::prelude::*;
/// use flate2::Compression;
/// use flate2::write::GzEncoder;
///
/// // Vec<u8> implements Write to print the compressed bytes of sample string
/// # fn main() {
///
/// let mut e = GzEncoder::new(Vec::new(), Compression::default());
/// e.write_all(b"Hello World").unwrap();
/// println!("{:?}", e.finish().unwrap());
/// # }
/// ```
#[derive(Debug)]
pub struct GzEncoder<W: Write> {
    inner: zio::Writer<W, Compress>,
    crc: Crc,
    crc_bytes_written: usize,
    header: Vec<u8>,
}

pub fn gz_encoder<W: Write>(header: Vec<u8>, w: W, lvl: Compression) -> GzEncoder<W> {
    GzEncoder {
        inner: zio::Writer::new(w, Compress::new(lvl, false)),
        crc: Crc::new(),
        header,
        crc_bytes_written: 0,
    }
}

impl<W: Write> GzEncoder<W> {
    /// Creates a new encoder which will use the given compression level.
    ///
    /// The encoder is not configured specially for the emitted header. For
    /// header configuration, see the `GzBuilder` type.
    ///
    /// The data written to the returned encoder will be compressed and then
    /// written to the stream `w`.
    pub fn new(w: W, level: Compression) -> GzEncoder<W> {
        GzBuilder::new().write(w, level)
    }

    /// Acquires a reference to the underlying writer.
    pub fn get_ref(&self) -> &W {
        self.inner.get_ref()
    }

    /// Acquires a mutable reference to the underlying writer.
    ///
    /// Note that mutation of the writer may result in surprising results if
    /// this encoder is continued to be used.
    pub fn get_mut(&mut self) -> &mut W {
        self.inner.get_mut()
    }

    /// Attempt to finish this output stream, writing out final chunks of data.
    ///
    /// Note that this function can only be used once data has finished being
    /// written to the output stream. After this function is called then further
    /// calls to `write` may result in a panic.
    ///
    /// # Panics
    ///
    /// Attempts to write data to this stream may result in a panic after this
    /// function is called.
    ///
    /// # Errors
    ///
    /// This function will perform I/O to complete this stream, and any I/O
    /// errors which occur will be returned from this function.
    pub fn try_finish(&mut self) -> io::Result<()> {
        self.write_header()?;
        self.inner.finish()?;

        while self.crc_bytes_written < 8 {
            let (sum, amt) = (self.crc.sum() as u32, self.crc.amount());
            let buf = [
                (sum >> 0) as u8,
                (sum >> 8) as u8,
                (sum >> 16) as u8,
                (sum >> 24) as u8,
                (amt >> 0) as u8,
                (amt >> 8) as u8,
                (amt >> 16) as u8,
                (amt >> 24) as u8,
            ];
            let inner = self.inner.get_mut();
            let n = inner.write(&buf[self.crc_bytes_written..])?;
            self.crc_bytes_written += n;
        }
        Ok(())
    }

    /// Finish encoding this stream, returning the underlying writer once the
    /// encoding is done.
    ///
    /// Note that this function may not be suitable to call in a situation where
    /// the underlying stream is an asynchronous I/O stream. To finish a stream
    /// the `try_finish` (or `shutdown`) method should be used instead. To
    /// re-acquire ownership of a stream it is safe to call this method after
    /// `try_finish` or `shutdown` has returned `Ok`.
    ///
    /// # Errors
    ///
    /// This function will perform I/O to complete this stream, and any I/O
    /// errors which occur will be returned from this function.
    pub fn finish(mut self) -> io::Result<W> {
        self.try_finish()?;
        Ok(self.inner.take_inner())
    }

    fn write_header(&mut self) -> io::Result<()> {
        while !self.header.is_empty() {
            let n = self.inner.get_mut().write(&self.header)?;
            self.header.drain(..n);
        }
        Ok(())
    }
}

impl<W: Write> Write for GzEncoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        assert_eq!(self.crc_bytes_written, 0);
        self.write_header()?;
        let n = self.inner.write(buf)?;
        self.crc.update(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        assert_eq!(self.crc_bytes_written, 0);
        self.write_header()?;
        self.inner.flush()
    }
}

impl<R: Read + Write> Read for GzEncoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.get_mut().read(buf)
    }
}

impl<W: Write> Drop for GzEncoder<W> {
    fn drop(&mut self) {
        if self.inner.is_present() {
            let _ = self.try_finish();
        }
    }
}

/// A gzip streaming decoder
///
/// This structure exposes a [`Write`] interface that will emit compressed data
/// to the underlying writer `W`.
///
/// [`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
///
/// # Examples
///
/// ```
/// use std::io::prelude::*;
/// use std::io;
/// use flate2::Compression;
/// use flate2::write::{GzEncoder, GzDecoder};
///
/// # fn main() {
/// #    let mut e = GzEncoder::new(Vec::new(), Compression::default());
/// #    e.write(b"Hello World").unwrap();
/// #    let bytes = e.finish().unwrap();
/// #    assert_eq!("Hello World", decode_writer(bytes).unwrap());
/// # }
/// // Uncompresses a gzip encoded vector of bytes and returns a string or error
/// // Here Vec<u8> implements Write
/// fn decode_writer(bytes: Vec<u8>) -> io::Result<String> {
///    let mut writer = Vec::new();
///    let mut decoder = GzDecoder::new(writer);
///    decoder.write_all(&bytes[..])?;
///    writer = decoder.finish()?;
///    let return_string = String::from_utf8(writer).expect("String parsing error");
///    Ok(return_string)
/// }
/// ```
#[derive(Debug)]
pub struct GzDecoder<W: Write> {
    inner: zio::Writer<CrcWriter<W>, Decompress>,
    crc_bytes: Vec<u8>,
    header: Option<GzHeader>,
    header_buf: Vec<u8>,
}

const CRC_BYTES_LEN: usize = 8;

impl<W: Write> GzDecoder<W> {
    /// Creates a new decoder which will write uncompressed data to the stream.
    ///
    /// When this encoder is dropped or unwrapped the final pieces of data will
    /// be flushed.
    pub fn new(w: W) -> GzDecoder<W> {
        GzDecoder {
            inner: zio::Writer::new(CrcWriter::new(w), Decompress::new(false)),
            crc_bytes: Vec::with_capacity(CRC_BYTES_LEN),
            header: None,
            header_buf: Vec::new(),
        }
    }

    /// Returns the header associated with this stream.
    pub fn header(&self) -> Option<&GzHeader> {
        self.header.as_ref()
    }

    /// Acquires a reference to the underlying writer.
    pub fn get_ref(&self) -> &W {
        self.inner.get_ref().get_ref()
    }

    /// Acquires a mutable reference to the underlying writer.
    ///
    /// Note that mutating the output/input state of the stream may corrupt this
    /// object, so care must be taken when using this method.
    pub fn get_mut(&mut self) -> &mut W {
        self.inner.get_mut().get_mut()
    }

    /// Attempt to finish this output stream, writing out final chunks of data.
    ///
    /// Note that this function can only be used once data has finished being
    /// written to the output stream. After this function is called then further
    /// calls to `write` may result in a panic.
    ///
    /// # Panics
    ///
    /// Attempts to write data to this stream may result in a panic after this
    /// function is called.
    ///
    /// # Errors
    ///
    /// This function will perform I/O to finish the stream, returning any
    /// errors which happen.
    pub fn try_finish(&mut self) -> io::Result<()> {
        self.finish_and_check_crc()?;
        Ok(())
    }

    /// Consumes this decoder, flushing the output stream.
    ///
    /// This will flush the underlying data stream and then return the contained
    /// writer if the flush succeeded.
    ///
    /// Note that this function may not be suitable to call in a situation where
    /// the underlying stream is an asynchronous I/O stream. To finish a stream
    /// the `try_finish` (or `shutdown`) method should be used instead. To
    /// re-acquire ownership of a stream it is safe to call this method after
    /// `try_finish` or `shutdown` has returned `Ok`.
    ///
    /// # Errors
    ///
    /// This function will perform I/O to complete this stream, and any I/O
    /// errors which occur will be returned from this function.
    pub fn finish(mut self) -> io::Result<W> {
        self.finish_and_check_crc()?;
        Ok(self.inner.take_inner().into_inner())
    }

    fn finish_and_check_crc(&mut self) -> io::Result<()> {
        self.inner.finish()?;

        if self.crc_bytes.len() != 8 {
            return Err(corrupt());
        }

        let crc = ((self.crc_bytes[0] as u32) << 0)
            | ((self.crc_bytes[1] as u32) << 8)
            | ((self.crc_bytes[2] as u32) << 16)
            | ((self.crc_bytes[3] as u32) << 24);
        let amt = ((self.crc_bytes[4] as u32) << 0)
            | ((self.crc_bytes[5] as u32) << 8)
            | ((self.crc_bytes[6] as u32) << 16)
            | ((self.crc_bytes[7] as u32) << 24);
        if crc != self.inner.get_ref().crc().sum() as u32 {
            return Err(corrupt());
        }
        if amt != self.inner.get_ref().crc().amount() {
            return Err(corrupt());
        }
        Ok(())
    }
}

struct Counter<T: Read> {
    inner: T,
    pos: usize,
}

impl<T: Read> Read for Counter<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let pos = self.inner.read(buf)?;
        self.pos += pos;
        Ok(pos)
    }
}

impl<W: Write> Write for GzDecoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.header.is_none() {
            // trying to avoid buffer usage
            let (res, pos) = {
                let mut counter = Counter {
                    inner: self.header_buf.chain(buf),
                    pos: 0,
                };
                let res = read_gz_header(&mut counter);
                (res, counter.pos)
            };

            match res {
                Err(err) => {
                    if err.kind() == io::ErrorKind::UnexpectedEof {
                        // not enough data for header, save to the buffer
                        self.header_buf.extend(buf);
                        Ok(buf.len())
                    } else {
                        Err(err)
                    }
                }
                Ok(header) => {
                    self.header = Some(header);
                    let pos = pos - self.header_buf.len();
                    self.header_buf.truncate(0);
                    Ok(pos)
                }
            }
        } else {
            let (n, status) = self.inner.write_with_status(buf)?;

            if status == Status::StreamEnd && n < buf.len() && self.crc_bytes.len() < 8 {
                let remaining = buf.len() - n;
                let crc_bytes = cmp::min(remaining, CRC_BYTES_LEN - self.crc_bytes.len());
                self.crc_bytes.extend(&buf[n..n + crc_bytes]);
                return Ok(n + crc_bytes);
            }
            Ok(n)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<W: Read + Write> Read for GzDecoder<W> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.get_mut().get_mut().read(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STR: &'static str = "Hello World Hello World Hello World Hello World Hello World \
                               Hello World Hello World Hello World Hello World Hello World \
                               Hello World Hello World Hello World Hello World Hello World \
                               Hello World Hello World Hello World Hello World Hello World \
                               Hello World Hello World Hello World Hello World Hello World";

    #[test]
    fn decode_writer_one_chunk() {
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write(STR.as_ref()).unwrap();
        let bytes = e.finish().unwrap();

        let mut writer = Vec::new();
        let mut decoder = GzDecoder::new(writer);
        let n = decoder.write(&bytes[..]).unwrap();
        decoder.write(&bytes[n..]).unwrap();
        decoder.try_finish().unwrap();
        writer = decoder.finish().unwrap();
        let return_string = String::from_utf8(writer).expect("String parsing error");
        assert_eq!(return_string, STR);
    }

    #[test]
    fn decode_writer_partial_header() {
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write(STR.as_ref()).unwrap();
        let bytes = e.finish().unwrap();

        let mut writer = Vec::new();
        let mut decoder = GzDecoder::new(writer);
        assert_eq!(decoder.write(&bytes[..5]).unwrap(), 5);
        let n = decoder.write(&bytes[5..]).unwrap();
        if n < bytes.len() - 5 {
            decoder.write(&bytes[n + 5..]).unwrap();
        }
        writer = decoder.finish().unwrap();
        let return_string = String::from_utf8(writer).expect("String parsing error");
        assert_eq!(return_string, STR);
    }

    #[test]
    fn decode_writer_exact_header() {
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write(STR.as_ref()).unwrap();
        let bytes = e.finish().unwrap();

        let mut writer = Vec::new();
        let mut decoder = GzDecoder::new(writer);
        assert_eq!(decoder.write(&bytes[..10]).unwrap(), 10);
        decoder.write(&bytes[10..]).unwrap();
        writer = decoder.finish().unwrap();
        let return_string = String::from_utf8(writer).expect("String parsing error");
        assert_eq!(return_string, STR);
    }

    #[test]
    fn decode_writer_partial_crc() {
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write(STR.as_ref()).unwrap();
        let bytes = e.finish().unwrap();

        let mut writer = Vec::new();
        let mut decoder = GzDecoder::new(writer);
        let l = bytes.len() - 5;
        let n = decoder.write(&bytes[..l]).unwrap();
        decoder.write(&bytes[n..]).unwrap();
        writer = decoder.finish().unwrap();
        let return_string = String::from_utf8(writer).expect("String parsing error");
        assert_eq!(return_string, STR);
    }
}
