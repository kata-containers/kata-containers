use core::{
    cmp,
    mem::{self, MaybeUninit},
    ptr, usize,
};

#[cfg(feature = "std")]
use std::fmt;

use alloc::{boxed::Box, vec::Vec};

/// A trait for values that provide sequential write access to bytes.
///
/// Write bytes to a buffer
///
/// A buffer stores bytes in memory such that write operations are infallible.
/// The underlying storage may or may not be in contiguous memory. A `BufMut`
/// value is a cursor into the buffer. Writing to `BufMut` advances the cursor
/// position.
///
/// The simplest `BufMut` is a `Vec<u8>`.
///
/// ```
/// use bytes::BufMut;
///
/// let mut buf = vec![];
///
/// buf.put(&b"hello world"[..]);
///
/// assert_eq!(buf, b"hello world");
/// ```
pub trait BufMut {
    /// Returns the number of bytes that can be written from the current
    /// position until the end of the buffer is reached.
    ///
    /// This value is greater than or equal to the length of the slice returned
    /// by `bytes_mut`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut dst = [0; 10];
    /// let mut buf = &mut dst[..];
    ///
    /// let original_remaining = buf.remaining_mut();
    /// buf.put(&b"hello"[..]);
    ///
    /// assert_eq!(original_remaining - 5, buf.remaining_mut());
    /// ```
    ///
    /// # Implementer notes
    ///
    /// Implementations of `remaining_mut` should ensure that the return value
    /// does not change unless a call is made to `advance_mut` or any other
    /// function that is documented to change the `BufMut`'s current position.
    fn remaining_mut(&self) -> usize;

    /// Advance the internal cursor of the BufMut
    ///
    /// The next call to `bytes_mut` will return a slice starting `cnt` bytes
    /// further into the underlying buffer.
    ///
    /// This function is unsafe because there is no guarantee that the bytes
    /// being advanced past have been initialized.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = Vec::with_capacity(16);
    ///
    /// unsafe {
    ///     // MaybeUninit::as_mut_ptr
    ///     buf.bytes_mut()[0].as_mut_ptr().write(b'h');
    ///     buf.bytes_mut()[1].as_mut_ptr().write(b'e');
    ///
    ///     buf.advance_mut(2);
    ///
    ///     buf.bytes_mut()[0].as_mut_ptr().write(b'l');
    ///     buf.bytes_mut()[1].as_mut_ptr().write(b'l');
    ///     buf.bytes_mut()[2].as_mut_ptr().write(b'o');
    ///
    ///     buf.advance_mut(3);
    /// }
    ///
    /// assert_eq!(5, buf.len());
    /// assert_eq!(buf, b"hello");
    /// ```
    ///
    /// # Panics
    ///
    /// This function **may** panic if `cnt > self.remaining_mut()`.
    ///
    /// # Implementer notes
    ///
    /// It is recommended for implementations of `advance_mut` to panic if
    /// `cnt > self.remaining_mut()`. If the implementation does not panic,
    /// the call must behave as if `cnt == self.remaining_mut()`.
    ///
    /// A call with `cnt == 0` should never panic and be a no-op.
    unsafe fn advance_mut(&mut self, cnt: usize);

    /// Returns true if there is space in `self` for more bytes.
    ///
    /// This is equivalent to `self.remaining_mut() != 0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut dst = [0; 5];
    /// let mut buf = &mut dst[..];
    ///
    /// assert!(buf.has_remaining_mut());
    ///
    /// buf.put(&b"hello"[..]);
    ///
    /// assert!(!buf.has_remaining_mut());
    /// ```
    fn has_remaining_mut(&self) -> bool {
        self.remaining_mut() > 0
    }

    /// Returns a mutable slice starting at the current BufMut position and of
    /// length between 0 and `BufMut::remaining_mut()`. Note that this *can* be shorter than the
    /// whole remainder of the buffer (this allows non-continuous implementation).
    ///
    /// This is a lower level function. Most operations are done with other
    /// functions.
    ///
    /// The returned byte slice may represent uninitialized memory.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = Vec::with_capacity(16);
    ///
    /// unsafe {
    ///     // MaybeUninit::as_mut_ptr
    ///     buf.bytes_mut()[0].as_mut_ptr().write(b'h');
    ///     buf.bytes_mut()[1].as_mut_ptr().write(b'e');
    ///
    ///     buf.advance_mut(2);
    ///
    ///     buf.bytes_mut()[0].as_mut_ptr().write(b'l');
    ///     buf.bytes_mut()[1].as_mut_ptr().write(b'l');
    ///     buf.bytes_mut()[2].as_mut_ptr().write(b'o');
    ///
    ///     buf.advance_mut(3);
    /// }
    ///
    /// assert_eq!(5, buf.len());
    /// assert_eq!(buf, b"hello");
    /// ```
    ///
    /// # Implementer notes
    ///
    /// This function should never panic. `bytes_mut` should return an empty
    /// slice **if and only if** `remaining_mut` returns 0. In other words,
    /// `bytes_mut` returning an empty slice implies that `remaining_mut` will
    /// return 0 and `remaining_mut` returning 0 implies that `bytes_mut` will
    /// return an empty slice.
    fn bytes_mut(&mut self) -> &mut [MaybeUninit<u8>];

