use core::fmt;
use core::ops::{Deref, DerefMut};

/// Pads and aligns a value to the length of a cache line.
///
/// In concurrent programming, sometimes it is desirable to make sure commonly accessed pieces of
/// data are not placed into the same cache line. Updating an atomic value invalides the whole
/// cache line it belongs to, which makes the next access to the same cache line slower for other
/// CPU cores. Use `CachePadded` to ensure updating one piece of data doesn't invalidate other
/// cached data.
///
/// # Size and alignment
///
/// Cache lines are assumed to be N bytes long, depending on the architecture:
///
/// * On x86-64, N = 128.
/// * On all others, N = 64.
///
/// Note that N is just a reasonable guess and is not guaranteed to match the actual cache line
/// length of the machine the program is running on. On modern Intel architectures, spatial
/// prefetcher is pulling pairs of 64-byte cache lines at a time, so we pessimistically assume that
/// cache lines are 128 bytes long.
///
/// The size of `CachePadded<T>` is the smallest multiple of N bytes large enough to accommodate
/// a value of type `T`.
///
/// The alignment of `CachePadded<T>` is the maximum of N bytes and the alignment of `T`.
///
/// # Examples
///
/// Alignment and padding:
///
/// ```
/// use crossbeam_utils::CachePadded;
///
/// let array = [CachePadded::new(1i8), CachePadded::new(2i8)];
/// let addr1 = &*array[0] as *const i8 as usize;
/// let addr2 = &*array[1] as *const i8 as usize;
///
/// assert!(addr2 - addr1 >= 64);
/// assert_eq!(addr1 % 64, 0);
/// assert_eq!(addr2 % 64, 0);
/// ```
///
/// When building a concurrent queue with a head and a tail index, it is wise to place them in
/// different cache lines so that concurrent threads pushing and popping elements don't invalidate
/// each other's cache lines:
///
/// ```
/// use crossbeam_utils::CachePadded;
/// use std::sync::atomic::AtomicUsize;
///
/// struct Queue<T> {
///     head: CachePadded<AtomicUsize>,
///     tail: CachePadded<AtomicUsize>,
///     buffer: *mut T,
/// }
/// ```
#[derive(Clone, Copy, Default, Hash, PartialEq, Eq)]
// Starting from Intel's Sandy Bridge, spatial prefetcher is now pulling pairs of 64-byte cache
// lines at a time, so we have to align to 128 bytes rather than 64.
//
// Sources:
// - https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-optimization-manual.pdf
// - https://github.com/facebook/folly/blob/1b5288e6eea6df074758f877c849b6e73bbb9fbb/folly/lang/Align.h#L107
#[cfg_attr(target_arch = "x86_64", repr(align(128)))]
#[cfg_attr(not(target_arch = "x86_64"), repr(align(64)))]
pub struct CachePadded<T> {
    value: T,
}

unsafe impl<T: Send> Send for CachePadded<T> {}
unsafe impl<T: Sync> Sync for CachePadded<T> {}

impl<T> CachePadded<T> {
    /// Pads and aligns a value to the length of a cache line.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::CachePadded;
    ///
    /// let padded_value = CachePadded::new(1);
    /// ```
    pub fn new(t: T) -> CachePadded<T> {
        CachePadded::<T> { value: t }
    }

    /// Returns the inner value.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::CachePadded;
    ///
    /// let padded_value = CachePadded::new(7);
    /// let value = padded_value.into_inner();
    /// assert_eq!(value, 7);
    /// ```
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> Deref for CachePadded<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> DerefMut for CachePadded<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

impl<T: fmt::Debug> fmt::Debug for CachePadded<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CachePadded")
            .field("value", &self.value)
            .finish()
    }
}

impl<T> From<T> for CachePadded<T> {
    fn from(t: T) -> Self {
        CachePadded::new(t)
    }
}
