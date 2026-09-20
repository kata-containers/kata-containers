// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::os::unix::{
    fs::{FileTypeExt, OpenOptionsExt},
    io::RawFd,
    prelude::AsRawFd,
};

use anyhow::{Context, Result};
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncWrite},
    net::unix::pipe::{Receiver, Sender},
};
use url::Url;

use super::binary_io::{self, BinaryLogger};

/// Clear O_NONBLOCK for an fd (turn it into blocking mode).
fn set_flag_with_blocking(fd: RawFd) {
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

async fn open_fifo_read(path: &str) -> Result<Box<dyn AsyncRead + Send + Unpin>> {
    let file = tokio::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .await
        .with_context(|| format!("open fifo for read: {path}"))?;
    if file.metadata().await?.file_type().is_fifo() {
        return Ok(Box::new(Receiver::from_owned_fd(
            file.into_std().await.into(),
        )?));
    }
    set_flag_with_blocking(file.as_raw_fd());
    Ok(Box::new(file))
}

fn open_fifo_write(path: &str) -> Result<Box<dyn AsyncWrite + Send + Unpin>> {
    let std_file = std::fs::OpenOptions::new()
        .write(true)
        // It's not for non-block openning FIFO but for non-block stream which
        // will be add into tokio runtime.
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .with_context(|| format!("open fifo for write: {path}"))?;

    if std_file.metadata()?.file_type().is_fifo() {
        return Ok(Box::new(Sender::from_owned_fd(std_file.into())?));
    }

    set_flag_with_blocking(std_file.as_raw_fd());

    Ok(Box::new(File::from_std(std_file)))
}

pub struct ShimIo {
    pub stdin: Option<Box<dyn AsyncRead + Send + Unpin>>,
    pub stdout: Option<Box<dyn AsyncWrite + Send + Unpin>>,
    pub stderr: Option<Box<dyn AsyncWrite + Send + Unpin>>,
    pub(crate) binary_logger: Option<BinaryLogger>,
}

impl ShimIo {
    pub async fn new(
        stdin: &Option<String>,
        stdout: &Option<String>,
        stderr: &Option<String>,
        container_id: &str,
        namespace: &str,
    ) -> Result<Self> {
        info!(
            sl!(),
            "new shim io stdin {:?} stdout {:?} stderr {:?}", stdin, stdout, stderr
        );

        let stdin_fd: Option<Box<dyn AsyncRead + Send + Unpin>> = if let Some(stdin) = stdin {
            info!(sl!(), "open stdin {:?}", &stdin);

            // Since we had opened the stdin as write mode in the Process::new function,
            // thus it wouldn't be blocked to open it as read mode.
            match open_fifo_read(stdin).await {
                Ok(file) => Some(file),
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
                        Url::parse(&format!("fifo://{}", out)).ok()
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
        if let Some(uri) = stdout_url.as_ref().filter(|uri| uri.scheme() == "binary") {
            let binary = binary_io::open(uri, container_id, namespace)
                .await
                .context("open binary logger")?;
            return Ok(Self {
                stdin: stdin_fd,
                stdout: Some(binary.stdout),
                stderr: Some(binary.stderr),
                binary_logger: Some(binary.logger),
            });
        }
        let get_fd = |url: &Option<Url>| -> Option<Box<dyn AsyncWrite + Send + Unpin>> {
            info!(sl!(), "get fd for {:?}", &url);
            if let Some(url) = url {
                if url.scheme() == "fifo" {
                    let path = url.path();
                    match open_fifo_write(path) {
                        Ok(f) => return Some(f),
                        Err(err) => error!(sl!(), "failed to open fifo {} error {:?}", path, err),
                    }
                } else {
                    warn!(sl!(), "unsupported io scheme {}", url.scheme());
                }
            }
            None
        };

        let stdout_url = get_url(stdout);
        let stderr_url = get_url(stderr);

        Ok(Self {
            stdin: stdin_fd,
            stdout: get_fd(&stdout_url),
            stderr: get_fd(&stderr_url),
            binary_logger: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn stdin_open_uses_blocking_pool() {
        let path = std::env::temp_dir().join(format!("kata-file-{}", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"stdin payload").unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        rt.block_on(async {
            let (entered_tx, entered_rx) = std::sync::mpsc::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            });
            entered_rx.recv().unwrap();
            let open = open_fifo_read(path.to_str().unwrap());
            tokio::pin!(open);
            let synchronous = tokio::select! {
                biased;
                _ = &mut open => true,
                _ = tokio::task::yield_now() => false,
            };
            // Release before asserting so even a regression cannot hang runtime drop.
            release_tx.send(()).unwrap();
            blocker.await.unwrap();
            assert!(!synchronous, "stdin open bypassed the blocking pool");
            let mut reader = open.await.unwrap();
            let mut data = Vec::new();
            reader.read_to_end(&mut data).await.unwrap();
            assert_eq!(data, b"stdin payload");
        });
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn regular_file_fallback_preserves_flags_and_flush() {
        let dir = std::env::temp_dir().join(format!("kata-file-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let input = dir.join("input");
        let output = dir.join("output");
        std::fs::write(&input, b"abcdef").unwrap();
        std::fs::write(&output, b"XXXXXXXX-tail").unwrap();
        let mut reader = open_fifo_read(input.to_str().unwrap()).await.unwrap();
        let mut writer = open_fifo_write(output.to_str().unwrap()).unwrap();
        assert_eq!(tokio::io::copy(&mut reader, &mut writer).await.unwrap(), 6);
        writer.flush().await.unwrap();
        drop(writer);
        assert_eq!(std::fs::read(&output).unwrap(), b"abcdefXX-tail");
        assert!(open_fifo_write(dir.join("missing").to_str().unwrap()).is_err());
        let mut null = open_fifo_write("/dev/null").unwrap();
        null.write_all(b"discard").await.unwrap();
        null.flush().await.unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
}
