use std::io;
use std::cmp;

use super::*;

/// Limits the amount of data that can be read from a
/// `BufferedReader`.
#[derive(Debug)]
pub struct Limitor<T: BufferedReader<C>, C: fmt::Debug + Sync + Send> {
    limit: u64,
    cookie: C,
    reader: T,
}

assert_send_and_sync!(Limitor<T, C>
                      where T: BufferedReader<C>,
                            C: fmt::Debug);

impl<T: BufferedReader<C>, C: fmt::Debug + Sync + Send> fmt::Display for Limitor<T, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Limitor")
            .field("limit", &self.limit)
            .finish()
    }
}

impl<T: BufferedReader<()>> Limitor<T, ()> {
    /// Instantiates a new limitor.
    ///
    /// `reader` is the source to wrap.  `limit` is the maximum number
    /// of bytes that can be read from the source.
    pub fn new(reader: T, limit: u64) -> Self {
        Self::with_cookie(reader, limit, ())
    }
}

impl<T: BufferedReader<C>, C: fmt::Debug + Sync + Send> Limitor<T, C> {
    /// Like `new()`, but sets a cookie.
    ///
    /// The cookie can be retrieved using the `cookie_ref` and
    /// `cookie_mut` methods, and set using the `cookie_set` method.
    pub fn with_cookie(reader: T, limit: u64, cookie: C)
            -> Limitor<T, C> {
        Limitor {
            reader,
            limit,
            cookie,
        }
    }
}

impl<T: BufferedReader<C>, C: fmt::Debug + Sync + Send> io::Read for Limitor<T, C> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        let len = cmp::min(self.limit, buf.len() as u64) as usize;
        let result = self.reader.read(&mut buf[0..len]);
        if let Ok(amount) = result {
            self.limit -= amount as u64;
        }
        result
    }
}

impl<T: BufferedReader<C>, C: fmt::Debug + Sync + Send> BufferedReader<C> for Limitor<T, C> {
    fn buffer(&self) -> &[u8] {
        let buf = self.reader.buffer();
        &buf[..cmp::min(buf.len(),
                        cmp::min(std::usize::MAX as u64,
                                 self.limit) as usize)]
    }

    /// Return the buffer.  Ensure that it contains at least `amount`
    /// bytes.
    fn data(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        let amount = cmp::min(amount as u64, self.limit) as usize;
        let result = self.reader.data(amount);
        match result {
            Ok(buffer) =>
                if buffer.len() as u64 > self.limit {
                    Ok(&buffer[0..self.limit as usize])
                } else {
                    Ok(buffer)
                },
            Err(err) => Err(err),
        }
    }

    fn consume(&mut self, amount: usize) -> &[u8] {
        assert!(amount as u64 <= self.limit);
        self.limit -= amount as u64;
        let data = self.reader.consume(amount);
        &data[..cmp::min(self.limit + amount as u64, data.len() as u64) as usize]
    }

    fn data_consume(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        let amount = cmp::min(amount as u64, self.limit) as usize;
        let result = self.reader.data_consume(amount);
        if let Ok(buffer) = result {
            let amount = cmp::min(amount, buffer.len());
            self.limit -= amount as u64;
            return Ok(&buffer[
                ..cmp::min(buffer.len() as u64, self.limit + amount as u64) as usize]);
        }
        result
    }

    fn data_consume_hard(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        if amount as u64 > self.limit {
            return Err(Error::new(ErrorKind::UnexpectedEof, "EOF"));
        }
        let result = self.reader.data_consume_hard(amount);
        if let Ok(buffer) = result {
            let amount = cmp::min(amount, buffer.len());
            self.limit -= amount as u64;
            return Ok(&buffer[
                ..cmp::min(buffer.len() as u64, self.limit + amount as u64) as usize]);
        }
        result
    }

    fn consummated(&mut self) -> bool {
        self.limit == 0
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
    fn buffered_reader_limitor_test() {
        let data : &[u8] = b"01234567890123456789";

        /* Add a single limitor.  */
        {
            let mut bio : Box<dyn BufferedReader<()>>
                = Box::new(Memory::new(data));

            bio = {
                let mut bio2 = Box::new(Limitor::new(bio, 5));
                {
                    let result = bio2.data(5).unwrap();
                    assert_eq!(result.len(), 5);
                    assert_eq!(result, &b"01234"[..]);
                }
                bio2.consume(5);
                {
                    let result = bio2.data(1).unwrap();
                    assert_eq!(result.len(), 0);
                    assert_eq!(result, &b""[..]);
                }

                bio2.into_inner().unwrap()
            };

            {
                {
                    let result = bio.data(15).unwrap();
                    assert_eq!(result.len(), 15);
                    assert_eq!(result, &b"567890123456789"[..]);
                }
                bio.consume(15);
                {
                    let result = bio.data(1).unwrap();
                    assert_eq!(result.len(), 0);
                    assert_eq!(result, &b""[..]);
                }
            }
        }

        /* Try with two limitors where the first one imposes the real
         * limit.  */
        {
            let mut bio : Box<dyn BufferedReader<()>>
                = Box::new(Memory::new(data));

            bio = {
                let bio2 : Box<dyn BufferedReader<()>>
                    = Box::new(Limitor::new(bio, 5));
                // We limit to 15 bytes, but bio2 will still limit us to 5
                // bytes.
                let mut bio3 : Box<dyn BufferedReader<()>>
                    = Box::new(Limitor::new(bio2, 15));
                {
                    let result = bio3.data(100).unwrap();
                    assert_eq!(result.len(), 5);
                    assert_eq!(result, &b"01234"[..]);
                }
                bio3.consume(5);
                {
                    let result = bio3.data(1).unwrap();
                    assert_eq!(result.len(), 0);
                    assert_eq!(result, &b""[..]);
                }

                bio3.into_inner().unwrap().into_inner().unwrap()
            };

            {
                {
                    let result = bio.data(15).unwrap();
                    assert_eq!(result.len(), 15);
                    assert_eq!(result, &b"567890123456789"[..]);
                }
                bio.consume(15);
                {
                    let result = bio.data(1).unwrap();
                    assert_eq!(result.len(), 0);
                    assert_eq!(result, &b""[..]);
                }
            }
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

        let reader = Generic::new(&input[..], None);
        let size = size / 2;
        let input = &input[..size];
        let mut reader = Limitor::new(reader, input.len() as u64);

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

    #[test]
    fn consummated() {
        let data = b"0123456789";

        let mut l = Limitor::new(Memory::new(data), 10);
        l.drop_eof().unwrap();
        assert!(l.consummated());

        let mut l = Limitor::new(Memory::new(data), 20);
        l.drop_eof().unwrap();
        eprintln!("{:?}", l);
        assert!(! l.consummated());
    }
}
