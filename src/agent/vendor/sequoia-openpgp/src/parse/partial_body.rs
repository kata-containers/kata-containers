
use std::cmp;
use std::io;
use std::io::{Error, ErrorKind};

use buffered_reader::{buffered_reader_generic_read_impl, BufferedReader};

use crate::{vec_resize, vec_truncate};
use crate::packet::header::BodyLength;
use crate::parse::{Cookie, Hashing};

const TRACE : bool = false;


/// A `BufferedReader` that transparently handles OpenPGP's chunking
/// scheme.  This implicitly implements a limitor.
pub(crate) struct BufferedReaderPartialBodyFilter<T: BufferedReader<Cookie>> {
    // The underlying reader.
    reader: T,

    // The amount of unread data in the current partial body chunk.
    // That is, if `buffer` contains 10 bytes and
    // `partial_body_length` is 20, then there are 30 bytes of
    // unprocessed (unconsumed) data in the current chunk.
    partial_body_length: u32,
    // Whether this is the last partial body chuck.
    last: bool,

    // Sometimes we have to double buffer.  This happens if the caller
    // requests X bytes and that chunk straddles a partial body length
    // boundary.
    buffer: Option<Vec<u8>>,
    // The position within the buffer.
    cursor: usize,
    /// Currently unused buffers.
    unused_buffers: Vec<Vec<u8>>,

    // The user-defined cookie.
    cookie: Cookie,

    // Whether to include the headers in any hash directly over the
    // current packet.  If not, calls Cookie::hashing at
    // the current level to disable hashing while reading headers.
    hash_headers: bool,
}

impl<T: BufferedReader<Cookie>> std::fmt::Display
        for BufferedReaderPartialBodyFilter<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "BufferedReaderPartialBodyFilter")
    }
}

impl<T: BufferedReader<Cookie>> std::fmt::Debug
        for BufferedReaderPartialBodyFilter<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("BufferedReaderPartialBodyFilter")
            .field("partial_body_length", &self.partial_body_length)
            .field("last", &self.last)
            .field("hash headers", &self.hash_headers)
            .field("buffer (bytes left)",
                   &self.buffer.as_ref().map(|buffer| buffer.len()))
            .field("reader", &self.reader)
            .finish()
    }
}

impl<T: BufferedReader<Cookie>> BufferedReaderPartialBodyFilter<T> {
    /// Create a new BufferedReaderPartialBodyFilter object.
    /// `partial_body_length` is the amount of data in the initial
    /// partial body chunk.
    pub fn with_cookie(reader: T, partial_body_length: u32,
                       hash_headers: bool, cookie: Cookie) -> Self {
        BufferedReaderPartialBodyFilter {
            reader,
            partial_body_length,
            last: false,
            buffer: None,
            cursor: 0,
            unused_buffers: Vec::with_capacity(2),
            cookie,
            hash_headers,
        }
    }