    /// Fills `dst` with potentially multiple mutable slices starting at `self`'s
    /// current position.
    ///
    /// If the `BufMut` is backed by disjoint slices of bytes, `bytes_vectored_mut`
    /// enables fetching more than one slice at once. `dst` is a slice of
    /// mutable `IoSliceMut` references, enabling the slice to be directly used with
    /// [`readv`] without any further conversion. The sum of the lengths of all
    /// the buffers in `dst` will be less than or equal to
    /// `Buf::remaining_mut()`.
    ///
    /// The entries in `dst` will be overwritten, but the data **contained** by
    /// the slices **will not** be modified. If `bytes_vectored_mut` does not fill every
    /// entry in `dst`, then `dst` is guaranteed to contain all remaining slices
    /// in `self.
    ///
    /// This is a lower level function. Most operations are done with other
    /// functions.
    ///
    /// # Implementer notes
    ///
    /// This function should never panic. Once the end of the buffer is reached,
    /// i.e., `BufMut::remaining_mut` returns 0, calls to `bytes_vectored_mut` must
    /// return 0 without mutating `dst`.
    ///
    /// Implementations should also take care to properly handle being called
    /// with `dst` being a zero length slice.
    ///
    /// [`readv`]: http://man7.org/linux/man-pages/man2/readv.2.html
    #[cfg(feature = "std")]
    fn bytes_vectored_mut<'a>(&'a mut self, dst: &mut [IoSliceMut<'a>]) -> usize {
        if dst.is_empty() {
            return 0;
        }

        if self.has_remaining_mut() {
            dst[0] = IoSliceMut::from(self.bytes_mut());
            1
        } else {
            0
        }
    }

    /// Transfer bytes into `self` from `src` and advance the cursor by the
    /// number of bytes written.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    ///
    /// buf.put_u8(b'h');
    /// buf.put(&b"ello"[..]);
    /// buf.put(&b" world"[..]);
    ///
    /// assert_eq!(buf, b"hello world");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `self` does not have enough capacity to contain `src`.
    fn put<T: super::Buf>(&mut self, mut src: T)
    where
        Self: Sized,
    {
        assert!(self.remaining_mut() >= src.remaining());

        while src.has_remaining() {
            let l;

            unsafe {
                let s = src.bytes();
                let d = self.bytes_mut();
                l = cmp::min(s.len(), d.len());

                ptr::copy_nonoverlapping(s.as_ptr(), d.as_mut_ptr() as *mut u8, l);
            }

            src.advance(l);
            unsafe {
                self.advance_mut(l);
            }
        }
    }

    /// Transfer bytes into `self` from `src` and advance the cursor by the
    /// number of bytes written.
    ///
    /// `self` must have enough remaining capacity to contain all of `src`.
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut dst = [0; 6];
    ///
    /// {
    ///     let mut buf = &mut dst[..];
    ///     buf.put_slice(b"hello");
    ///
    ///     assert_eq!(1, buf.remaining_mut());
    /// }
    ///
    /// assert_eq!(b"hello\0", &dst);
    /// ```
    fn put_slice(&mut self, src: &[u8]) {
        let mut off = 0;

        assert!(
            self.remaining_mut() >= src.len(),
            "buffer overflow; remaining = {}; src = {}",
            self.remaining_mut(),
            src.len()
        );

        while off < src.len() {
            let cnt;

            unsafe {
                let dst = self.bytes_mut();
                cnt = cmp::min(dst.len(), src.len() - off);

                ptr::copy_nonoverlapping(src[off..].as_ptr(), dst.as_mut_ptr() as *mut u8, cnt);

                off += cnt;
            }

            unsafe {
                self.advance_mut(cnt);
            }
        }
    }

