//! Stackable writers.

#[cfg(feature = "compression-bzip2")]
mod writer_bzip2;
#[cfg(feature = "compression-bzip2")]
pub use self::writer_bzip2::BZ;
#[cfg(feature = "compression-deflate")]
mod writer_deflate;
#[cfg(feature = "compression-deflate")]
pub use self::writer_deflate::{ZIP, ZLIB};

use std::fmt;
use std::io;

use crate::armor;
use crate::crypto::{aead, symmetric};
use crate::types::{
    AEADAlgorithm,
    SymmetricAlgorithm,
};
use crate::{
    Result,
    crypto::SessionKey,
};
use super::{Message, Cookie};

impl<'a> Message<'a> {
    pub(super) fn from(bs: BoxStack<'a, Cookie>) -> Self {
        Message(bs)
    }

    pub(super) fn as_ref(&self) -> &BoxStack<'a, Cookie> {
        &self.0
    }

    pub(super) fn as_mut(&mut self) -> &mut BoxStack<'a, Cookie> {
        &mut self.0
    }
}

impl<'a> io::Write for Message<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl<'a> From<Message<'a>> for BoxStack<'a, Cookie> {
    fn from(s: Message<'a>) -> Self {
        s.0
    }
}

pub(crate) type BoxStack<'a, C> = Box<dyn Stackable<'a, C> + Send + Sync + 'a>;

/// Makes a writer stackable and provides convenience functions.
pub(crate) trait Stackable<'a, C> : io::Write + fmt::Debug {
    /// Recovers the inner stackable.
    ///
    /// This can fail if the current `Stackable` has buffered data
    /// that hasn't been written to the underlying `Stackable`.
    fn into_inner(self: Box<Self>) -> Result<Option<BoxStack<'a, C>>>;

    /// Pops the stackable from the stack, detaching it.
    ///
    /// Returns the detached stack.
    ///
    /// Note: Only the Signer implements this interface.
    fn pop(&mut self) -> Result<Option<BoxStack<'a, C>>>;

    /// Sets the inner stackable.
    ///
    /// Note: Only the Signer implements this interface.
    fn mount(&mut self, new: BoxStack<'a, C>);

    /// Returns a mutable reference to the inner `Writer`, if
    /// any.
    ///
    /// It is a very bad idea to write any data from the inner
    /// `Writer`, but it can sometimes be useful to get the cookie.
    fn inner_mut(&mut self) -> Option<&mut (dyn Stackable<'a, C> + Send + Sync)>;

    /// Returns a reference to the inner `Writer`.
    fn inner_ref(&self) -> Option<&(dyn Stackable<'a, C> + Send + Sync)>;

    /// Sets the cookie and returns the old value.
    fn cookie_set(&mut self, cookie: C) -> C;

    /// Returns a reference to the cookie.
    fn cookie_ref(&self) -> &C;

    /// Returns a mutable reference to the cookie.
    fn cookie_mut(&mut self) -> &mut C;

    /// Returns the number of bytes written to this filter.
    fn position(&self) -> u64;

    /// Writes a byte.
    fn write_u8(&mut self, b: u8) -> io::Result<()> {
        self.write_all(&[b])
    }

    /// Writes a big endian `u16`.
    fn write_be_u16(&mut self, n: u16) -> io::Result<()> {
        self.write_all(&n.to_be_bytes())
    }

    /// Writes a big endian `u32`.
    fn write_be_u32(&mut self, n: u32) -> io::Result<()> {
        self.write_all(&n.to_be_bytes())
    }
}

/// Make a `Box<Stackable>` look like a Stackable.
impl <'a, C> Stackable<'a, C> for BoxStack<'a, C> {
    fn into_inner(self: Box<Self>) -> Result<Option<BoxStack<'a, C>>> {
        (*self).into_inner()
    }
    /// Recovers the inner stackable.
    fn pop(&mut self) -> Result<Option<BoxStack<'a, C>>> {
        self.as_mut().pop()
    }
    /// Sets the inner stackable.
    fn mount(&mut self, new: BoxStack<'a, C>) {
        self.as_mut().mount(new);
    }
    fn inner_mut(&mut self) -> Option<&mut (dyn Stackable<'a, C> + Send + Sync)> {
        self.as_mut().inner_mut()
    }
    fn inner_ref(&self) -> Option<&(dyn Stackable<'a, C> + Send + Sync)> {
        self.as_ref().inner_ref()
    }
    fn cookie_set(&mut self, cookie: C) -> C {
        self.as_mut().cookie_set(cookie)
    }
    fn cookie_ref(&self) -> &C {
        self.as_ref().cookie_ref()
    }
    fn cookie_mut(&mut self) -> &mut C {
        self.as_mut().cookie_mut()
    }
    fn position(&self) -> u64 {
        self.as_ref().position()
    }
}

