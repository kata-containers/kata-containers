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
use tokio_util::sync::CancellationToken;

struct ContainerIoInfo {
    pub agent: Arc<dyn Agent>,
    pub process: ContainerProcess,
    vmm_exit_confirmed: CancellationToken,
}

pub struct ContainerIo {
    pub stdin: Box<dyn AsyncWrite + Send + Unpin>,
    pub stdout: Box<dyn AsyncRead + Send + Unpin>,
    pub stderr: Box<dyn AsyncRead + Send + Unpin>,
}

impl ContainerIo {
    pub fn new(
        agent: Arc<dyn Agent>,
        process: ContainerProcess,
        vmm_exit_confirmed: CancellationToken,
    ) -> Self {
        let info = Arc::new(ContainerIoInfo {
            agent,
            process,
            vmm_exit_confirmed,
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
                Err(err) => Poll::Ready(Err(std::io::Error::other(err))),
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
                Err(err) => Poll::Ready(Err(std::io::Error::other(err))),
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

type ReadFuture = Pin<Box<dyn Future<Output = Result<agent::ReadStreamResponse>> + Send>>;

struct ContainerIoRead {
    info: Arc<ContainerIoInfo>,
    is_stdout: bool,
    read_future: Option<ReadFuture>,
    buffered: io::Cursor<Vec<u8>>,
}

impl ContainerIoRead {
    fn new(info: Arc<ContainerIoInfo>, is_stdout: bool) -> Self {
        Self {
            info,
            is_stdout,
            read_future: None,
            buffered: Default::default(),
        }
    }
}

impl AsyncRead for ContainerIoRead {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        if self.buffered.position() == self.buffered.get_ref().len() as u64 {
            let Self {
                info,
                is_stdout,
                read_future,
                ..
            } = &mut *self;
            let future = read_future.get_or_insert_with(|| {
                Box::pin(read_output(info.clone(), *is_stdout, buf.remaining()))
            });
            let result = std::task::ready!(future.as_mut().poll(cx));
            self.read_future = None;
            self.buffered = io::Cursor::new(result.map_err(io::Error::other)?.data);
        }
        Pin::new(&mut self.buffered).poll_read(cx, buf)
    }
}

async fn read_output(
    info: Arc<ContainerIoInfo>,
    stdout: bool,
    len: usize,
) -> Result<agent::ReadStreamResponse> {
    let request = agent::ReadStreamRequest {
        process_id: info.process.clone().into(),
        len: len.min(u32::MAX as usize) as u32,
    };
    let read = crate::container_manager::retry_on_disconnect(|| {
        if stdout {
            info.agent.read_stdout(request.clone())
        } else {
            info.agent.read_stderr(request.clone())
        }
    });
    tokio::select! {
        biased;
        result = read => result,
        _ = info.vmm_exit_confirmed.cancelled() => Ok(Default::default()),
    }
}