    /// Writes an unsigned 8 bit integer to `self`.
    ///
    /// The current position is advanced by 1.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u8(0x01);
    /// assert_eq!(buf, b"\x01");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_u8(&mut self, n: u8) {
        let src = [n];
        self.put_slice(&src);
    }

    /// Writes a signed 8 bit integer to `self`.
    ///
    /// The current position is advanced by 1.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i8(0x01);
    /// assert_eq!(buf, b"\x01");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_i8(&mut self, n: i8) {
        let src = [n as u8];
        self.put_slice(&src)
    }

    /// Writes an unsigned 16 bit integer to `self` in big-endian byte order.
    ///
    /// The current position is advanced by 2.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u16(0x0809);
    /// assert_eq!(buf, b"\x08\x09");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_u16(&mut self, n: u16) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Writes an unsigned 16 bit integer to `self` in little-endian byte order.
    ///
    /// The current position is advanced by 2.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u16_le(0x0809);
    /// assert_eq!(buf, b"\x09\x08");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_u16_le(&mut self, n: u16) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Writes a signed 16 bit integer to `self` in big-endian byte order.
    ///
    /// The current position is advanced by 2.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i16(0x0809);
    /// assert_eq!(buf, b"\x08\x09");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_i16(&mut self, n: i16) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Writes a signed 16 bit integer to `self` in little-endian byte order.
    ///
    /// The current position is advanced by 2.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i16_le(0x0809);
    /// assert_eq!(buf, b"\x09\x08");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_i16_le(&mut self, n: i16) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Writes an unsigned 32 bit integer to `self` in big-endian byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u32(0x0809A0A1);
    /// assert_eq!(buf, b"\x08\x09\xA0\xA1");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_u32(&mut self, n: u32) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Writes an unsigned 32 bit integer to `self` in little-endian byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u32_le(0x0809A0A1);
    /// assert_eq!(buf, b"\xA1\xA0\x09\x08");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_u32_le(&mut self, n: u32) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Writes a signed 32 bit integer to `self` in big-endian byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i32(0x0809A0A1);
    /// assert_eq!(buf, b"\x08\x09\xA0\xA1");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_i32(&mut self, n: i32) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Writes a signed 32 bit integer to `self` in little-endian byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i32_le(0x0809A0A1);
    /// assert_eq!(buf, b"\xA1\xA0\x09\x08");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_i32_le(&mut self, n: i32) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Writes an unsigned 64 bit integer to `self` in the big-endian byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u64(0x0102030405060708);
    /// assert_eq!(buf, b"\x01\x02\x03\x04\x05\x06\x07\x08");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_u64(&mut self, n: u64) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Writes an unsigned 64 bit integer to `self` in little-endian byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u64_le(0x0102030405060708);
    /// assert_eq!(buf, b"\x08\x07\x06\x05\x04\x03\x02\x01");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_u64_le(&mut self, n: u64) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Writes a signed 64 bit integer to `self` in the big-endian byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i64(0x0102030405060708);
    /// assert_eq!(buf, b"\x01\x02\x03\x04\x05\x06\x07\x08");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_i64(&mut self, n: i64) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Writes a signed 64 bit integer to `self` in little-endian byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i64_le(0x0102030405060708);
    /// assert_eq!(buf, b"\x08\x07\x06\x05\x04\x03\x02\x01");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_i64_le(&mut self, n: i64) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Writes an unsigned 128 bit integer to `self` in the big-endian byte order.
    ///
    /// The current position is advanced by 16.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u128(0x01020304050607080910111213141516);
    /// assert_eq!(buf, b"\x01\x02\x03\x04\x05\x06\x07\x08\x09\x10\x11\x12\x13\x14\x15\x16");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_u128(&mut self, n: u128) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Writes an unsigned 128 bit integer to `self` in little-endian byte order.
    ///
    /// The current position is advanced by 16.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u128_le(0x01020304050607080910111213141516);
    /// assert_eq!(buf, b"\x16\x15\x14\x13\x12\x11\x10\x09\x08\x07\x06\x05\x04\x03\x02\x01");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_u128_le(&mut self, n: u128) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Writes a signed 128 bit integer to `self` in the big-endian byte order.
    ///
    /// The current position is advanced by 16.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i128(0x01020304050607080910111213141516);
    /// assert_eq!(buf, b"\x01\x02\x03\x04\x05\x06\x07\x08\x09\x10\x11\x12\x13\x14\x15\x16");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_i128(&mut self, n: i128) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Writes a signed 128 bit integer to `self` in little-endian byte order.
    ///
    /// The current position is advanced by 16.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i128_le(0x01020304050607080910111213141516);
    /// assert_eq!(buf, b"\x16\x15\x14\x13\x12\x11\x10\x09\x08\x07\x06\x05\x04\x03\x02\x01");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_i128_le(&mut self, n: i128) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Writes an unsigned n-byte integer to `self` in big-endian byte order.
    ///
    /// The current position is advanced by `nbytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_uint(0x010203, 3);
    /// assert_eq!(buf, b"\x01\x02\x03");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_uint(&mut self, n: u64, nbytes: usize) {
        self.put_slice(&n.to_be_bytes()[mem::size_of_val(&n) - nbytes..]);
    }

    /// Writes an unsigned n-byte integer to `self` in the little-endian byte order.
    ///
    /// The current position is advanced by `nbytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_uint_le(0x010203, 3);
    /// assert_eq!(buf, b"\x03\x02\x01");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_uint_le(&mut self, n: u64, nbytes: usize) {
        self.put_slice(&n.to_le_bytes()[0..nbytes]);
    }

    /// Writes a signed n-byte integer to `self` in big-endian byte order.
    ///
    /// The current position is advanced by `nbytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_int(0x010203, 3);
    /// assert_eq!(buf, b"\x01\x02\x03");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_int(&mut self, n: i64, nbytes: usize) {
        self.put_slice(&n.to_be_bytes()[mem::size_of_val(&n) - nbytes..]);
    }

    /// Writes a signed n-byte integer to `self` in little-endian byte order.
    ///
    /// The current position is advanced by `nbytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_int_le(0x010203, 3);
    /// assert_eq!(buf, b"\x03\x02\x01");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_int_le(&mut self, n: i64, nbytes: usize) {
        self.put_slice(&n.to_le_bytes()[0..nbytes]);
    }

    /// Writes  an IEEE754 single-precision (4 bytes) floating point number to
    /// `self` in big-endian byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_f32(1.2f32);
    /// assert_eq!(buf, b"\x3F\x99\x99\x9A");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_f32(&mut self, n: f32) {
        self.put_u32(n.to_bits());
    }

    /// Writes  an IEEE754 single-precision (4 bytes) floating point number to
    /// `self` in little-endian byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_f32_le(1.2f32);
    /// assert_eq!(buf, b"\x9A\x99\x99\x3F");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_f32_le(&mut self, n: f32) {
        self.put_u32_le(n.to_bits());
    }

    /// Writes  an IEEE754 double-precision (8 bytes) floating point number to
    /// `self` in big-endian byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_f64(1.2f64);
    /// assert_eq!(buf, b"\x3F\xF3\x33\x33\x33\x33\x33\x33");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_f64(&mut self, n: f64) {
        self.put_u64(n.to_bits());
    }

    /// Writes  an IEEE754 double-precision (8 bytes) floating point number to
    /// `self` in little-endian byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_f64_le(1.2f64);
    /// assert_eq!(buf, b"\x33\x33\x33\x33\x33\x33\xF3\x3F");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_f64_le(&mut self, n: f64) {
        self.put_u64_le(n.to_bits());
    }
}