/// Maps a function over the stack of writers.
#[allow(dead_code)]
pub(crate) fn map<C, F>(head: &(dyn Stackable<C> + Send + Sync), mut fun: F)
    where F: FnMut(&(dyn Stackable<C> + Send + Sync)) -> bool {
    let mut ow = Some(head);
    while let Some(w) = ow {
        if ! fun(w) {
            break;
        }
        ow = w.inner_ref()
    }
}

/// Maps a function over the stack of mutable writers.
#[allow(dead_code)]
pub(crate) fn map_mut<C, F>(head: &mut (dyn Stackable<C> + Send + Sync), mut fun: F)
    where F: FnMut(&mut (dyn Stackable<C> + Send + Sync)) -> bool {
    let mut ow = Some(head);
    while let Some(w) = ow {
        if ! fun(w) {
            break;
        }
        ow = w.inner_mut()
    }
}

/// Dumps the writer stack.
#[allow(dead_code)]
pub(crate) fn dump<C>(head: &(dyn Stackable<C> + Send + Sync)) {
    let mut depth = 0;
    map(head, |w| {
        eprintln!("{}: {:?}", depth, w);
        depth += 1;
        true
    });
}

/// The identity writer just relays anything written.
pub struct Identity<'a, C> {
    inner: Option<BoxStack<'a, C>>,
    cookie: C,
}
assert_send_and_sync!(Identity<'_, C> where C);

#[allow(clippy::new_ret_no_self)]
impl<'a> Identity<'a, Cookie> {
    /// Makes an identity writer.
    pub fn new(inner: Message<'a>, cookie: Cookie)
                  -> Message<'a> {
        Message::from(Box::new(Self{inner: Some(inner.into()), cookie }))
    }
}

impl<'a, C> fmt::Debug for Identity<'a, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Identity")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<'a, C> io::Write for Identity<'a, C> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let writer = self.inner.as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe,
                                          "Writer is finalized."))?;
        writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let writer = self.inner.as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe,
                                          "Writer is finalized."))?;
        writer.flush()
    }
}

impl<'a, C> Stackable<'a, C> for Identity<'a, C> {
    /// Recovers the inner stackable.
    fn into_inner(self: Box<Self>) -> Result<Option<BoxStack<'a, C>>> {
        Ok(self.inner)
    }
    /// Recovers the inner stackable.
    fn pop(&mut self) -> Result<Option<BoxStack<'a, C>>> {
        Ok(self.inner.take())
    }
    /// Sets the inner stackable.
    fn mount(&mut self, new: BoxStack<'a, C>) {
        self.inner = Some(new);
    }
    fn inner_ref(&self) -> Option<&(dyn Stackable<'a, C> + Send + Sync)> {
        if let Some(ref i) = self.inner {
            Some(i)
        } else {
            None
        }
    }
    fn inner_mut(&mut self) -> Option<&mut (dyn Stackable<'a, C> + Send + Sync)> {
        if let Some(ref mut i) = self.inner {
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
        self.inner.as_ref().map(|i| i.position()).unwrap_or(0)
    }
}

/// Generic writer wrapping `io::Write`.
pub struct Generic<W: io::Write + Send + Sync, C> {
    inner: W,
    cookie: C,
    position: u64,
}
assert_send_and_sync!(Generic<W, C> where W: io::Write, C);

#[allow(clippy::new_ret_no_self)]
impl<'a, W: 'a + io::Write + Send + Sync> Generic<W, Cookie> {
    /// Wraps an `io::Write`r.
    pub fn new(inner: W, cookie: Cookie) -> Message<'a> {
        Message::from(Box::new(Self::new_unboxed(inner, cookie)))
    }

    fn new_unboxed(inner: W, cookie: Cookie) -> Self {
        Generic {
            inner,
            cookie,
            position: 0,
        }
    }
}

