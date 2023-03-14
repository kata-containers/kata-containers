// Copyright (c) 2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

//! Because Tokio has removed UnixIncoming since version 0.3,
//! we define the UnixIncoming and implement the Stream for UnixIncoming.

use std::io;
use std::os::unix::io::{AsRawFd, RawFd};
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{ready, Stream};
use tokio::net::{UnixListener, UnixStream};

/// Stream of listeners
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct UnixIncoming {
    inner: UnixListener,
}

impl UnixIncoming {
    pub fn new(listener: UnixListener) -> Self {
        Self { inner: listener }
    }
}

impl Stream for UnixIncoming {
    type Item = io::Result<UnixStream>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let (socket, _) = ready!(self.inner.poll_accept(cx))?;
        Poll::Ready(Some(Ok(socket)))
    }
}

impl AsRawFd for UnixIncoming {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}
