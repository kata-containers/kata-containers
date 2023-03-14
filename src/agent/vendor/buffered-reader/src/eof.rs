use std::io;
use std::io::{Error, ErrorKind, Read};
use std::fmt;

use crate::BufferedReader;

/// Always returns EOF.
#[derive(Debug)]
pub struct EOF<C> {
    cookie: C,
}

assert_send_and_sync!(EOF<C>
                      where C: fmt::Debug);

impl<C> fmt::Display for EOF<C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EOF")
            .finish()
    }
}

#[allow(clippy::new_without_default)]
impl EOF<()> {
    /// Instantiates a new `EOF`.
    pub fn new() -> Self {
        EOF {
            cookie: (),
        }
    }
}

impl<C> EOF<C> {
    /// Instantiates a new `EOF` with a cookie.
    pub fn with_cookie(cookie: C) -> Self {
        EOF {
            cookie,
        }
    }
}

impl<C> Read for EOF<C> {
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, io::Error> {
        Ok(0)
    }
}

impl<C: fmt::Debug + Sync + Send> BufferedReader<C> for EOF<C> {
    fn buffer(&self) -> &[u8] {
        &b""[..]
    }

    fn data(&mut self, _amount: usize) -> Result<&[u8], io::Error> {
        Ok(&b""[..])
    }

    fn consume(&mut self, amount: usize) -> &[u8] {
        assert_eq!(amount, 0);
        &b""[..]
    }

    fn data_consume(&mut self, _amount: usize) -> Result<&[u8], io::Error> {
        Ok(&b""[..])
    }

    fn data_consume_hard(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        if amount == 0 {
            Ok(&b""[..])
        } else {
            Err(Error::new(ErrorKind::UnexpectedEof, "unexpected EOF"))
        }
    }

    fn into_inner<'a>(self: Box<Self>) -> Option<Box<dyn BufferedReader<C> + 'a>>
        where Self: 'a
    {
        None
    }

    fn get_mut(&mut self) -> Option<&mut dyn BufferedReader<C>>
    {
        None
    }

    fn get_ref(&self) -> Option<&dyn BufferedReader<C>>
    {
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
    fn basics() {
        let mut reader = EOF::new();

        assert_eq!(reader.buffer(), &b""[..]);
        assert_eq!(reader.data(100).unwrap(), &b""[..]);
        assert_eq!(reader.buffer(), &b""[..]);
        assert_eq!(reader.consume(0), &b""[..]);
        assert_eq!(reader.data_hard(0).unwrap(), &b""[..]);
        assert!(reader.data_hard(1).is_err());
    }
}
