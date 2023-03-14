//! Encodes a byte stream using OpenPGP's partial body encoding.

use std::fmt;
use std::io;
use std::cmp;

use crate::Error;
use crate::Result;
use crate::packet::header::BodyLength;
use crate::serialize::{
    log2,
    stream::{
        writer,
        Message,
        Cookie,
    },
    write_byte,
    Marshal,
};

pub struct PartialBodyFilter<'a, C: 'a> {
    // The underlying writer.
    //
    // XXX: Opportunity for optimization.  Previously, this writer
    // implemented `Drop`, so we could not move the inner writer out
    // of this writer.  We therefore wrapped it with `Option` so that
    // we can `take()` it.  This writer no longer implements Drop, so
    // we could avoid the Option here.
    inner: Option<writer::BoxStack<'a, C>>,

    // The cookie.
    cookie: C,

    // The buffer.
    buffer: Vec<u8>,

    // The amount to buffer before flushing.
    buffer_threshold: usize,

    // The maximum size of a partial body chunk.  The standard allows
    // for chunks up to 1 GB in size.
    max_chunk_size: usize,

    // The number of bytes written to this filter.
    position: u64,
}
assert_send_and_sync!(PartialBodyFilter<'_, C> where C);

const PARTIAL_BODY_FILTER_MAX_CHUNK_SIZE : usize = 1 << 30;

// The amount to buffer before flushing.  If this is small, we get
// lots of small partial body packets, which is annoying.
const PARTIAL_BODY_FILTER_BUFFER_THRESHOLD : usize = 4 * 1024 * 1024;

#[allow(clippy::new_ret_no_self)]
impl<'a> PartialBodyFilter<'a, Cookie> {
    /// Returns a new partial body encoder.
    pub fn new(inner: Message<'a>, cookie: Cookie)
               -> Message<'a> {
        Self::with_limits(inner, cookie,
                          PARTIAL_BODY_FILTER_BUFFER_THRESHOLD,
                          PARTIAL_BODY_FILTER_MAX_CHUNK_SIZE)
            .expect("safe limits")
    }

    /// Returns a new partial body encoder with the given limits.
    pub fn with_limits(inner: Message<'a>, cookie: Cookie,
                       buffer_threshold: usize,
                       max_chunk_size: usize)
                       -> Result<Message<'a>> {
        if buffer_threshold.count_ones() != 1 {
            return Err(Error::InvalidArgument(
                "buffer_threshold is not a power of two".into()).into());
        }

        if max_chunk_size.count_ones() != 1 {
            return Err(Error::InvalidArgument(
                "max_chunk_size is not a power of two".into()).into());
        }

        if max_chunk_size > PARTIAL_BODY_FILTER_MAX_CHUNK_SIZE {
            return Err(Error::InvalidArgument(
                "max_chunk_size exceeds limit".into()).into());
        }

        Ok(Message::from(Box::new(PartialBodyFilter {
            inner: Some(inner.into()),
            cookie,
            buffer: Vec::with_capacity(buffer_threshold),
            buffer_threshold,
            max_chunk_size,
            position: 0,
        })))
    }
}

impl<'a, C: 'a> PartialBodyFilter<'a, C> {
    // Writes out any full chunks between `self.buffer` and `other`.
    // Any extra data is buffered.
    //
    // If `done` is set, then flushes any data, and writes the end of
    // the partial body encoding.
    fn write_out(&mut self, mut other: &[u8], done: bool)
                 -> io::Result<()> {
        if self.inner.is_none() {
            return Ok(());
        }
        let mut inner = self.inner.as_mut().unwrap();

        if done {
            // We're done.  The last header MUST be a non-partial body
            // header.  We have to write it even if it is 0 bytes
            // long.

            // Write the header.
            let l = self.buffer.len() + other.len();
            if l > std::u32::MAX as usize {
                unimplemented!();
            }
            BodyLength::Full(l as u32).serialize(inner).map_err(
                |e| match e.downcast::<io::Error>() {
                        // An io::Error.  Pass as-is.
                        Ok(err) => err,
                        // A failure.  Wrap it.
                        Err(e) => io::Error::new(io::ErrorKind::Other, e),
                    })?;

            // Write the body.
            inner.write_all(&self.buffer[..])?;
            crate::vec_truncate(&mut self.buffer, 0);
            inner.write_all(other)?;
        } else {
            while self.buffer.len() + other.len() > self.buffer_threshold {

                // Write a partial body length header.
                let chunk_size_log2 =
                    log2(cmp::min(self.max_chunk_size,
                                  self.buffer.len() + other.len())
                         as u32);
                let chunk_size = (1usize) << chunk_size_log2;

                let size = BodyLength::Partial(chunk_size as u32);
                let mut size_byte = [0u8];
                size.serialize(&mut io::Cursor::new(&mut size_byte[..]))
                    .expect("size should be representable");
                let size_byte = size_byte[0];

                // Write out the chunk...
                write_byte(&mut inner, size_byte)?;

                // ... from our buffer first...
                let l = cmp::min(self.buffer.len(), chunk_size);
                inner.write_all(&self.buffer[..l])?;
                crate::vec_drain_prefix(&mut self.buffer, l);

                // ... then from other.
                if chunk_size > l {
                    inner.write_all(&other[..chunk_size - l])?;
                    other = &other[chunk_size - l..];
                }
            }

            self.buffer.extend_from_slice(other);
            assert!(self.buffer.len() <= self.buffer_threshold);
        }

        Ok(())
    }
}

