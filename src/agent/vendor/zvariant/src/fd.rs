use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};
use static_assertions::assert_impl_all;
use std::os::unix::io;

use crate::{Basic, EncodingFormat, Signature, Type};

/// A [`RawFd`](https://doc.rust-lang.org/std/os/unix/io/type.RawFd.html) wrapper.
///
/// See also `OwnedFd` if you need a wrapper that takes ownership of the file.
///
/// We wrap the `RawFd` type so that we can implement [`Serialize`] and [`Deserialize`] for it.
/// File descriptors are serialized in a special way and you need to use specific [serializer] and
/// [deserializer] API when file descriptors are or could be involved.
///
/// [`Serialize`]: https://docs.serde.rs/serde/trait.Serialize.html
/// [`Deserialize`]: https://docs.serde.rs/serde/de/trait.Deserialize.html
/// [deserializer]: fn.from_slice_fds.html
/// [serializer]: fn.to_bytes_fds.html
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct Fd(io::RawFd);

macro_rules! fd_impl {
    ($i:ident) => {
        assert_impl_all!($i: Send, Sync, Unpin);

        impl Basic for $i {
            const SIGNATURE_CHAR: char = 'h';
            const SIGNATURE_STR: &'static str = "h";

            fn alignment(format: EncodingFormat) -> usize {
                u32::alignment(format)
            }
        }

        impl Type for $i {
            fn signature() -> Signature<'static> {
                Signature::from_static_str_unchecked(Self::SIGNATURE_STR)
            }
        }
    };
}

fd_impl!(Fd);

impl Serialize for Fd {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i32(self.0)
    }
}

impl<'de> Deserialize<'de> for Fd {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Fd(i32::deserialize(deserializer)?))
    }
}

impl From<io::RawFd> for Fd {
    fn from(value: io::RawFd) -> Self {
        Self(value)
    }
}

impl<T> From<&T> for Fd
where
    T: io::AsRawFd,
{
    fn from(t: &T) -> Self {
        Self(t.as_raw_fd())
    }
}

impl io::AsRawFd for Fd {
    fn as_raw_fd(&self) -> io::RawFd {
        self.0
    }
}

impl std::fmt::Display for Fd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// An owned [`RawFd`](https://doc.rust-lang.org/std/os/unix/io/type.RawFd.html) wrapper.
///
/// See also [`Fd`]. This type owns the file and will close it on drop. On deserialize, it will
/// duplicate the file descriptor.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct OwnedFd {
    inner: io::RawFd,
}

impl Drop for OwnedFd {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.inner);
        }
    }
}

fd_impl!(OwnedFd);

impl Serialize for OwnedFd {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i32(self.inner)
    }
}

impl<'de> Deserialize<'de> for OwnedFd {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fd = unsafe { libc::dup(i32::deserialize(deserializer)?) };
        if fd < 0 {
            return Err(D::Error::custom(std::io::Error::last_os_error()));
        }
        Ok(OwnedFd { inner: fd })
    }
}

impl io::FromRawFd for OwnedFd {
    unsafe fn from_raw_fd(fd: io::RawFd) -> Self {
        Self { inner: fd }
    }
}

impl io::AsRawFd for OwnedFd {
    fn as_raw_fd(&self) -> io::RawFd {
        self.inner
    }
}

impl io::IntoRawFd for OwnedFd {
    fn into_raw_fd(self) -> io::RawFd {
        let fd = self.inner;
        std::mem::forget(self);
        fd
    }
}

impl std::fmt::Display for OwnedFd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}
