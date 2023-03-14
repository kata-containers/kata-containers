use std::io;

use super::*;

/// A `Reserve` allows a reader to read everything
/// except for the last N bytes (the reserve) from the underlying
/// `BufferedReader`.
///
/// Note: because the `Reserve` doesn't generally know
/// how much data can be read from the underlying `BufferedReader`,
/// it causes at least N bytes to by buffered.
#[derive(Debug)]
pub struct Reserve<T: BufferedReader<C>, C: fmt::Debug + Sync + Send> {
    reserve: usize,
    cookie: C,
    reader: T,
}

assert_send_and_sync!(Reserve<T, C>
                      where T: BufferedReader<C>,
                            C: fmt::Debug);

impl<T: BufferedReader<C>, C: fmt::Debug + Sync + Send> fmt::Display for Reserve<T, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Reserve")
            .field("reserve", &self.reserve)
            .finish()
    }
}

impl<T: BufferedReader<()>> Reserve<T, ()> {
    /// Instantiates a new `Reserve`.
    ///
    /// `reader` is the source to wrap.  `reserve` is the number of
    /// bytes that will not be returned to the reader.
    pub fn new(reader: T, reserve: usize) -> Self {
        Self::with_cookie(reader, reserve, ())
    }
}

impl<T: BufferedReader<C>, C: fmt::Debug + Sync + Send> Reserve<T, C> {
    /// Like `new()`, but sets a cookie.
    ///
    /// The cookie can be retrieved using the `cookie_ref` and
    /// `cookie_mut` methods, and set using the `cookie_set` method.
    pub fn with_cookie(reader: T, reserve: usize, cookie: C)
            -> Reserve<T, C> {
        Reserve {
            reader,
            reserve,
            cookie,
        }
    }
}

impl<T: BufferedReader<C>, C: fmt::Debug + Sync + Send> io::Read for Reserve<T, C> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        let to_read = {
            let data = self.reader.data(buf.len() + self.reserve)?;
            if data.len() > self.reserve {
                data.len() - self.reserve
            } else {
                return Ok(0);
            }
        };

        let to_read = cmp::min(buf.len(), to_read);

        self.reader.read(&mut buf[..to_read])
    }
}

impl<T: BufferedReader<C>, C: fmt::Debug + Send + Sync> BufferedReader<C> for Reserve<T, C> {
    fn buffer(&self) -> &[u8] {
        let buf = self.reader.buffer();
        if buf.len() > self.reserve {
            &buf[..buf.len() - self.reserve]
        } else {
            b""
        }
    }

    /// Return the buffer.  Ensure that it contains at least `amount`
    /// bytes.
    fn data(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        let data = self.reader.data(amount + self.reserve)?;
        if data.len() <= self.reserve {
            // EOF.
            Ok(b"")
        } else {
            // More than enough.
            Ok(&data[..data.len() - self.reserve])
        }
    }

    fn consume(&mut self, amount: usize) -> &[u8] {
        assert!(amount <= self.buffer().len());

        // consume may return more than amount.  If it does, make sure
        // it doesn't return any of the reserve.
        let data = self.reader.consume(amount);
        assert!(data.len() >= amount);

        if data.len() > amount {
            // We got more than `amount`.  We need to be careful to
            // not return data from the reserve.  But, we also know
            // that `amount` does not include data from the reserve.
            if data.len() > amount + self.reserve {
                return &data[..data.len() - self.reserve];
            }
        }
        &data[..amount]
    }

    fn data_consume(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        let amount = cmp::min(amount, self.data(amount)?.len());
        Ok(self.consume(amount))
    }

    fn data_consume_hard(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        self.data_hard(amount)?;
        Ok(self.consume(amount))
    }

    fn get_mut(&mut self) -> Option<&mut dyn BufferedReader<C>> {
        Some(&mut self.reader)
    }

    fn get_ref(&self) -> Option<&dyn BufferedReader<C>> {
        Some(&self.reader)
    }