macro_rules! deref_forward_bufmut {
    () => {
        fn remaining_mut(&self) -> usize {
            (**self).remaining_mut()
        }

        fn bytes_mut(&mut self) -> &mut [MaybeUninit<u8>] {
            (**self).bytes_mut()
        }

        #[cfg(feature = "std")]
        fn bytes_vectored_mut<'b>(&'b mut self, dst: &mut [IoSliceMut<'b>]) -> usize {
            (**self).bytes_vectored_mut(dst)
        }

        unsafe fn advance_mut(&mut self, cnt: usize) {
            (**self).advance_mut(cnt)
        }

        fn put_slice(&mut self, src: &[u8]) {
            (**self).put_slice(src)
        }

        fn put_u8(&mut self, n: u8) {
            (**self).put_u8(n)
        }

        fn put_i8(&mut self, n: i8) {
            (**self).put_i8(n)
        }

        fn put_u16(&mut self, n: u16) {
            (**self).put_u16(n)
        }

        fn put_u16_le(&mut self, n: u16) {
            (**self).put_u16_le(n)
        }

        fn put_i16(&mut self, n: i16) {
            (**self).put_i16(n)
        }

        fn put_i16_le(&mut self, n: i16) {
            (**self).put_i16_le(n)
        }

        fn put_u32(&mut self, n: u32) {
            (**self).put_u32(n)
        }

        fn put_u32_le(&mut self, n: u32) {
            (**self).put_u32_le(n)
        }

        fn put_i32(&mut self, n: i32) {
            (**self).put_i32(n)
        }

        fn put_i32_le(&mut self, n: i32) {
            (**self).put_i32_le(n)
        }

        fn put_u64(&mut self, n: u64) {
            (**self).put_u64(n)
        }

        fn put_u64_le(&mut self, n: u64) {
            (**self).put_u64_le(n)
        }

        fn put_i64(&mut self, n: i64) {
            (**self).put_i64(n)
        }

        fn put_i64_le(&mut self, n: i64) {
            (**self).put_i64_le(n)
        }
    };
}

