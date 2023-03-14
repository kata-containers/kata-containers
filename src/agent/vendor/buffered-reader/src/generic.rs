use std::io;
use std::fmt;
use std::cmp;
use std::io::{Error, ErrorKind};

use super::*;

/// Controls tracing.
const TRACE: bool = false;

/// Wraps a `Read`er.
///
/// This is useful when reading from a generic `std::io::Read`er.  To
/// read from a file, use [`File`].  To read from a buffer, use
/// [`Memory`].  Both are more efficient than `Generic`.
///
pub struct Generic<T: io::Read + Send + Sync, C: fmt::Debug + Sync + Send> {
    buffer: Option<Vec<u8>>,
    // The next byte to read in the buffer.
    cursor: usize,
    /// Currently unused buffer.
    unused_buffer: Option<Vec<u8>>,
    // The preferred chunk size.  This is just a hint.
    preferred_chunk_size: usize,
    // The wrapped reader.
    reader: T,
    // Stashed error, if any.
    error: Option<Error>,
    /// Whether we hit EOF on the underlying reader.
    eof: bool,

    // The user settable cookie.
    cookie: C,
}

assert_send_and_sync!(Generic<T, C>
                      where T: io::Read,
                            C: fmt::Debug);

impl<T: io::Read + Send + Sync, C: fmt::Debug + Sync + Send> fmt::Display for Generic<T, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Generic")
    }
}

impl<T: io::Read + Send + Sync, C: fmt::Debug + Sync + Send> fmt::Debug for Generic<T, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let buffered_data = if let Some(ref buffer) = self.buffer {
            buffer.len() - self.cursor
        } else {
            0
        };

        f.debug_struct("Generic")
            .field("preferred_chunk_size", &self.preferred_chunk_size)
            .field("buffer data", &buffered_data)
            .finish()
    }
}

impl<T: io::Read + Send + Sync> Generic<T, ()> {
    /// Instantiate a new generic reader.  `reader` is the source to
    /// wrap.  `preferred_chuck_size` is the preferred chuck size.  If
    /// None, then the default will be used, which is usually what you
    /// want.
    pub fn new(reader: T, preferred_chunk_size: Option<usize>) -> Self {
        Self::with_cookie(reader, preferred_chunk_size, ())
    }
}

impl<T: io::Read + Send + Sync, C: fmt::Debug + Sync + Send> Generic<T, C> {
    /// Like `new()`, but sets a cookie, which can be retrieved using
    /// the `cookie_ref` and `cookie_mut` methods, and set using
    /// the `cookie_set` method.
    pub fn with_cookie(
           reader: T, preferred_chunk_size: Option<usize>, cookie: C)
           -> Self {
        Generic {
            buffer: None,
            cursor: 0,
            unused_buffer: None,
            preferred_chunk_size:
                if let Some(s) = preferred_chunk_size { s }
                else { DEFAULT_BUF_SIZE },
            reader,
            error: None,
            eof: false,
            cookie,
        }
    }

    /// Returns a reference to the wrapped writer.
    pub fn reader_ref(&self) -> &T {
        &self.reader
    }

    /// Returns a mutable reference to the wrapped writer.
    pub fn reader_mut(&mut self) -> &mut T {
        &mut self.reader
    }

    /// Returns the wrapped writer.
    pub fn into_reader(self) -> T {
        self.reader
    }