    fn into_inner<'b>(self: Box<Self>) -> Option<Box<dyn BufferedReader<C> + 'b>>
        where Self: 'b {
        Some(self.reader.as_boxed())
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
    fn data() {
        use crate::Memory;

        // orig is the original buffer
        //
        // cursor is the Reserve's position in orig.
        //
        // to_read is how much to read.
        //
        // total is the total to_read that be read from orig.
        //
        //           cursor                    /  reserve  \
        // orig: [      | to_read  |          |             ]
        //        \          total           /
        //
        fn read_chunk<'a, R: BufferedReader<C>, C: fmt::Debug + Sync + Send>(
            orig: &[u8], r: &mut R, to_read: usize, cursor: usize, total: usize,
            mode: usize)
        {
            // Use data.
            let data_len = {
                let data = r.data(to_read).unwrap();
                assert_eq!(data, &orig[cursor..cursor + data.len()]);
                data.len()
            };
            assert!(data_len <= total - cursor);
            assert_eq!(r.buffer().len(), data_len);

            // Use data_hard.
            let data_hard_len = {
                let data_hard = r.data_hard(to_read).unwrap();
                assert_eq!(data_hard, &orig[cursor..cursor + data_hard.len()]);
                data_hard.len()
            };
            assert!(data_len <= data_hard_len);
            assert!(data_hard_len <= total - cursor);
            assert_eq!(r.buffer().len(), data_hard_len);



            // Make sure data_hard fails when requesting too much
            // data.
            assert!(r.data_hard(total - cursor + 1).is_err());

            // And that a failing data_hard does not move the cursor.
            let data_len = {
                let data = r.data(to_read).unwrap();
                assert_eq!(data, &orig[cursor..cursor + data.len()]);
                data.len()
            };
            assert!(data_len <= total - cursor);
            assert_eq!(r.buffer().len(), data_len);


            // Likewise for data_consume_hard.
            assert!(r.data_consume_hard(total - cursor + 1).is_err());

            // And that a failing data_hard does not move the cursor.
            let data_len = {
                let data = r.data(to_read).unwrap();
                assert_eq!(data, &orig[cursor..cursor + data.len()]);
                data.len()
            };
            assert!(data_len <= total - cursor);
            assert_eq!(r.buffer().len(), data_len);



            // Consume the chunk.
            match mode {
                0 => {
                    // Use consume.
                    let l = r.consume(to_read).len();
                    assert!(to_read <= l);
                    assert!(l <= total - cursor);
                }
                1 => {
                    // Use data_consume.
                    let data_len = {
                        let data = r.data_consume(to_read).unwrap();
                        assert_eq!(data, &orig[cursor..cursor + data.len()]);
                        data.len()
                    };
                    assert!(data_len <= total - cursor);
                    assert!(r.buffer().len() <= total - cursor - to_read);
                }
                2 => {
                    // Use data_consume_hard.
                    let data_len = {
                        let data = r.data_consume_hard(to_read).unwrap();
                        assert_eq!(data, &orig[cursor..cursor + data.len()]);
                        data.len()
                    };
                    assert!(data_len <= total - cursor);
                    assert!(r.buffer().len() <= total - cursor - to_read);
                }
                _ => panic!("Invalid mode"),
            }
        }

        fn test(orig: &[u8], mode: usize, reserve: usize,
                mid1: usize, mid2: usize) {
            let total = orig.len() - reserve;

            let mut r = Reserve::new(
                Memory::new(orig), reserve);

            // Read the first chunk.
            read_chunk(orig, &mut r, mid1, 0, total, mode);

            // Read the second chunk.
            read_chunk(orig, &mut r, mid2 - mid1, mid1, total, mode);

            // Read the remaining bit.
            read_chunk(orig, &mut r, total - mid2, mid2, total, mode);

            // And, we should be at EOF.
            assert_eq!(r.data(100).unwrap().len(), 0);
            assert_eq!(r.buffer().len(), 0);
            assert!(r.data_hard(100).is_err());
            assert_eq!(r.data_hard(0).unwrap().len(), 0);

            let mut g = Box::new(r).into_inner().unwrap();
            read_chunk(orig, &mut g,
                       orig.len() - total, total, orig.len(),
                       mode);
        }

        // 26 letters.
        let orig : &[u8] = b"abcdefghijklmnopqrstuvwxyz";

        // We break up the above into four pieces: three chunks, and
        // the reserved area.
        for mode in 0..3 {
            for reserve in 0..orig.len() {
                let total = orig.len() - reserve;

                for mid1 in 0..total {
                    for mid2 in mid1..total {
                        test(orig, mode, reserve, mid1, mid2);
                    }
                }
            }
        }
    }
}
