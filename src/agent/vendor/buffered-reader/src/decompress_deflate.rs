use std::io;
use std::fmt;

use flate2::read::DeflateDecoder;
use flate2::read::ZlibDecoder;

use super::*;

/// Decompresses the underlying `BufferedReader` using the deflate
/// algorithm.
#[derive(Debug)]
pub struct Deflate<R: BufferedReader<C>, C: fmt::Debug + Sync + Send> {
    reader: Generic<DeflateDecoder<R>, C>,
}

assert_send_and_sync!(Deflate<R, C>
                      where R: BufferedReader<C>,
                            C: fmt::Debug);

impl <R: BufferedReader<()>> Deflate<R, ()> {
    /// Instantiates a new deflate decompression reader.
    ///
    /// `reader` is the source to wrap.
    pub fn new(reader: R) -> Self {
        Self::with_cookie(reader, ())
    }
}

impl <R: BufferedReader<C>, C: fmt::Debug + Sync + Send> Deflate<R, C> {
    /// Like `new()`, but uses a cookie.
    ///
    /// The cookie can be retrieved using the `cookie_ref` and
    /// `cookie_mut` methods, and set using the `cookie_set` method.
    pub fn with_cookie(reader: R, cookie: C) -> Self {
        Deflate {
            reader: Generic::with_cookie(
                DeflateDecoder::new(reader), None, cookie),
        }
    }
}

impl<R: BufferedReader<C>, C: fmt::Debug + Sync + Send> io::Read for Deflate<R, C> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        self.reader.read(buf)
    }
}

impl<R: BufferedReader<C>, C: fmt::Debug + Sync + Send> fmt::Display for Deflate<R, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Deflate").finish()
    }
}

impl<R: BufferedReader<C>, C: fmt::Debug + Send + Sync> BufferedReader<C>
        for Deflate<R, C> {
    fn buffer(&self) -> &[u8] {
        self.reader.buffer()
    }

    fn data(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        self.reader.data(amount)
    }

    fn data_hard(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        self.reader.data_hard(amount)
    }

    fn data_eof(&mut self) -> Result<&[u8], io::Error> {
        self.reader.data_eof()
    }

    fn consume(&mut self, amount: usize) -> &[u8] {
        self.reader.consume(amount)
    }

    fn data_consume(&mut self, amount: usize)
                    -> Result<&[u8], io::Error> {
        self.reader.data_consume(amount)
    }

    fn data_consume_hard(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        self.reader.data_consume_hard(amount)
    }

    fn read_be_u16(&mut self) -> Result<u16, io::Error> {
        self.reader.read_be_u16()
    }

    fn read_be_u32(&mut self) -> Result<u32, io::Error> {
        self.reader.read_be_u32()
    }

    fn steal(&mut self, amount: usize) -> Result<Vec<u8>, io::Error> {
        self.reader.steal(amount)
    }

    fn steal_eof(&mut self) -> Result<Vec<u8>, io::Error> {
        self.reader.steal_eof()
    }

    fn get_mut(&mut self) -> Option<&mut dyn BufferedReader<C>> {
        Some(self.reader.reader_mut().get_mut())
    }

    fn get_ref(&self) -> Option<&dyn BufferedReader<C>> {
        Some(self.reader.reader_ref().get_ref())
    }

    fn into_inner<'b>(self: Box<Self>)
            -> Option<Box<dyn BufferedReader<C> + 'b>> where Self: 'b {
        // Strip the outer box.
        Some(self.reader.into_reader().into_inner().as_boxed())
    }

    fn cookie_set(&mut self, cookie: C) -> C {
        self.reader.cookie_set(cookie)
    }

    fn cookie_ref(&self) -> &C {
        self.reader.cookie_ref()
    }

    fn cookie_mut(&mut self) -> &mut C {
        self.reader.cookie_mut()
    }
}

/// Decompresses the underlying `BufferedReader` using the zlib
/// algorithm.
pub struct Zlib<R: BufferedReader<C>, C: fmt::Debug + Sync + Send> {
    reader: Generic<ZlibDecoder<R>, C>,
}

assert_send_and_sync!(Zlib<R, C>
                      where R: BufferedReader<C>,
                            C: fmt::Debug);

impl <R: BufferedReader<()>> Zlib<R, ()> {
    /// Instantiates a new zlib decompression reader.
    ///
    /// `reader` is the source to wrap.
    pub fn new(reader: R) -> Self {
        Self::with_cookie(reader, ())
    }
}

