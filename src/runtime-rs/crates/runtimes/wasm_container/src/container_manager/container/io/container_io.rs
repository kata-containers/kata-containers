// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::container_manager::container::container_inner::ContainerInner;
use rustjail::pipestream as InnerPipestream;
use rustjail::process as InnerProcess;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use std::{
    future::Future,
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use anyhow::{anyhow, Result};
use common::types::ContainerProcess;
use tokio::io::ReadHalf;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::Mutex;
use tokio::sync::RwLock;

struct ContainerIoInfo {
    pub container_inner: Arc<RwLock<ContainerInner>>,
    pub process: ContainerProcess,
}

async fn read_stream(
    reader: Arc<Mutex<ReadHalf<InnerPipestream::PipeStream>>>,
    l: usize,
) -> Result<Vec<u8>> {
    let mut content = vec![0u8; l];

    let mut reader = reader.lock().await;
    let len = reader.read(&mut content).await?;
    content.resize(len, 0);

    if len == 0 {
        return Err(anyhow!("read meet eof"));
    }

    Ok(content)
}

impl ContainerIoInfo {
    async fn write_input(&self, data: Vec<u8>) -> Result<usize> {
        let writer = {
            let mut inner = self.container_inner.write().await;

            let p = inner.get_inner_process(&self.process)?;

            // use ptmx io
            if p.term_master.is_some() {
                p.get_writer(InnerProcess::StreamType::TermMaster)
            } else {
                // use piped io
                p.get_writer(InnerProcess::StreamType::ParentStdin)
            }
        };

        let writer = writer.ok_or_else(|| anyhow!("Cannot get writer"))?;
        writer.lock().await.write_all(data.as_slice()).await?;

        Ok(data.len() as usize)
    }

    async fn read_output(&self, len: usize, stdout: bool) -> Result<Vec<u8>> {
        let mut term_exit_notifier = Arc::new(tokio::sync::Notify::new());
        let reader = {
            let mut inner = self.container_inner.write().await;

            let p = inner.get_inner_process(&self.process)?;

            if p.term_master.is_some() {
                term_exit_notifier = p.term_exit_notifier.clone();
                p.get_reader(InnerProcess::StreamType::TermMaster)
            } else if stdout {
                if p.parent_stdout.is_some() {
                    p.get_reader(InnerProcess::StreamType::ParentStdout)
                } else {
                    None
                }
            } else {
                p.get_reader(InnerProcess::StreamType::ParentStderr)
            }
        };

        if reader.is_none() {
            return Err(anyhow!("Unable to determine stream reader, is None"));
        }

        let reader = reader.ok_or_else(|| anyhow!("cannot get stream reader"))?;

        tokio::select! {
            _ = term_exit_notifier.notified() => {
                Err(anyhow!("eof"))
            }
            v = read_stream(reader, len)  => {
                let vector = v?;
                Ok(vector)
            }
        }
    }
}

pub struct ContainerIo {
    pub stdin: Box<dyn AsyncWrite + Send + Unpin>,
    pub stdout: Box<dyn AsyncRead + Send + Unpin>,
    pub stderr: Box<dyn AsyncRead + Send + Unpin>,
}

impl ContainerIo {
    pub fn new(container_inner: Arc<RwLock<ContainerInner>>, process: ContainerProcess) -> Self {
        let info = Arc::new(ContainerIoInfo {
            container_inner,
            process,
        });

        Self {
            stdin: Box::new(ContainerIoWrite::new(info.clone())),
            stdout: Box::new(ContainerIoRead::new(info.clone(), true)),
            stderr: Box::new(ContainerIoRead::new(info, false)),
        }
    }
}

struct ContainerIoWrite<'inner> {
    pub info: Arc<ContainerIoInfo>,
    write_future: Option<Pin<Box<dyn Future<Output = Result<usize>> + Send + 'inner>>>,
}

impl<'inner> ContainerIoWrite<'inner> {
    pub fn new(info: Arc<ContainerIoInfo>) -> Self {
        Self {
            info,
            write_future: Default::default(),
        }
    }

    fn poll_write_inner(
        &'inner mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut write_future = self.write_future.take();
        if write_future.is_none() {
            write_future = Some(Box::pin(self.info.write_input(buf.to_vec())));
        }

        let mut write_future = write_future.unwrap();
        match write_future.as_mut().poll(cx) {
            Poll::Ready(v) => match v {
                Ok(len) => Poll::Ready(Ok(len)),
                Err(err) => Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, err))),
            },
            Poll::Pending => {
                self.write_future = Some(write_future);
                Poll::Pending
            }
        }
    }
}

impl<'inner> AsyncWrite for ContainerIoWrite<'inner> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let me = unsafe {
            std::mem::transmute::<&mut ContainerIoWrite<'_>, &mut ContainerIoWrite<'inner>>(
                &mut *self,
            )
        };
        me.poll_write_inner(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

struct ContainerIoRead<'inner> {
    pub info: Arc<ContainerIoInfo>,
    is_stdout: bool,
    read_future: Option<Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'inner>>>,
}

impl<'inner> ContainerIoRead<'inner> {
    pub fn new(info: Arc<ContainerIoInfo>, is_stdout: bool) -> Self {
        Self {
            info,
            is_stdout,
            read_future: Default::default(),
        }
    }
    fn poll_read_inner(
        &'inner mut self,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let mut read_future = self.read_future.take();
        if read_future.is_none() {
            let len = buf.remaining();
            read_future = if self.is_stdout {
                Some(Box::pin(self.info.read_output(len, true)))
            } else {
                Some(Box::pin(self.info.read_output(len, false)))
            };
        }

        let mut read_future = read_future.unwrap();
        match read_future.as_mut().poll(cx) {
            Poll::Ready(v) => match v {
                Ok(data) => {
                    buf.put_slice(&data);
                    Poll::Ready(Ok(()))
                }
                Err(err) => Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, err))),
            },
            Poll::Pending => {
                self.read_future = Some(read_future);
                Poll::Pending
            }
        }
    }
}

impl<'inner> AsyncRead for ContainerIoRead<'inner> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let me = unsafe {
            std::mem::transmute::<&mut ContainerIoRead<'_>, &mut ContainerIoRead<'inner>>(
                &mut *self,
            )
        };
        me.poll_read_inner(cx, buf)
    }
}