    // Make sure that the local buffer contains `amount` bytes.
    fn do_fill_buffer (&mut self, amount: usize) -> Result<(), std::io::Error> {
        tracer!(TRACE, "PBF::do_fill_buffer", self.cookie.level.unwrap_or(0));
        t!("BufferedReaderPartialBodyFilter::do_fill_buffer(\
            amount: {}) (partial body length: {}, last: {})",
           amount, self.partial_body_length, self.last);

        if self.last && self.partial_body_length == 0 {
            // We reached the end.  Avoid fruitlessly copying data
            // over and over again trying to buffer more data.
            return Ok(());
        }

        // We want to avoid double buffering as much as possible.
        // Thus, we only buffer as much as needed.
        let mut buffer = self.unused_buffers.pop()
            .map(|mut v| {
                vec_resize(&mut v, amount);
                v
            })
            .unwrap_or_else(|| vec![0u8; amount]);
        let mut amount_buffered = 0;

        if let Some(ref old_buffer) = self.buffer {
            // The amount of data that is left in the old buffer.
            let amount_left = old_buffer.len() - self.cursor;

            // This function should only be called if we actually need
            // to read something.
            assert!(amount > amount_left);

            amount_buffered = amount_left;

            // Copy the data that is still in buffer.
            buffer[..amount_buffered]
                .copy_from_slice(&old_buffer[self.cursor..]);

            t!("Copied {} bytes from the old buffer", amount_buffered);
        }

        let mut err = None;

        loop {
            let to_read = cmp::min(
                // Data in current chunk.
                self.partial_body_length as usize,
                // Space left in the buffer.
                buffer.len() - amount_buffered);
            t!("Trying to buffer {} bytes \
                (partial body length: {}; space: {})",
               to_read, self.partial_body_length,
               buffer.len() - amount_buffered);
            if to_read > 0 {
                let result = self.reader.read(
                    &mut buffer[amount_buffered..amount_buffered + to_read]);
                match result {
                    Ok(did_read) => {
                        t!("Buffered {} bytes", did_read);
                        amount_buffered += did_read;
                        self.partial_body_length -= did_read as u32;

                        if did_read < to_read {
                            // Short read => EOF.  We're done.
                            // (Although the underlying message is
                            // probably corrupt.)
                            break;
                        }
                    },
                    Err(e) => {
                        t!("Err reading: {:?}", e);
                        err = Some(e);
                        break;
                    },
                }
            }

            if amount_buffered == amount || self.last {
                // We're read enough or we've read everything.
                break;
            }

            // Read the next partial body length header.
            assert_eq!(self.partial_body_length, 0);

            // Disable hashing, if necessary.
            if ! self.hash_headers {
                if let Some(level) = self.reader.cookie_ref().level {
                    Cookie::hashing(
                        &mut self.reader, Hashing::Disabled, level);
                }
            }

            t!("Reading next chunk's header (hashing: {}, level: {:?})",
               self.hash_headers, self.reader.cookie_ref().level);
            let body_length = BodyLength::parse_new_format(&mut self.reader);

            if ! self.hash_headers {
                if let Some(level) = self.reader.cookie_ref().level {
                    Cookie::hashing(
                        &mut self.reader, Hashing::Enabled, level);
                }
            }

            match body_length {
                Ok(BodyLength::Full(len)) => {
                    t!("Last chunk: {} bytes", len);
                    self.last = true;
                    self.partial_body_length = len;
                },
                Ok(BodyLength::Partial(len)) => {
                    t!("Next chunk: {} bytes", len);
                    self.partial_body_length = len;
                },
                Ok(BodyLength::Indeterminate) => {
                    // A new format packet can't return Indeterminate.
                    unreachable!();
                },
                Err(e) => {
                    t!("Err reading next chunk: {:?}", e);
                    err = Some(e);
                    break;
                }
            }
        }
        vec_truncate(&mut buffer, amount_buffered);

        // We're done.

        if let Some(b) = self.buffer.take() {
            self.unused_buffers.push(b);
        }
        self.buffer = Some(buffer);
        self.cursor = 0;

        if let Some(err) = err {
            Err(err)
        } else {
            Ok(())
        }
    }

    fn data_helper(&mut self, amount: usize, hard: bool, and_consume: bool)
                   -> Result<&[u8], std::io::Error> {
        tracer!(TRACE, "PBF::data_helper", self.cookie.level.unwrap_or(0));
        let mut need_fill = false;

        t!("amount {}, hard {:?}, and_consume {:?}, buffered {}, cursor {}",
           amount, hard, and_consume,
           self.buffer.as_ref().map(|b| b.len()).unwrap_or(0),
           self.cursor);

        if self.buffer.as_ref().map(|b| b.len() == self.cursor).unwrap_or(false)
        {
            // We have data and it has been exhausted exactly.  This
            // is our opportunity to get back to the happy path where
            // we don't need to double buffer.
            self.unused_buffers.push(self.buffer.take().expect("have buffer"));
            self.cursor = 0;
        }

        if let Some(ref buffer) = self.buffer {
            // We have some data buffered locally.

            //println!("  Reading from buffer");

            let amount_buffered = buffer.len() - self.cursor;
            if amount > amount_buffered {
                // The requested amount exceeds what is in the buffer.
                // Read more.

                // We can't call self.do_fill_buffer here, because self
                // is borrowed.  Set a flag and do it after the borrow
                // ends.
                need_fill = true;
                t!("Read of {} bytes exceeds buffered {} bytes",
                   amount, amount_buffered);
            }
        } else {
            // We don't have any data buffered.

            assert_eq!(self.cursor, 0);

            if amount <= self.partial_body_length as usize
                || /* Short read.  */ self.last {
                // The amount of data that the caller requested does
                // not exceed the amount of data in the current chunk.
                // As such, there is no need to double buffer.

                //println!("  Reading from inner reader");

                let result = if hard && and_consume {
                    self.reader.data_consume_hard (amount)
                } else if and_consume {
                    self.reader.data_consume (amount)
                } else {
                    self.reader.data(amount)
                };
                match result {
                    Ok(buffer) => {
                        let amount_buffered =
                            std::cmp::min(buffer.len(),
                                          self.partial_body_length as usize);
                        if hard && amount_buffered < amount {
                            return Err(Error::new(ErrorKind::UnexpectedEof,
                                                  "unexpected EOF"));
                        } else {
                            if and_consume {
                                self.partial_body_length -=
                                    cmp::min(amount, amount_buffered) as u32;
                            }
                            return Ok(&buffer[..amount_buffered]);
                        }
                    },
                    Err(err) => return Err(err),
                }
            } else {
                // `amount` crosses a partial body length boundary.
                // Do some buffering.

                //println!("  Read crosses chunk boundary.  Need to buffer.");

                need_fill = true;
                t!("Read straddles partial body chunk boundary");
            }
        }

        if need_fill {
            t!("Need to refill the buffer.");
            let result = self.do_fill_buffer(amount);
            if let Err(err) = result {
                return Err(err);
            }
        }

        //println!("  Buffer: {:?} (cursor at {})",
        //         if let Some(ref buffer) = self.buffer { Some(buffer.len()) } else { None },
        //         self.cursor);


        // Note: if we hit the EOF, then we might still have less
        // than `amount` data.  But, that's okay.  We just need to
        // return as much as we can in that case.
        let buffer = &self.buffer.as_ref().unwrap()[self.cursor..];
        if hard && buffer.len() < amount {
            return Err(Error::new(ErrorKind::UnexpectedEof, "unexpected EOF"));
        }
        if and_consume {
            self.cursor += cmp::min(amount, buffer.len());
        }
        Ok(buffer)
    }

}

