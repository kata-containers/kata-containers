// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    io,
    os::unix::{
        fs::{FileTypeExt, OpenOptionsExt},
        io::{AsRawFd, RawFd},
    },
    pin::Pin,
    task::{Context as TaskContext, Poll},
};

use anyhow::{Context, Result};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncRead, AsyncWrite},
};
use url::Url;

/// Clear O_NONBLOCK for an fd (turn it into blocking mode).
fn set_flag_with_blocking(fd: RawFd) {
    info!(sl!(), "Clear O_NONBLOCK for an fd (turn it into blocking mode)");

    let flag = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flag < 0 {
        error!(sl!(), "failed to fcntl(F_GETFL) fd {} ret {}", fd, flag);
        return;
    }

    let ret = unsafe { libc::fcntl(fd, libc::F_SETFL, flag & !libc::O_NONBLOCK) };
    if ret < 0 {
        error!(sl!(), "failed to fcntl(F_SETFL) fd {} ret {}", fd, ret);
    }
}

/// Open FIFO for write.
///
/// Strategy:
/// - open with O_NONBLOCK to avoid open() blocking when no reader is present.
/// - then clear O_NONBLOCK to avoid EAGAIN busy-loop during writes.
fn open_fifo_write(path: &str) -> Result<File> {
    let std_file = std::fs::OpenOptions::new()
        .write(true)
        // It's not for non-block openning FIFO but for non-block stream which
        // will be add into tokio runtime.
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .with_context(|| format!("open fifo for write: {path}"))?;

    // Debug
    let meta = std_file.metadata()?;
    if !meta.file_type().is_fifo() {
        warn!(sl!(), "[DEBUG]{} is not a fifo (type mismatch)", path);
    }

    set_flag_with_blocking(std_file.as_raw_fd());

    Ok(File::from_std(std_file))
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

        // stdin: open FIFO read end (tokio File) and set to blocking to avoid EAGAIN spin.
        let stdin_fd: Option<Box<dyn AsyncRead + Send + Unpin>> = if let Some(stdin) = stdin {
            info!(sl!(), "open FIFO stdin {:?}", &stdin);

            match OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(&stdin)
                .await
            {
                Ok(file) => {
                    set_flag_with_blocking(file.as_raw_fd());
                    Some(Box::new(file))
                }
                Err(err) => {
                    error!(sl!(), "failed to open {} error {:?}", &stdin, err);
                    None
                }
            }
        } else {
            None
        };

        // Parse "fifo://..." and also accept plain path by prepending fifo://
        let get_url = |s: &Option<String>| -> Option<Url> {
            info!(sl!(), "get url for {:?}", s);
            match s {
                None => None,
                Some(v) => match Url::parse(v.as_str()) {
                    Err(url::ParseError::RelativeUrlWithoutBase) => {
                        Url::parse(&format!("fifo://{}", v)).ok()
                    }
                    Ok(u) => Some(u),
                    Err(err) => {
                        warn!(sl!(), "unable to parse uri: {}", err);
                        None
                    }
                },
            }
        };

        fn get_fifo_writer(url: &Option<Url>) -> Option<Box<dyn AsyncWrite + Send + Unpin>> {
            info!(sl!(), "get fd for {:?}", url);
            if let Some(url) = url {
                if url.scheme() == "fifo" {
                    let path = url.path();
                    match open_fifo_write(path) {
                        Ok(f) => return Some(Box::new(ShimIoWrite::File(f))),
                        Err(err) => error!(sl!(), "failed to open fifo {} error {:?}", path, err),
                    }
                } else {
                    warn!(sl!(), "unsupported io scheme {}", url.scheme());
                }
            }
            None
        }

        let stdout_url = get_url(stdout);
        let stderr_url = get_url(stderr);

        Ok(Self {
            stdin: stdin_fd,
            stdout: get_fifo_writer(&stdout_url),
            stderr: get_fifo_writer(&stderr_url),
        })
    }
}

#[derive(Debug)]
enum ShimIoWrite {
    File(File),
    // TODO: support other types (e.g. real unix socket for non-fifo schemes)
}

impl AsyncWrite for ShimIoWrite {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            ShimIoWrite::File(f) => Pin::new(f).poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            ShimIoWrite::File(f) => Pin::new(f).poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            // For File/FIFO, shutdown just closes/drops; tokio's File implements it as Ok.
            ShimIoWrite::File(f) => Pin::new(f).poll_shutdown(cx),
        }
    }
}
