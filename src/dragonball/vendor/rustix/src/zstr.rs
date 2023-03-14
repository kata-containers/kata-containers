/// A macro for [`ZStr`] literals.
///
/// This can make passing string literals to rustix APIs more efficient, since
/// most underlying system calls with string arguments expect NUL-terminated
/// strings, and passing strings to rustix as `ZStr`s means that rustix doesn't
/// need to copy them into a separate buffer to NUL-terminate them.
///
/// [`ZStr`]: crate::ffi::ZStr
///
/// # Examples
///
/// ```rust,no_run
/// # fn main() -> rustix::io::Result<()> {
/// use rustix::fs::{cwd, statat, AtFlags};
/// use rustix::zstr;
///
/// let metadata = statat(&cwd(), zstr!("test.txt"), AtFlags::empty())?;
/// # Ok(())
/// # }
/// ```
#[allow(unused_macros)]
#[macro_export]
macro_rules! zstr {
    ($str:literal) => {{
        // Check for NUL manually, to ensure safety.
        //
        // In release builds, with strings that don't contain NULs, this
        // constant-folds away.
        //
        // We don't use std's `CStr::from_bytes_with_nul`; as of this writing,
        // that function isn't defined as `#[inline]` in std and doesn't
        // constant-fold away.
        assert!(
            !$str.bytes().any(|b| b == b'\0'),
            "zstr argument contains embedded NUL bytes",
        );

        // Now that we know the string doesn't have embedded NULs, we can call
        // `from_bytes_with_nul_unchecked`, which as of this writing is defined
        // as `#[inline]` and completely optimizes away.
        //
        // # Safety
        //
        // We have manually checked that the string does not contain embedded
        // NULs above, and we append or own NUL terminator here.
        #[allow(unsafe_code)]
        unsafe {
            $crate::ffi::ZStr::from_bytes_with_nul_unchecked(concat!($str, "\0").as_bytes())
        }
    }};
}

#[test]
fn test_zstr() {
    use crate::ffi::ZString;
    use alloc::borrow::ToOwned;
    assert_eq!(zstr!(""), &*ZString::new("").unwrap());
    assert_eq!(zstr!("").to_owned(), ZString::new("").unwrap());
    assert_eq!(zstr!("hello"), &*ZString::new("hello").unwrap());
    assert_eq!(zstr!("hello").to_owned(), ZString::new("hello").unwrap());
}

#[test]
#[should_panic]
fn test_invalid_zstr() {
    let _ = zstr!("hello\0world");
}