    /// Return the buffer.  Ensure that it contains at least `amount`
    /// bytes.
    //
    // Note:
    //
    // If you find a bug in this function, consider whether
    // sequoia_openpgp::armor::Reader::data_helper is also affected.
    fn data_helper(&mut self, amount: usize, hard: bool, and_consume: bool)
                   -> Result<&[u8], io::Error> {
        tracer!(TRACE, "Generic::data_helper");
        t!("amount: {}, hard: {}, and_consume: {} (cursor: {}, buffer: {:?})",
           amount, hard, and_consume,
           self.cursor,
           self.buffer.as_ref().map(|buffer| buffer.len()));

        // See if there is an error from the last invocation.
        if let Some(e) = self.error.take() {
            t!("Returning stashed error: {}", e);
            return Err(e);
        }

        if let Some(ref buffer) = self.buffer {
            // We have a buffer.  Make sure `cursor` is sane.
            assert!(self.cursor <= buffer.len());
        } else {
            // We don't have a buffer.  Make sure cursor is 0.
            assert_eq!(self.cursor, 0);
        }

        let amount_buffered
            = self.buffer.as_ref().map(|b| b.len() - self.cursor).unwrap_or(0);
        if amount > amount_buffered {
            // The caller wants more data than we have readily
            // available.  Read some more.

            let capacity : usize = cmp::max(cmp::max(
                DEFAULT_BUF_SIZE,
                2 * self.preferred_chunk_size), amount);

            let mut buffer_new = self.unused_buffer.take()
                .map(|mut v| {
                    vec_resize(&mut v, capacity);
                    v
                })
                .unwrap_or_else(|| vec![0u8; capacity]);

            let mut amount_read = 0;
            while amount_buffered + amount_read < amount {
                t!("Have {} bytes, need {} bytes",
                   amount_buffered + amount_read, amount);

                if self.eof {
                    t!("Hit EOF on the underlying reader, don't poll again.");
                    break;
                }

                match self.reader.read(&mut buffer_new
                                       [amount_buffered + amount_read..]) {
                    Ok(read) => {
                        t!("Read {} bytes", read);
                        if read == 0 {
                            self.eof = true;
                            break;
                        } else {
                            amount_read += read;
                            continue;
                        }
                    },
                    Err(ref err) if err.kind() == ErrorKind::Interrupted =>
                        continue,
                    Err(err) => {
                        // Don't return yet, because we may have
                        // actually read something.
                        self.error = Some(err);
                        break;
                    },
                }
            }

            if amount_read > 0 {
                // We read something.
                if let Some(ref buffer) = self.buffer {
                    // We need to copy in the old data.
                    buffer_new[0..amount_buffered]
                        .copy_from_slice(
                            &buffer[self.cursor..self.cursor + amount_buffered]);
                }

                vec_truncate(&mut buffer_new, amount_buffered + amount_read);

                self.unused_buffer = self.buffer.take();
                self.buffer = Some(buffer_new);
                self.cursor = 0;
            }
        }

        let amount_buffered
            = self.buffer.as_ref().map(|b| b.len() - self.cursor).unwrap_or(0);

        if self.error.is_some() {
            t!("Encountered an error: {}", self.error.as_ref().unwrap());
            // An error occurred.  If we have enough data to fulfill
            // the caller's request, then don't return the error.
            if hard && amount > amount_buffered {
                t!("Not enough data to fulfill request, returning error");
                return Err(self.error.take().unwrap());
            }
            if !hard && amount_buffered == 0 {
                t!("No data data buffered, returning error");
                return Err(self.error.take().unwrap());
            }
        }

        if hard && amount_buffered < amount {
            t!("Unexpected EOF");
            Err(Error::new(ErrorKind::UnexpectedEof, "EOF"))
        } else if amount == 0 || amount_buffered == 0 {
            t!("Returning zero-length slice");
            Ok(&b""[..])
        } else {
            let buffer = self.buffer.as_ref().unwrap();
            if and_consume {
                let amount_consumed = cmp::min(amount_buffered, amount);
                self.cursor += amount_consumed;
                assert!(self.cursor <= buffer.len());
                t!("Consuming {} bytes, returning {} bytes",
                   amount_consumed,
                   buffer[self.cursor-amount_consumed..].len());
                Ok(&buffer[self.cursor-amount_consumed..])
            } else {
                t!("Returning {} bytes",
                   buffer[self.cursor..].len());
                Ok(&buffer[self.cursor..])
            }
        }
    }
}

impl<T: io::Read + Send + Sync, C: fmt::Debug + Sync + Send> io::Read for Generic<T, C> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        buffered_reader_generic_read_impl(self, buf)
    }
}

