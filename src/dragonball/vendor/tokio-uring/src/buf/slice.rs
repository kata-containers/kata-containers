use crate::buf::{IoBuf, IoBufMut};

use std::ops;

/// An owned view into a contiguous sequence of bytes.
///
/// This is similar to Rust slices (`&buf[..]`) but owns the underlying buffer.
/// This type is useful for performing io-uring read and write operations using
/// a subset of a buffer.
///
/// Slices are created using [`IoBuf::slice`].
///
/// # Examples
///
/// Creating a slice
///
/// ```
/// use tokio_uring::buf::IoBuf;
///
/// let buf = b"hello world".to_vec();
/// let slice = buf.slice(..5);
///
/// assert_eq!(&slice[..], b"hello");
/// ```
pub struct Slice<T> {
    buf: T,
    begin: usize,
    end: usize,
}

impl<T> Slice<T> {
    pub(crate) fn new(buf: T, begin: usize, end: usize) -> Slice<T> {
        Slice { buf, begin, end }
    }

    /// Offset in the underlying buffer at which this slice starts.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_uring::buf::IoBuf;
    ///
    /// let buf = b"hello world".to_vec();
    /// let slice = buf.slice(1..5);
    ///
    /// assert_eq!(1, slice.begin());
    /// ```
    pub fn begin(&self) -> usize {
        self.begin
    }

    /// Ofset in the underlying buffer at which this slice ends.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_uring::buf::IoBuf;
    ///
    /// let buf = b"hello world".to_vec();
    /// let slice = buf.slice(1..5);
    ///
    /// assert_eq!(5, slice.end());
    /// ```
    pub fn end(&self) -> usize {
        self.end
    }

    /// Gets a reference to the underlying buffer.
    ///
    /// This method escapes the slice's view.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_uring::buf::IoBuf;
    ///
    /// let buf = b"hello world".to_vec();
    /// let slice = buf.slice(..5);
    ///
    /// assert_eq!(slice.get_ref(), b"hello world");
    /// assert_eq!(&slice[..], b"hello");
    /// ```
    pub fn get_ref(&self) -> &T {
        &self.buf
    }

    /// Gets a mutable reference to the underlying buffer.
    ///
    /// This method escapes the slice's view.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_uring::buf::IoBuf;
    ///
    /// let buf = b"hello world".to_vec();
    /// let mut slice = buf.slice(..5);
    ///
    /// slice.get_mut()[0] = b'b';
    ///
    /// assert_eq!(slice.get_mut(), b"bello world");
    /// assert_eq!(&slice[..], b"bello");
    /// ```
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.buf
    }

    /// Unwraps this `Slice`, returning the underlying buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_uring::buf::IoBuf;
    ///
    /// let buf = b"hello world".to_vec();
    /// let slice = buf.slice(..5);
    ///
    /// let buf = slice.into_inner();
    /// assert_eq!(buf, b"hello world");
    /// ```
    pub fn into_inner(self) -> T {
        self.buf
    }
}

impl<T: IoBuf> ops::Deref for Slice<T> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &super::deref(&self.buf)[self.begin..self.end]
    }
}

impl<T: IoBufMut> ops::DerefMut for Slice<T> {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut super::deref_mut(&mut self.buf)[self.begin..self.end]
    }
}

unsafe impl<T: IoBuf> IoBuf for Slice<T> {
    fn stable_ptr(&self) -> *const u8 {
        ops::Deref::deref(self).as_ptr()
    }

    fn bytes_init(&self) -> usize {
        ops::Deref::deref(self).len()
    }

    fn bytes_total(&self) -> usize {
        self.end - self.begin
    }
}

unsafe impl<T: IoBufMut> IoBufMut for Slice<T> {
    fn stable_mut_ptr(&mut self) -> *mut u8 {
        ops::DerefMut::deref_mut(self).as_mut_ptr()
    }

    unsafe fn set_init(&mut self, pos: usize) {
        self.buf.set_init(self.begin + pos);
    }
}
