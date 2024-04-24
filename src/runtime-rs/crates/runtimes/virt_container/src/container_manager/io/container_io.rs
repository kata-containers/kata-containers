// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    future::Future,
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use agent::Agent;
use anyhow::Result;
use common::types::ContainerProcess;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

struct ContainerIoInfo {
    pub agent: Arc<dyn Agent>,
    pub process: ContainerProcess,
}

pub struct ContainerIo {
    pub stdin: Box<dyn AsyncWrite + Send + Unpin>,
    pub stdout: Box<dyn AsyncRead + Send + Unpin>,
    pub stderr: Box<dyn AsyncRead + Send + Unpin>,
}

impl ContainerIo {
    pub fn new(agent: Arc<dyn Agent>, process: ContainerProcess) -> Self {
        let info = Arc::new(ContainerIoInfo { agent, process });

        Self {
            stdin: Box::new(ContainerIoWrite::new(info.clone())),
            stdout: Box::new(ContainerIoRead::new(info.clone(), true)),
            stderr: Box::new(ContainerIoRead::new(info, false)),
        }
    }
}

struct ContainerIoWrite<'inner> {
    pub info: Arc<ContainerIoInfo>,
    write_future:
        Option<Pin<Box<dyn Future<Output = Result<agent::WriteStreamResponse>> + Send + 'inner>>>,
    shutdown_future:
        Option<Pin<Box<dyn Future<Output = Result<agent::WriteStreamResponse>> + Send + 'inner>>>,
}

impl<'inner> ContainerIoWrite<'inner> {
    pub fn new(info: Arc<ContainerIoInfo>) -> Self {
        Self {
            info,
            write_future: Default::default(),
            shutdown_future: Default::default(),
        }
    }

    fn poll_write_inner(
        &'inner mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut write_future = self.write_future.take();
        if write_future.is_none() {
            let req = agent::WriteStreamRequest {
                process_id: self.info.process.clone().into(),
                data: buf.to_vec(),
            };
            write_future = Some(Box::pin(self.info.agent.write_stdin(req)));
        }

        let mut write_future = write_future.unwrap();
        match write_future.as_mut().poll(cx) {
            Poll::Ready(v) => match v {
                Ok(resp) => Poll::Ready(Ok(resp.length as usize)),
                Err(err) => Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, err))),
            },
            Poll::Pending => {
                self.write_future = Some(write_future);
                Poll::Pending
            }
        }
    }

    // Call rpc agent.write_stdin() with empty data to tell agent to close stdin of the process
    fn poll_shutdown_inner(&'inner mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut shutdown_future = self.shutdown_future.take();
        if shutdown_future.is_none() {
            let req = agent::WriteStreamRequest {
                process_id: self.info.process.clone().into(),
                data: Vec::with_capacity(0),
            };
            shutdown_future = Some(Box::pin(self.info.agent.write_stdin(req)));
        }

        let mut shutdown_future = shutdown_future.unwrap();
        match shutdown_future.as_mut().poll(cx) {
            Poll::Ready(v) => match v {
                Ok(_) => Poll::Ready(Ok(())),
                Err(err) => Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, err))),
            },
            Poll::Pending => {
                self.shutdown_future = Some(shutdown_future);
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

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let me = unsafe {
            std::mem::transmute::<&mut ContainerIoWrite<'_>, &mut ContainerIoWrite<'inner>>(
                &mut *self,
            )
        };
        me.poll_shutdown_inner(cx)
    }
}

type ResultBuffer = Result<agent::ReadStreamResponse>;
struct ContainerIoRead<'inner> {
    pub info: Arc<ContainerIoInfo>,
    is_stdout: bool,
    read_future: Option<Pin<Box<dyn Future<Output = ResultBuffer> + Send + 'inner>>>,
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
            let req = agent::ReadStreamRequest {
                process_id: self.info.process.clone().into(),
                len: buf.remaining() as u32,
            };
            read_future = if self.is_stdout {
                Some(Box::pin(self.info.agent.read_stdout(req)))
            } else {
                Some(Box::pin(self.info.agent.read_stderr(req)))
            };
        }

        let mut read_future = read_future.unwrap();
        match read_future.as_mut().poll(cx) {
            Poll::Ready(v) => match v {
                Ok(resp) => {
                    buf.put_slice(&resp.data);
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
