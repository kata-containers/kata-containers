//! Encodes a byte stream as OpenPGP's dash-escaped text.
//!
//! This filter is used to generate messages using the Cleartext
//! Signature Framework (see [Section 7.1 of RFC 4880]).
//!
//!   [Section 7.1 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-7.1

use std::fmt;
use std::io;

use crate::{
    Error,
    Result,
    serialize::stream::{
        writer,
        Message,
        Cookie,
    },
};

pub(super) struct DashEscapeFilter<'a, C: 'a> {
    // The underlying writer.
    inner: writer::BoxStack<'a, C>,

    // The cookie.
    cookie: C,

    // The buffer.
    buffer: Vec<u8>,

    // The number of bytes written to this filter.
    position: u64,
}
assert_send_and_sync!(DashEscapeFilter<'_, C> where C);

#[allow(clippy::new_ret_no_self)]
impl<'a> DashEscapeFilter<'a, Cookie> {
    /// Returns a new filter applying dash-escaping to all lines.
    pub fn new(inner: Message<'a>, cookie: Cookie)
               -> Message<'a> {
        Message::from(Box::new(DashEscapeFilter {
            inner: inner.into(),
            cookie,
            buffer: Vec::new(),
            position: 0,
        }))
    }
}

impl<'a, C: 'a> DashEscapeFilter<'a, C> {
    /// Writes out any complete lines between `self.buffer` and `other`.
    ///
    /// Any extra data is buffered.
    ///
    /// If `done` is set, then flushes any data, and writes a final
    /// newline.
    fn write_out(&mut self, other: &[u8], done: bool)
                 -> io::Result<()> {
        // XXX: Currently, we don't mind copying the data.  This
        // could be optimized.
        self.buffer.extend_from_slice(other);

        if done && ! self.buffer.is_empty() && ! self.buffer.ends_with(b"\n") {
            self.buffer.push(b'\n');
        }

        // Write out all whole lines (i.e. those terminated by a
        // newline).  This is a bit awkward, because we only know that
        // a line was whole when we are looking at the next line.
        let mut last_line: Option<&[u8]> = None;
        for line in self.buffer.split(|b| *b == b'\n') {
            if let Some(l) = last_line.take() {
                if l.starts_with(b"-") || l.starts_with(b"From ") {
                    // Dash-escape!
                    self.inner.write_all(b"- ")?;
                }
                self.inner.write_all(l)?;
                self.inner.write_all(b"\n")?;
            }
            last_line = Some(line);
        }

        let new_buffer = last_line.map(|l| l.to_vec())
            .unwrap_or_else(Vec::new);
        crate::vec_truncate(&mut self.buffer, 0);
        self.buffer = new_buffer;

        Ok(())
    }
}

impl<'a, C: 'a> io::Write for DashEscapeFilter<'a, C> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_out(buf, false)?;
        self.position += buf.len() as u64;
        Ok(buf.len())
    }

    // XXX: The API says that `flush` is supposed to flush any
    // internal buffers to disk.  We don't do that.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a, C: 'a> fmt::Debug for DashEscapeFilter<'a, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("DashEscapeFilter")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<'a, C: 'a> writer::Stackable<'a, C> for DashEscapeFilter<'a, C> {
    fn into_inner(mut self: Box<Self>) -> Result<Option<writer::BoxStack<'a, C>>> {
        self.write_out(&b""[..], true)?;
        Ok(Some(self.inner))
    }
    fn pop(&mut self) -> Result<Option<writer::BoxStack<'a, C>>> {
        Err(Error::InvalidOperation(
            "Cannot pop DashEscapeFilter".into()).into())
    }
    fn mount(&mut self, new: writer::BoxStack<'a, C>) {
        self.inner = new;
    }
    fn inner_mut(&mut self) -> Option<&mut (dyn writer::Stackable<'a, C> + Send + Sync)> {
        Some(&mut self.inner)
    }
    fn inner_ref(&self) -> Option<&(dyn writer::Stackable<'a, C> + Send + Sync)> {
        Some(&self.inner)
    }
    fn cookie_set(&mut self, cookie: C) -> C {
        ::std::mem::replace(&mut self.cookie, cookie)
    }
    fn cookie_ref(&self) -> &C {
        &self.cookie
    }
    fn cookie_mut(&mut self) -> &mut C {
        &mut self.cookie
    }
    fn position(&self) -> u64 {
        self.position
    }
}