impl<W: io::Write + Send + Sync, C> fmt::Debug for Generic<W, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("writer::Generic")
            .finish()
    }
}

impl<W: io::Write + Send + Sync, C> io::Write for Generic<W, C> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self.inner.write(bytes) {
            Ok(n) => {
                self.position += n as u64;
                Ok(n)
            },
            Err(e) => Err(e),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<'a, W: io::Write + Send + Sync, C> Stackable<'a, C> for Generic<W, C> {
    /// Recovers the inner stackable.
    fn into_inner(self: Box<Self>) -> Result<Option<BoxStack<'a, C>>> {
        Ok(None)
    }
    /// Recovers the inner stackable.
    fn pop(&mut self) -> Result<Option<BoxStack<'a, C>>> {
        Ok(None)
    }
    /// Sets the inner stackable.
    fn mount(&mut self, _new: BoxStack<'a, C>) {
    }
    fn inner_mut(&mut self) -> Option<&mut (dyn Stackable<'a, C> + Send + Sync)> {
        // If you use Generic to wrap an io::Writer, and you know that
        // the io::Writer's inner is also a Stackable, then return a
        // reference to the innermost Stackable in your
        // implementation.  See e.g. writer::ZLIB.
        None
    }
    fn inner_ref(&self) -> Option<&(dyn Stackable<'a, C> + Send + Sync)> {
        // If you use Generic to wrap an io::Writer, and you know that
        // the io::Writer's inner is also a Stackable, then return a
        // reference to the innermost Stackable in your
        // implementation.  See e.g. writer::ZLIB.
        None
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


/// Armoring writer.
pub struct Armorer<'a, C: 'a> {
    inner: Generic<armor::Writer<BoxStack<'a, C>>, C>,
}
assert_send_and_sync!(Armorer<'_, C> where C);

#[allow(clippy::new_ret_no_self)]
impl<'a> Armorer<'a, Cookie> {
    /// Makes an armoring writer.
    pub fn new<I, K, V>(inner: Message<'a>, cookie: Cookie,
                        kind: armor::Kind, headers: I)
                        -> Result<Message<'a>>
        where I: IntoIterator<Item = (K, V)>,
              K: AsRef<str>,
              V: AsRef<str>,
    {
        Ok(Message::from(Box::new(Armorer {
            inner: Generic::new_unboxed(
                armor::Writer::with_headers(inner.into(), kind, headers)?,
                cookie),
        })))
    }
}

impl<'a, C: 'a> fmt::Debug for Armorer<'a, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("writer::Armorer")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<'a, C: 'a> io::Write for Armorer<'a, C> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.inner.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<'a, C: 'a> Stackable<'a, C> for Armorer<'a, C> {
    fn into_inner(self: Box<Self>) -> Result<Option<BoxStack<'a, C>>> {
        let inner = self.inner.inner.finalize()?;
        Ok(Some(inner))
    }
    fn pop(&mut self) -> Result<Option<BoxStack<'a, C>>> {
        unreachable!("Only implemented by Signer")
    }
    fn mount(&mut self, _new: BoxStack<'a, C>) {
        unreachable!("Only implemented by Signer")
    }
    fn inner_mut(&mut self) -> Option<&mut (dyn Stackable<'a, C> + Send + Sync)> {
        Some(self.inner.inner.get_mut().as_mut())
    }
    fn inner_ref(&self) -> Option<&(dyn Stackable<'a, C> + Send + Sync)> {
        Some(self.inner.inner.get_ref().as_ref())
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


/// Encrypting writer.
pub struct Encryptor<'a, C: 'a> {
    inner: Generic<symmetric::Encryptor<Box<dyn Stackable<'a, C> + Send + Sync + 'a>>, C>,
}
assert_send_and_sync!(Encryptor<'_, C> where C);

#[allow(clippy::new_ret_no_self)]
impl<'a> Encryptor<'a, Cookie> {
    /// Makes an encrypting writer.
    pub fn new(inner: Message<'a>, cookie: Cookie, algo: SymmetricAlgorithm,
               key: &[u8])
        -> Result<Message<'a>>
    {
        Ok(Message::from(Box::new(Encryptor {
            inner: Generic::new_unboxed(
                symmetric::Encryptor::new(algo, key, inner.into())?,
                cookie),
        })))
    }
}

impl<'a, C: 'a> fmt::Debug for Encryptor<'a, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("writer::Encryptor")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<'a, C: 'a> io::Write for Encryptor<'a, C> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.inner.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<'a, C: 'a> Stackable<'a, C> for Encryptor<'a, C> {
    fn into_inner(mut self: Box<Self>) -> Result<Option<BoxStack<'a, C>>> {
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
        // XXX: Unfortunately, this doesn't work due to a lifetime mismatch:
        // self.inner.inner.get_mut().map(|r| r.as_mut())
        None
    }
    fn inner_ref(&self) -> Option<&(dyn Stackable<'a, C> + Send + Sync)> {
        self.inner.inner.get_ref().map(|r| r.as_ref())
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


/// AEAD encrypting writer.
pub struct AEADEncryptor<'a, C: 'a, S: aead::Schedule> {
    inner: Generic<aead::Encryptor<BoxStack<'a, C>, S>, C>,
}
assert_send_and_sync!(AEADEncryptor<'_, C, S> where C, S: aead::Schedule);

#[allow(clippy::new_ret_no_self)]
impl<'a, S: 'a + aead::Schedule> AEADEncryptor<'a, Cookie, S> {
    /// Makes an encrypting writer.
    pub fn new(inner: Message<'a>, cookie: Cookie,
               cipher: SymmetricAlgorithm, aead: AEADAlgorithm,
               chunk_size: usize, schedule: S, key: SessionKey)
        -> Result<Message<'a>>
    {
        Ok(Message::from(Box::new(AEADEncryptor {
            inner: Generic::new_unboxed(
                aead::Encryptor::new(cipher, aead, chunk_size, schedule, key,
                                     inner.into())?,
                cookie),
        })))
    }
}

impl<'a, C: 'a, S: aead::Schedule> fmt::Debug for AEADEncryptor<'a, C, S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("writer::AEADEncryptor")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<'a, C: 'a, S: aead::Schedule> io::Write for AEADEncryptor<'a, C, S> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.inner.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<'a, C: 'a, S: aead::Schedule> Stackable<'a, C> for AEADEncryptor<'a, C, S> {
    fn into_inner(mut self: Box<Self>) -> Result<Option<BoxStack<'a, C>>> {
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
        // XXX: Unfortunately, this doesn't work due to a lifetime mismatch:
        // self.inner.inner.get_mut().map(|r| r.as_mut())
        None
    }
    fn inner_ref(&self) -> Option<&(dyn Stackable<'a, C> + Send + Sync)> {
        self.inner.inner.get_ref().map(|r| r.as_ref())
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

#[cfg(test)]
mod test {
    use std::io::Write;
    use super::*;

    #[test]
    fn generic_writer() {
        let mut inner = Vec::new();
        {
            let mut w = Generic::new(&mut inner, Cookie::new(0));
            assert_eq!(w.as_ref().cookie_ref().level, 0);
            dump(w.as_ref());

            *w.as_mut().cookie_mut() = Cookie::new(1);
            assert_eq!(w.as_ref().cookie_ref().level, 1);

            w.write_all(b"be happy").unwrap();
            let mut count = 0;
            map_mut(w.as_mut(), |g| {
                let new = Cookie::new(0);
                let old = g.cookie_set(new);
                assert_eq!(old.level, 1);
                count += 1;
                true
            });
            assert_eq!(count, 1);
            assert_eq!(w.as_ref().cookie_ref().level, 0);
        }
        assert_eq!(&inner, b"be happy");
    }

    #[test]
    fn stack() {
        let mut inner = Vec::new();
        {
            let w = Generic::new(&mut inner, Cookie::new(0));
            dump(w.as_ref());

            let w = Identity::new(w, Cookie::new(0));
            dump(w.as_ref());

            let mut count = 0;
            map(w.as_ref(), |g| {
                assert_eq!(g.cookie_ref().level, 0);
                count += 1;
                true
            });
            assert_eq!(count, 2);
        }
    }
}