impl<T: io::Read + Send + Sync, C: fmt::Debug + Sync + Send> BufferedReader<C> for Generic<T, C> {
    fn buffer(&self) -> &[u8] {
        if let Some(ref buffer) = self.buffer {
            &buffer[self.cursor..]
        } else {
            &b""[..]
        }
    }

    fn data(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        self.data_helper(amount, false, false)
    }

    fn data_hard(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        self.data_helper(amount, true, false)
    }

    // Note:
    //
    // If you find a bug in this function, consider whether
    // sequoia_openpgp::armor::Reader::consume is also affected.
    fn consume(&mut self, amount: usize) -> &[u8] {
        // println!("Generic.consume({}) \
        //           (cursor: {}, buffer: {:?})",
        //          amount, self.cursor,
        //          if let Some(ref buffer) = self.buffer { Some(buffer.len()) }
        //          else { None });

        // The caller can't consume more than is buffered!
        if let Some(ref buffer) = self.buffer {
            assert!(self.cursor <= buffer.len());
            assert!(amount <= buffer.len() - self.cursor,
                    "buffer contains just {} bytes, but you are trying to \
                    consume {} bytes.  Did you forget to call data()?",
                    buffer.len() - self.cursor, amount);

            self.cursor += amount;
            return &self.buffer.as_ref().unwrap()[self.cursor - amount..];
        } else {
            assert_eq!(amount, 0);
            &b""[..]
        }
    }

    fn data_consume(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        self.data_helper(amount, false, true)
    }

    fn data_consume_hard(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        self.data_helper(amount, true, true)
    }

    fn get_mut(&mut self) -> Option<&mut dyn BufferedReader<C>> {
        None
    }

    fn get_ref(&self) -> Option<&dyn BufferedReader<C>> {
        None
    }

    fn into_inner<'b>(self: Box<Self>) -> Option<Box<dyn BufferedReader<C> + 'b>>
        where Self: 'b {
        None
    }

    fn cookie_set(&mut self, cookie: C) -> C {
        use std::mem;

        mem::replace(&mut self.cookie, cookie)
    }

    fn cookie_ref(&self) -> &C {
        &self.cookie
    }

    fn cookie_mut(&mut self) -> &mut C {
        &mut self.cookie
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn buffered_reader_generic_test() {
        // Test reading from a file.
        {
            use std::path::PathBuf;
            use std::fs::File;

            let path : PathBuf = [env!("CARGO_MANIFEST_DIR"),
                                  "src", "buffered-reader-test.txt"]
                .iter().collect();
            let mut f = File::open(&path).expect(&path.to_string_lossy());
            let mut bio = Generic::new(&mut f, None);

            buffered_reader_test_data_check(&mut bio);
        }

        // Same test, but as a slice.
        {
            let mut data : &[u8] = include_bytes!("buffered-reader-test.txt");
            let mut bio = Generic::new(&mut data, None);

            buffered_reader_test_data_check(&mut bio);
        }
    }

    // Test that buffer() returns the same data as data().
    #[test]
    fn buffer_test() {
        // Test vector.
        let size = 10 * DEFAULT_BUF_SIZE;
        let mut input = Vec::with_capacity(size);
        let mut v = 0u8;
        for _ in 0..size {
            input.push(v);
            if v == std::u8::MAX {
                v = 0;
            } else {
                v += 1;
            }
        }

        let mut reader = Generic::new(&input[..], None);

        // Gather some stats to make it easier to figure out whether
        // this test is working.
        let stats_count =  2 * DEFAULT_BUF_SIZE;
        let mut stats = vec![0usize; stats_count];

        for i in 0..input.len() {
            let data = reader.data(DEFAULT_BUF_SIZE + 1).unwrap().to_vec();
            assert!(!data.is_empty());
            assert_eq!(data, reader.buffer());
            // And, we may as well check to make sure we read the
            // right data.
            assert_eq!(data, &input[i..i+data.len()]);

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