impl <R: BufferedReader<C>, C: fmt::Debug + Sync + Send> Zlib<R, C> {
    /// Like `new()`, but uses a cookie.
    ///
    /// The cookie can be retrieved using the `cookie_ref` and
    /// `cookie_mut` methods, and set using the `cookie_set` method.
    pub fn with_cookie(reader: R, cookie: C) -> Self {
        Zlib {
            reader: Generic::with_cookie(
                ZlibDecoder::new(reader), None, cookie),
        }
    }
}

impl<R: BufferedReader<C>, C: fmt::Debug + Sync + Send> io::Read for Zlib<R, C> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        self.reader.read(buf)
    }
}

impl<R: BufferedReader<C>, C: fmt::Debug + Sync + Send> fmt::Display for Zlib<R, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Zlib")
    }
}

impl<R: BufferedReader<C>, C: fmt::Debug + Sync + Send> fmt::Debug for Zlib<R, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Zlib")
            .field("reader", &self.get_ref().unwrap())
            .finish()
    }
}

impl<R: BufferedReader<C>, C: fmt::Debug + Send + Sync> BufferedReader<C>
        for Zlib<R, C> {
    fn buffer(&self) -> &[u8] {
        self.reader.buffer()
    }

    fn data(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        self.reader.data(amount)
    }

    fn data_hard(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        self.reader.data_hard(amount)
    }

    fn data_eof(&mut self) -> Result<&[u8], io::Error> {
        self.reader.data_eof()
    }

    fn consume(&mut self, amount: usize) -> &[u8] {
        self.reader.consume(amount)
    }

    fn data_consume(&mut self, amount: usize)
                    -> Result<&[u8], io::Error> {
        self.reader.data_consume(amount)
    }

    fn data_consume_hard(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        self.reader.data_consume_hard(amount)
    }

    fn read_be_u16(&mut self) -> Result<u16, io::Error> {
        self.reader.read_be_u16()
    }

    fn read_be_u32(&mut self) -> Result<u32, io::Error> {
        self.reader.read_be_u32()
    }

    fn steal(&mut self, amount: usize) -> Result<Vec<u8>, io::Error> {
        self.reader.steal(amount)
    }

    fn steal_eof(&mut self) -> Result<Vec<u8>, io::Error> {
        self.reader.steal_eof()
    }

    fn get_mut(&mut self) -> Option<&mut dyn BufferedReader<C>> {
        Some(self.reader.reader_mut().get_mut())
    }

    fn get_ref(&self) -> Option<&dyn BufferedReader<C>> {
        Some(self.reader.reader_ref().get_ref())
    }

    fn into_inner<'b>(self: Box<Self>)
            -> Option<Box<dyn BufferedReader<C> + 'b>> where Self: 'b {
        // Strip the outer box.
        Some(self.reader.into_reader().into_inner().as_boxed())
    }

    fn cookie_set(&mut self, cookie: C) -> C {
        self.reader.cookie_set(cookie)
    }

    fn cookie_ref(&self) -> &C {
        self.reader.cookie_ref()
    }

    fn cookie_mut(&mut self) -> &mut C {
        self.reader.cookie_mut()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // Test that buffer() returns the same data as data().
    #[test]
    fn buffer_test() {
        use flate2::write::DeflateEncoder;
        use flate2::Compression;
        use std::io::prelude::*;

        // Test vector.
        let size = 10 * DEFAULT_BUF_SIZE;
        let mut input_raw = Vec::with_capacity(size);
        let mut v = 0u8;
        for _ in 0..size {
            input_raw.push(v);
            if v == std::u8::MAX {
                v = 0;
            } else {
                v += 1;
            }
        }

        // Compress the raw input.
        let mut input = Vec::new();
        {
            let mut encoder =
                DeflateEncoder::new(&mut input, Compression::default());
            encoder.write(&input_raw[..]).unwrap();
            encoder.try_finish().unwrap();
        }

        let mut reader = Deflate::new(
            Generic::new(&input[..], None));

        // Gather some stats to make it easier to figure out whether
        // this test is working.
        let stats_count =  2 * DEFAULT_BUF_SIZE;
        let mut stats = vec![0usize; stats_count];

        for i in 0..input_raw.len() {
            let data = reader.data(DEFAULT_BUF_SIZE + 1).unwrap().to_vec();
            assert!(!data.is_empty());
            assert_eq!(data, reader.buffer());
            // And, we may as well check to make sure we read the
            // right data.
            assert_eq!(data, &input_raw[i..i+data.len()]);

            stats[cmp::min(data.len(), stats_count - 1)] += 1;

            // Consume one byte and see what happens.
            reader.consume(1);
        }

        if false {
            for i in 0..stats.len() {
                if stats[i] > 0 {
                    if i == stats.len() - 1 {
                        eprint!(">=");
                    }
                    eprintln!("{}: {}", i, stats[i]);
                }
            }
        }
    }
}