impl<T: BufMut + ?Sized> BufMut for &mut T {
    deref_forward_bufmut!();
}

impl<T: BufMut + ?Sized> BufMut for Box<T> {
    deref_forward_bufmut!();
}

impl BufMut for &mut [u8] {
    #[inline]
    fn remaining_mut(&self) -> usize {
        self.len()
    }

    #[inline]
    fn bytes_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        // MaybeUninit is repr(transparent), so safe to transmute
        unsafe { mem::transmute(&mut **self) }
    }

    #[inline]
    unsafe fn advance_mut(&mut self, cnt: usize) {
        // Lifetime dance taken from `impl Write for &mut [u8]`.
        let (_, b) = core::mem::replace(self, &mut []).split_at_mut(cnt);
        *self = b;
    }
}

impl BufMut for Vec<u8> {
    #[inline]
    fn remaining_mut(&self) -> usize {
        usize::MAX - self.len()
    }

    #[inline]
    unsafe fn advance_mut(&mut self, cnt: usize) {
        let len = self.len();
        let remaining = self.capacity() - len;

        assert!(
            cnt <= remaining,
            "cannot advance past `remaining_mut`: {:?} <= {:?}",
            cnt,
            remaining
        );

        self.set_len(len + cnt);
    }

    #[inline]
    fn bytes_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        use core::slice;

        if self.capacity() == self.len() {
            self.reserve(64); // Grow the vec
        }

        let cap = self.capacity();
        let len = self.len();

        let ptr = self.as_mut_ptr() as *mut MaybeUninit<u8>;
        unsafe { &mut slice::from_raw_parts_mut(ptr, cap)[len..] }
    }

    // Specialize these methods so they can skip checking `remaining_mut`
    // and `advance_mut`.

    fn put<T: super::Buf>(&mut self, mut src: T)
    where
        Self: Sized,
    {
        // In case the src isn't contiguous, reserve upfront
        self.reserve(src.remaining());

        while src.has_remaining() {
            let l;

            // a block to contain the src.bytes() borrow
            {
                let s = src.bytes();
                l = s.len();
                self.extend_from_slice(s);
            }

            src.advance(l);
        }
    }

    fn put_slice(&mut self, src: &[u8]) {
        self.extend_from_slice(src);
    }
}

// The existence of this function makes the compiler catch if the BufMut
// trait is "object-safe" or not.
fn _assert_trait_object(_b: &dyn BufMut) {}

// ===== impl IoSliceMut =====

/// A buffer type used for `readv`.
///
/// This is a wrapper around an `std::io::IoSliceMut`, but does not expose
/// the inner bytes in a safe API, as they may point at uninitialized memory.
///
/// This is `repr(transparent)` of the `std::io::IoSliceMut`, so it is valid to
/// transmute them. However, as the memory might be uninitialized, care must be
/// taken to not *read* the internal bytes, only *write* to them.
#[repr(transparent)]
#[cfg(feature = "std")]
pub struct IoSliceMut<'a>(std::io::IoSliceMut<'a>);

#[cfg(feature = "std")]
impl fmt::Debug for IoSliceMut<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IoSliceMut")
            .field("len", &self.0.len())
            .finish()
    }
}

#[cfg(feature = "std")]
impl<'a> From<&'a mut [u8]> for IoSliceMut<'a> {
    fn from(buf: &'a mut [u8]) -> IoSliceMut<'a> {
        IoSliceMut(std::io::IoSliceMut::new(buf))
    }
}

#[cfg(feature = "std")]
impl<'a> From<&'a mut [MaybeUninit<u8>]> for IoSliceMut<'a> {
    fn from(buf: &'a mut [MaybeUninit<u8>]) -> IoSliceMut<'a> {
        IoSliceMut(std::io::IoSliceMut::new(unsafe {
            // We don't look at the contents, and `std::io::IoSliceMut`
            // doesn't either.
            mem::transmute::<&'a mut [MaybeUninit<u8>], &'a mut [u8]>(buf)
        }))
    }
}
