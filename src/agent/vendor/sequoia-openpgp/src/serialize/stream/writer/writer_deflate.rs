use flate2::write::{DeflateEncoder, ZlibEncoder};
use std::fmt;
use std::io;

use crate::Result;
use crate::types::CompressionLevel;
use super::{Generic, Message, BoxStack, Stackable, Cookie};

/// ZIP compressing writer.
#[allow(clippy::upper_case_acronyms)]
pub struct ZIP<'a, C: 'a> {
    inner: Generic<DeflateEncoder<BoxStack<'a, C>>, C>,
}
assert_send_and_sync!(ZIP<'_, C> where C);

#[allow(clippy::new_ret_no_self)]
impl<'a> ZIP<'a, Cookie> {
    /// Makes a ZIP compressing writer.
    pub fn new<L>(inner: Message<'a>, cookie: Cookie, level: L) -> Message<'a>
        where L: Into<Option<CompressionLevel>>
    {
        Message::from(Box::new(ZIP {
            inner: Generic::new_unboxed(
                DeflateEncoder::new(inner.into(),
                                    level.into().unwrap_or_default().into()),
                cookie),
        }))
    }
}

impl<'a, C: 'a> fmt::Debug for ZIP<'a, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("writer::ZIP")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<'a, C: 'a> io::Write for ZIP<'a, C> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.inner.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<'a, C: 'a> Stackable<'a, C> for ZIP<'a, C> {
    fn into_inner(self: Box<Self>) -> Result<Option<BoxStack<'a, C>>> {
        let inner = self.inner.inner.finish()?;
        Ok(Some(inner))
    }
    fn pop(&mut self) -> Result<Option<BoxStack<'a, C>>> {
        unreachable!("Only implemented by Signer")
    }
    fn mount(&mut self, _new: BoxStack<'a, C>) {
        unreachable!("Only implemented by Signer")
    }
    fn inner_mut(&mut self) -> Option<&mut (dyn Stackable<'a, C> + Send + Sync)> {
        Some(self.inner.inner.get_mut())
    }
    fn inner_ref(&self) -> Option<&(dyn Stackable<'a, C> + Send + Sync)> {
        Some(self.inner.inner.get_ref())
    }
    fn cookie_set(&mut self, cookie: C) -> C {
        self.inner.cookie_set(cookie)
    }
    fn cookie_ref(&self) -> &C {
        self.inner.cookie_ref()
    }
    fn cookie_mut(&mut self) -> &mut C {
        self.inner.cookie_mut()
    }
    fn position(&self) -> u64 {
        self.inner.position
    }
}

/// ZLIB compressing writer.
#[allow(clippy::upper_case_acronyms)]
pub struct ZLIB<'a, C: 'a> {
    inner: Generic<ZlibEncoder<BoxStack<'a, C>>, C>,
}
assert_send_and_sync!(ZLIB<'_, C> where C);

#[allow(clippy::new_ret_no_self)]
impl<'a> ZLIB<'a, Cookie> {
    /// Makes a ZLIB compressing writer.
    pub fn new<L>(inner: Message<'a>, cookie: Cookie, level: L) -> Message<'a>
        where L: Into<Option<CompressionLevel>>
    {
        Message::from(Box::new(ZLIB {
            inner: Generic::new_unboxed(
                ZlibEncoder::new(inner.into(),
                                 level.into().unwrap_or_default().into()),
                cookie),
        }))
    }
}

impl<'a, C:> fmt::Debug for ZLIB<'a, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("writer::ZLIB")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<'a, C: 'a> io::Write for ZLIB<'a, C> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.inner.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<'a, C: 'a> Stackable<'a, C> for ZLIB<'a, C> {
    fn into_inner(self: Box<Self>) -> Result<Option<BoxStack<'a, C>>> {
        let inner = self.inner.inner.finish()?;
        Ok(Some(inner))
    }
    fn pop(&mut self) -> Result<Option<BoxStack<'a, C>>> {
        unreachable!("Only implemented by Signer")
    }
    fn mount(&mut self, _new: BoxStack<'a, C>) {
        unreachable!("Only implemented by Signer")
    }
    fn inner_mut(&mut self) -> Option<&mut (dyn Stackable<'a, C> + Send + Sync)> {
        Some(self.inner.inner.get_mut())
    }
    fn inner_ref(&self) -> Option<&(dyn Stackable<'a, C> + Send + Sync)> {
        Some(self.inner.inner.get_ref())
    }
    fn cookie_set(&mut self, cookie: C) -> C {
        self.inner.cookie_set(cookie)
    }
    fn cookie_ref(&self) -> &C {
        self.inner.cookie_ref()
    }
    fn cookie_mut(&mut self) -> &mut C {
        self.inner.cookie_mut()
    }
    fn position(&self) -> u64 {
        self.inner.position
    }
}
