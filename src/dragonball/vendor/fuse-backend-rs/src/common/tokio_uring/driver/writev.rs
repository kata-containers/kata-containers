use crate::{
    buf::IoBuf,
    driver::{Op, SharedFd},
    BufResult,
};
use libc::iovec;
use std::{
    io,
    task::{Context, Poll},
};

pub(crate) struct Writev<T> {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(dead_code)]
    fd: SharedFd,

    pub(crate) bufs: Vec<T>,

    /// Parameter for `io_uring::op::readv`, referring `bufs`.
    iovs: Vec<iovec>,
}

impl<T: IoBuf> Op<Writev<T>> {
    pub(crate) fn writev_at(
        fd: &SharedFd,
        mut bufs: Vec<T>,
        offset: u64,
    ) -> io::Result<Op<Writev<T>>> {
        use io_uring::{opcode, types};

        // Build `iovec` objects referring the provided `bufs` for `io_uring::opcode::Readv`.
        let iovs: Vec<iovec> = bufs
            .iter_mut()
            .map(|b| iovec {
                iov_base: b.stable_ptr() as *mut libc::c_void,
                iov_len: b.bytes_init(),
            })
            .collect();

        Op::submit_with(
            Writev {
                fd: fd.clone(),
                bufs,
                iovs,
            },
            |write| {
                opcode::Writev::new(
                    types::Fd(fd.raw_fd()),
                    write.iovs.as_ptr(),
                    write.iovs.len() as u32,
                )
                .offset(offset as _)
                .build()
            },
        )
    }

    pub(crate) async fn writev(mut self) -> BufResult<usize, Vec<T>> {
        use crate::future::poll_fn;

        poll_fn(move |cx| self.poll_writev(cx)).await
    }

    pub(crate) fn poll_writev(&mut self, cx: &mut Context<'_>) -> Poll<BufResult<usize, Vec<T>>> {
        use std::future::Future;
        use std::pin::Pin;

        let complete = ready!(Pin::new(self).poll(cx));
        Poll::Ready((complete.result.map(|v| v as _), complete.data.bufs))
    }
}