impl<'a, C: 'a> io::Write for PartialBodyFilter<'a, C> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // If we can write out a chunk, avoid an extra copy.
        if buf.len() >= self.buffer_threshold - self.buffer.len() {
            self.write_out(buf, false)?;
        } else {
            self.buffer.append(buf.to_vec().as_mut());
        }
        self.position += buf.len() as u64;
        Ok(buf.len())
    }

    // XXX: The API says that `flush` is supposed to flush any
    // internal buffers to disk.  We don't do that.
    fn flush(&mut self) -> io::Result<()> {
        self.write_out(&b""[..], false)
    }
}

impl<'a, C: 'a> fmt::Debug for PartialBodyFilter<'a, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PartialBodyFilter")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<'a, C: 'a> writer::Stackable<'a, C> for PartialBodyFilter<'a, C> {
    fn into_inner(mut self: Box<Self>) -> Result<Option<writer::BoxStack<'a, C>>> {
        self.write_out(&b""[..], true)?;
        Ok(self.inner.take())
    }
    fn pop(&mut self) -> Result<Option<writer::BoxStack<'a, C>>> {
        self.write_out(&b""[..], true)?;
        Ok(self.inner.take())
    }
    fn mount(&mut self, new: writer::BoxStack<'a, C>) {
        self.inner = Some(new);
    }
    fn inner_mut(&mut self) -> Option<&mut (dyn writer::Stackable<'a, C> + Send + Sync)> {
        if let Some(ref mut i) = self.inner {
            Some(i)
        } else {
            None
        }
    }
    fn inner_ref(&self) -> Option<&(dyn writer::Stackable<'a, C> + Send + Sync)> {
        if let Some(ref i) = self.inner {
            Some(i)
        } else {
            None
        }
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
    fn basic() {
        let mut buf = Vec::new();
        {
            let message = Message::new(&mut buf);
            let mut pb = PartialBodyFilter::with_limits(
                message, Default::default(),
                /* buffer_threshold: */ 16,
                /*   max_chunk_size: */ 16)
                .unwrap();
            pb.write_all(b"0123").unwrap();
            pb.write_all(b"4567").unwrap();
            pb.finalize().unwrap();
        }
        assert_eq!(&buf,
                   &[8, // no chunking
                     0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37]);
    }

    #[test]
    fn no_avoidable_chunking() {
        let mut buf = Vec::new();
        {
            let message = Message::new(&mut buf);
            let mut pb = PartialBodyFilter::with_limits(
                message, Default::default(),
                /* buffer_threshold: */ 4,
                /*   max_chunk_size: */ 16)
                .unwrap();
            pb.write_all(b"01234567").unwrap();
            pb.finalize().unwrap();
        }
        assert_eq!(&buf,
                   &[0xe0 + 3, // first chunk
                     0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37,
                     0, // rest
                   ]);
    }

    #[test]
    fn write_exceeding_buffer_threshold() {
        let mut buf = Vec::new();
        {
            let message = Message::new(&mut buf);
            let mut pb = PartialBodyFilter::with_limits(
                message, Default::default(),
                /* buffer_threshold: */ 8,
                /*   max_chunk_size: */ 16)
                .unwrap();
            pb.write_all(b"012345670123456701234567").unwrap();
            pb.finalize().unwrap();
        }
        assert_eq!(&buf,
                   &[0xe0 + 4, // first chunk
                     0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37,
                     0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37,
                     8, // rest
                     0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37]);
    }
}
