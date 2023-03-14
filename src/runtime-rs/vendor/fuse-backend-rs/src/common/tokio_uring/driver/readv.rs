use crate::buf::IoBufMut;
use crate::driver::{Op, SharedFd};
use crate::BufResult;

use libc::iovec;
use std::io;
use std::task::{Context, Poll};

pub(crate) struct Readv<T> {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(dead_code)]
    fd: SharedFd,

    /// Reference to the in-flight buffer.
    pub(crate) bufs: Vec<T>,
    /// Parameter for `io_uring::op::readv`, referring `bufs`.
    iovs: Vec<iovec>,
}

impl<T: IoBufMut> Op<Readv<T>> {
    pub(crate) fn readv_at(
        fd: &SharedFd,
        mut bufs: Vec<T>,
        offset: u64,
    ) -> io::Result<Op<Readv<T>>> {
        use io_uring::{opcode, types};

        // Build `iovec` objects referring the provided `bufs` for `io_uring::opcode::Readv`.
        let iovs: Vec<iovec> = bufs
            .iter_mut()
            .map(|b| iovec {
                // Safety guaranteed by `IoBufMut`.
                iov_base: unsafe { b.stable_mut_ptr().add(b.bytes_init()) as *mut libc::c_void },
                iov_len: b.bytes_total() - b.bytes_init(),
            })
            .collect();

        Op::submit_with(
            Readv {
                fd: fd.clone(),
                bufs,
                iovs,
            },
            |read| {
                opcode::Readv::new(
                    types::Fd(fd.raw_fd()),
                    read.iovs.as_ptr(),
                    read.iovs.len() as u32,
                )
                .offset(offset as _)
                .build()
            },
        )
    }

    pub(crate) async fn readv(mut self) -> BufResult<usize, Vec<T>> {
        crate::future::poll_fn(move |cx| self.poll_readv(cx)).await
    }

    pub(crate) fn poll_readv(&mut self, cx: &mut Context<'_>) -> Poll<BufResult<usize, Vec<T>>> {
        use std::future::Future;
        use std::pin::Pin;

        let complete = ready!(Pin::new(self).poll(cx));

        // Convert the operation result to `usize`
        let res = complete.result.map(|v| v as usize);
        // Recover the buffers
        let mut bufs = complete.data.bufs;

        // If the operation was successful, advance the initialized cursor.
        if let Ok(n) = res {
            let mut count = n;
            for b in bufs.iter_mut() {
                let sz = std::cmp::min(count, b.bytes_total() - b.bytes_init());
                let pos = b.bytes_init() + sz;
                // Safety: the kernel returns bytes written, and we have ensured that `pos` is
                // valid for current buffer.
                unsafe { b.set_init(pos) };
                count = count - sz;
                if count == 0 {
                    break;
                }
            }
            assert_eq!(count, 0);
        }

        Poll::Ready((res, bufs))
    }
}
