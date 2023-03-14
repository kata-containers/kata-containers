//! Split a single value implementing `AsyncRead + AsyncWrite` into separate
//! `AsyncRead` and `AsyncWrite` handles.
//!
//! To restore this read/write object from its `split::ReadHalf` and
//! `split::WriteHalf` use `unsplit`.

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::VsockStream;
use futures::ready;
use std::cell::UnsafeCell;
use std::fmt;
use std::io;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::{Acquire, Release};
use std::sync::Arc;
use std::task::{Context, Poll};

/// The readable half of a value returned from [`split`](split()).
pub struct ReadHalf {
    inner: Arc<Inner>,
}

/// The writable half of a value returned from [`split`](split()).
pub struct WriteHalf {
    inner: Arc<Inner>,
}

pub fn split(stream: VsockStream) -> (ReadHalf, WriteHalf) {
    let inner = Arc::new(Inner {
        locked: AtomicBool::new(false),
        stream: UnsafeCell::new(stream),
    });

    let rd = ReadHalf {
        inner: inner.clone(),
    };

    let wr = WriteHalf { inner };

    (rd, wr)
}

struct Inner {
    locked: AtomicBool,
    stream: UnsafeCell<VsockStream>,
}

struct Guard<'a> {
    inner: &'a Inner,
}

impl ReadHalf {
    /// Checks if this `ReadHalf` and some `WriteHalf` were split from the same
    /// stream.
    pub fn is_pair_of(&self, other: &WriteHalf) -> bool {
        other.is_pair_of(self)
    }

    /// Reunites with a previously split `WriteHalf`.
    ///
    /// # Panics
    ///
    /// If this `ReadHalf` and the given `WriteHalf` do not originate from the
    /// same `split` operation this method will panic.
    /// This can be checked ahead of time by comparing the stream ID
    /// of the two halves.
    #[track_caller]
    pub fn unsplit(self, wr: WriteHalf) -> VsockStream {
        if self.is_pair_of(&wr) {
            drop(wr);

            let inner = Arc::try_unwrap(self.inner)
                .ok()
                .expect("`Arc::try_unwrap` failed");

            inner.stream.into_inner()
        } else {
            panic!("Unrelated `split::Write` passed to `split::Read::unsplit`.")
        }
    }
}

impl WriteHalf {
    /// Checks if this `WriteHalf` and some `ReadHalf` were split from the same
    /// stream.
    pub fn is_pair_of(&self, other: &ReadHalf) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl AsyncRead for ReadHalf {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let mut inner = ready!(self.inner.poll_lock(cx));
        inner.stream_pin().poll_read(cx, buf)
    }
}

impl AsyncWrite for WriteHalf {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let mut inner = ready!(self.inner.poll_lock(cx));
        inner.stream_pin().poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let mut inner = ready!(self.inner.poll_lock(cx));
        inner.stream_pin().poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let mut inner = ready!(self.inner.poll_lock(cx));
        inner.stream_pin().poll_shutdown(cx)
    }
}

impl Inner {
    fn poll_lock(&self, cx: &mut Context<'_>) -> Poll<Guard<'_>> {
        if self
            .locked
            .compare_exchange(false, true, Acquire, Acquire)
            .is_ok()
        {
            Poll::Ready(Guard { inner: self })
        } else {
            // Spin... but investigate a better strategy

            std::thread::yield_now();
            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }
}

impl Guard<'_> {
    fn stream_pin(&mut self) -> Pin<&mut VsockStream> {
        // safety: the stream is pinned in `Arc` and the `Guard` ensures mutual
        // exclusion.
        unsafe { Pin::new_unchecked(&mut *self.inner.stream.get()) }
    }
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.inner.locked.store(false, Release);
    }
}

unsafe impl Send for ReadHalf {}
unsafe impl Send for WriteHalf {}
unsafe impl Sync for ReadHalf {}
unsafe impl Sync for WriteHalf {}

impl fmt::Debug for ReadHalf {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("split::ReadHalf").finish()
    }
}

impl fmt::Debug for WriteHalf {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("split::WriteHalf").finish()
    }
}
