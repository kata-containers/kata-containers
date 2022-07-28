// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    io,
    os::unix::{io::FromRawFd, net::UnixStream as StdUnixStream},
    pin::Pin,
    task::Context as TaskContext,
    task::Poll,
};

use anyhow::{anyhow, Context, Result};
use nix::{
    fcntl::{self, OFlag},
    sys::stat::Mode,
};
use tokio::{
    fs::OpenOptions,
    io::{AsyncRead, AsyncWrite},
    net::UnixStream as AsyncUnixStream,
};
use url::Url;

fn open_fifo(path: &str) -> Result<AsyncUnixStream> {
    let fd = fcntl::open(path, OFlag::O_RDWR, Mode::from_bits(0).unwrap())?;

    let std_stream = unsafe { StdUnixStream::from_raw_fd(fd) };
    std_stream
        .set_nonblocking(true)
        .context("set nonblocking")?;

    AsyncUnixStream::from_std(std_stream).map_err(|e| anyhow!(e))
}

pub struct ShimIo {
    pub stdin: Option<Box<dyn AsyncRead + Send + Unpin>>,
    pub stdout: Option<Box<dyn AsyncWrite + Send + Unpin>>,
    pub stderr: Option<Box<dyn AsyncWrite + Send + Unpin>>,
}

impl ShimIo {
    pub async fn new(
        stdin: &Option<String>,
        stdout: &Option<String>,
        stderr: &Option<String>,
    ) -> Result<Self> {
        info!(
            sl!(),
            "new shim io stdin {:?} stdout {:?} stderr {:?}", stdin, stdout, stderr
        );

        let stdin_fd: Option<Box<dyn AsyncRead + Send + Unpin>> = if let Some(stdin) = stdin {
            info!(sl!(), "open stdin {:?}", &stdin);
            match OpenOptions::new()
                .read(true)
                .write(false)
                .custom_flags(libc::O_NONBLOCK)
                .open(&stdin)
                .await
            {
                Ok(file) => Some(Box::new(file)),
                Err(err) => {
                    error!(sl!(), "failed to open {} error {:?}", &stdin, err);
                    None
                }
            }
        } else {
            None
        };

        let get_url = |url: &Option<String>| -> Option<Url> {
            info!(sl!(), "get url for {:?}", url);

            match url {
                None => None,
                Some(out) => match Url::parse(out.as_str()) {
                    Err(url::ParseError::RelativeUrlWithoutBase) => {
                        let out = "fifo://".to_owned() + out.as_str();
                        let u = Url::parse(out.as_str()).unwrap();
                        Some(u)
                    }
                    Err(err) => {
                        warn!(sl!(), "unable to parse stdout uri: {}", err);
                        None
                    }
                    Ok(u) => Some(u),
                },
            }
        };

        let stdout_url = get_url(stdout);
        let get_fd = |url: &Option<Url>| -> Option<Box<dyn AsyncWrite + Send + Unpin>> {
            info!(sl!(), "get fd for {:?}", &url);
            if let Some(url) = url {
                if url.scheme() == "fifo" {
                    let path = url.path();
                    match open_fifo(path) {
                        Ok(s) => {
                            return Some(Box::new(ShimIoWrite::Stream(s)));
                        }
                        Err(err) => {
                            error!(sl!(), "failed to open file {} error {:?}", url.path(), err);
                        }
                    }
                }
            }
            None
        };

        let stderr_url = get_url(stderr);
        Ok(Self {
            stdin: stdin_fd,
            stdout: get_fd(&stdout_url),
            stderr: get_fd(&stderr_url),
        })
    }
}

#[derive(Debug)]
enum ShimIoWrite {
    Stream(AsyncUnixStream),
    // TODO: support other type
}

impl AsyncWrite for ShimIoWrite {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match *self {
            ShimIoWrite::Stream(ref mut s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<io::Result<()>> {
        match *self {
            ShimIoWrite::Stream(ref mut s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<io::Result<()>> {
        match *self {
            ShimIoWrite::Stream(ref mut s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}