impl<T: BufferedReader<Cookie>> std::io::Read
        for BufferedReaderPartialBodyFilter<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        buffered_reader_generic_read_impl(self, buf)
    }
}

impl<T: BufferedReader<Cookie>> BufferedReader<Cookie>
        for BufferedReaderPartialBodyFilter<T> {
    fn buffer(&self) -> &[u8] {
        if let Some(ref buffer) = self.buffer {
            &buffer[self.cursor..]
        } else {
            let buf = self.reader.buffer();
            &buf[..cmp::min(buf.len(),
                            self.partial_body_length as usize)]
        }
    }

    // Due to the mixing of usize (for lengths) and u32 (for OpenPGP),
    // we require that usize is at least as large as u32.
    // #[cfg(target_point_with = "32") or cfg(target_point_with = "64")]
    fn data(&mut self, amount: usize) -> Result<&[u8], std::io::Error> {
        self.data_helper(amount, false, false)
    }

    fn data_hard(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        self.data_helper(amount, true, false)
    }

    fn consume(&mut self, amount: usize) -> &[u8] {
        if let Some(ref buffer) = self.buffer {
            // We have a local buffer.

            self.cursor += amount;
            // The caller can't consume more than is buffered!
            assert!(self.cursor <= buffer.len());

            &buffer[self.cursor - amount..]
        } else {
            // Since we don't have a buffer, just pass through to the
            // underlying reader.
            assert!(amount <= self.partial_body_length as usize);
            self.partial_body_length -= amount as u32;
            self.reader.consume(amount)
        }
    }

    fn data_consume(&mut self, amount: usize) -> Result<&[u8], std::io::Error> {
        self.data_helper(amount, false, true)
    }

    fn data_consume_hard(&mut self, amount: usize) -> Result<&[u8], std::io::Error> {
        self.data_helper(amount, true, true)
    }

    fn consummated(&mut self) -> bool {
        self.partial_body_length == 0 && self.last
    }

    fn get_mut(&mut self) -> Option<&mut dyn BufferedReader<Cookie>> {
        Some(&mut self.reader)
    }

    fn get_ref(&self) -> Option<&dyn BufferedReader<Cookie>> {
        Some(&self.reader)
    }

    fn into_inner<'b>(self: Box<Self>) -> Option<Box<dyn BufferedReader<Cookie> + 'b>>
            where Self: 'b {
        Some(self.reader.as_boxed())
    }

    fn cookie_set(&mut self, cookie: Cookie) -> Cookie {
        use std::mem;

        mem::replace(&mut self.cookie, cookie)
    }

    fn cookie_ref(&self) -> &Cookie {
        &self.cookie
    }

    fn cookie_mut(&mut self) -> &mut Cookie {
        &mut self.cookie
    }
}
