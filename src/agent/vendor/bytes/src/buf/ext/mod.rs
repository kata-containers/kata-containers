//! Extra utilities for `Buf` and `BufMut` types.

use super::{Buf, BufMut};

mod chain;
mod limit;
#[cfg(feature = "std")]
mod reader;
mod take;
#[cfg(feature = "std")]
mod writer;

pub use self::chain::Chain;
pub use self::limit::Limit;
pub use self::take::Take;

#[cfg(feature = "std")]
pub use self::{reader::Reader, writer::Writer};

/// Extra methods for implementations of `Buf`.
pub trait BufExt: Buf {
    /// Creates an adaptor which will read at most `limit` bytes from `self`.
    ///
    /// This function returns a new instance of `Buf` which will read at most
    /// `limit` bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{BufMut, buf::BufExt};
    ///
    /// let mut buf = b"hello world"[..].take(5);
    /// let mut dst = vec![];
    ///
    /// dst.put(&mut buf);
    /// assert_eq!(dst, b"hello");
    ///
    /// let mut buf = buf.into_inner();
    /// dst.clear();
    /// dst.put(&mut buf);
    /// assert_eq!(dst, b" world");
    /// ```
    fn take(self, limit: usize) -> Take<Self>
    where
        Self: Sized,
    {
        take::new(self, limit)
    }

    /// Creates an adaptor which will chain this buffer with another.
    ///
    /// The returned `Buf` instance will first consume all bytes from `self`.
    /// Afterwards the output is equivalent to the output of next.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{Buf, buf::BufExt};
    ///
    /// let mut chain = b"hello "[..].chain(&b"world"[..]);
    ///
    /// let full = chain.to_bytes();
    /// assert_eq!(full.bytes(), b"hello world");
    /// ```
    fn chain<U: Buf>(self, next: U) -> Chain<Self, U>
    where
        Self: Sized,
    {
        Chain::new(self, next)
    }

    /// Creates an adaptor which implements the `Read` trait for `self`.
    ///
    /// This function returns a new value which implements `Read` by adapting
    /// the `Read` trait functions to the `Buf` trait functions. Given that
    /// `Buf` operations are infallible, none of the `Read` functions will
    /// return with `Err`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{Bytes, buf::BufExt};
    /// use std::io::Read;
    ///
    /// let buf = Bytes::from("hello world");
    ///
    /// let mut reader = buf.reader();
    /// let mut dst = [0; 1024];
    ///
    /// let num = reader.read(&mut dst).unwrap();
    ///
    /// assert_eq!(11, num);
    /// assert_eq!(&dst[..11], &b"hello world"[..]);
    /// ```
    #[cfg(feature = "std")]
    fn reader(self) -> Reader<Self>
    where
        Self: Sized,
    {
        reader::new(self)
    }
}

impl<B: Buf + ?Sized> BufExt for B {}

/// Extra methods for implementations of `BufMut`.
pub trait BufMutExt: BufMut {
    /// Creates an adaptor which can write at most `limit` bytes to `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{BufMut, buf::BufMutExt};
    ///
    /// let arr = &mut [0u8; 128][..];
    /// assert_eq!(arr.remaining_mut(), 128);
    ///
    /// let dst = arr.limit(10);
    /// assert_eq!(dst.remaining_mut(), 10);
    /// ```
    fn limit(self, limit: usize) -> Limit<Self>
    where
        Self: Sized,
    {
        limit::new(self, limit)
    }

    /// Creates an adaptor which implements the `Write` trait for `self`.
    ///
    /// This function returns a new value which implements `Write` by adapting
    /// the `Write` trait functions to the `BufMut` trait functions. Given that
    /// `BufMut` operations are infallible, none of the `Write` functions will
    /// return with `Err`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::buf::BufMutExt;
    /// use std::io::Write;
    ///
    /// let mut buf = vec![].writer();
    ///
    /// let num = buf.write(&b"hello world"[..]).unwrap();
    /// assert_eq!(11, num);
    ///
    /// let buf = buf.into_inner();
    ///
    /// assert_eq!(*buf, b"hello world"[..]);
    /// ```
    #[cfg(feature = "std")]
    fn writer(self) -> Writer<Self>
    where
        Self: Sized,
    {
        writer::new(self)
    }

    /// Creates an adapter which will chain this buffer with another.
    ///
    /// The returned `BufMut` instance will first write to all bytes from
    /// `self`. Afterwards, it will write to `next`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{BufMut, buf::BufMutExt};
    ///
    /// let mut a = [0u8; 5];
    /// let mut b = [0u8; 6];
    ///
    /// let mut chain = (&mut a[..]).chain_mut(&mut b[..]);
    ///
    /// chain.put_slice(b"hello world");
    ///
    /// assert_eq!(&a[..], b"hello");
    /// assert_eq!(&b[..], b" world");
    /// ```
    fn chain_mut<U: BufMut>(self, next: U) -> Chain<Self, U>
    where
        Self: Sized,
    {
        Chain::new(self, next)
    }
}

impl<B: BufMut + ?Sized> BufMutExt for B {}