#[cfg(test)]
mod test {
    use std::io::Write;
    use super::*;
    use crate::serialize::stream::Message;

    #[test]
    fn no_escape() -> Result<()> {
        let mut buf = Vec::new();
        {
            let m = Message::new(&mut buf);
            let mut m = DashEscapeFilter::new(m, Default::default());
            m.write_all(b"0123")?;
            m.write_all(b"4567\n")?;
            m.write_all(b"89ab")?;
            m.write_all(b"cdef")?;
            m.write_all(b"\n")?;
            m.finalize()?;
        }
        assert_eq!(&buf[..], &b"01234567\n89abcdef\n"[..]);

        let mut buf = Vec::new();
        {
            let m = Message::new(&mut buf);
            let mut m = DashEscapeFilter::new(m, Default::default());
            m.write_all(b"0123")?;
            m.write_all(b"4567\n")?;
            m.write_all(b"89ab")?;
            m.write_all(b"cdef")?;
            // No final newline.
            m.finalize()?;
        }
        assert_eq!(&buf[..], &b"01234567\n89abcdef\n"[..]);

        Ok(())
    }

    #[test]
    fn dash_escape() -> Result<()> {
        let mut buf = Vec::new();
        {
            let m = Message::new(&mut buf);
            let mut m = DashEscapeFilter::new(m, Default::default());
            m.write_all(b"-0123")?;
            m.write_all(b"-4567\n")?;
            m.write_all(b"-89ab")?;
            m.write_all(b"-cdef")?;
            m.write_all(b"-\n")?;
            m.finalize()?;
        }
        assert_eq!(&buf[..], &b"- -0123-4567\n- -89ab-cdef-\n"[..]);

        let mut buf = Vec::new();
        {
            let m = Message::new(&mut buf);
            let mut m = DashEscapeFilter::new(m, Default::default());
            m.write_all(b"-0123")?;
            m.write_all(b"-4567\n")?;
            m.write_all(b"-89ab")?;
            m.write_all(b"-cdef")?;
            m.write_all(b"-")?;
            // No final newline.
            m.finalize()?;
        }
        assert_eq!(&buf[..], &b"- -0123-4567\n- -89ab-cdef-\n"[..]);

        Ok(())
    }

    #[test]
    fn from_escape() -> Result<()> {
        let mut buf = Vec::new();
        {
            let m = Message::new(&mut buf);
            let mut m = DashEscapeFilter::new(m, Default::default());
            m.write_all(b"From 0123")?;
            m.write_all(b"From 4567\n")?;
            m.write_all(b"From 89ab")?;
            m.write_all(b"From cdef")?;
            m.write_all(b"From \n")?;
            m.finalize()?;
        }
        assert_eq!(&buf[..], &b"- From 0123From 4567\n- From 89abFrom cdefFrom \n"[..]);

        let mut buf = Vec::new();
        {
            let m = Message::new(&mut buf);
            let mut m = DashEscapeFilter::new(m, Default::default());
            m.write_all(b"From 0123")?;
            m.write_all(b"From 4567\n")?;
            m.write_all(b"From 89ab")?;
            m.write_all(b"From cdef")?;
            m.write_all(b"From ")?;
            // No final newline.
            m.finalize()?;
        }
        assert_eq!(&buf[..], &b"- From 0123From 4567\n- From 89abFrom cdefFrom \n"[..]);

        Ok(())
    }
}
