// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::r#async::utils;
use nix::unistd;
use std::collections::HashMap;
use std::os::unix::io::RawFd;
use std::result::Result as StdResult;
use std::sync::Arc;
use std::time::Duration;

use crate::asynchronous::stream::{receive, respond, respond_with_status};
use crate::asynchronous::unix_incoming::UnixIncoming;
use crate::common::{self, Domain, MESSAGE_TYPE_REQUEST};
use crate::context;
use crate::error::{get_status, Error, Result};
use crate::proto::{Code, Status};
use crate::r#async::{MethodHandler, TtrpcContext};
use crate::MessageHeader;
use futures::stream::Stream;
use futures::StreamExt as _;
use std::marker::Unpin;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::os::unix::net::UnixListener as SysUnixListener;
use tokio::{
    self,
    io::{split, AsyncRead, AsyncWrite, AsyncWriteExt},
    net::UnixListener,
    select, spawn,
    sync::mpsc::{channel, Receiver, Sender},
    sync::watch,
    time::timeout,
};

#[cfg(target_os = "linux")]
use tokio_vsock::VsockListener;

/// A ttrpc Server (async).
pub struct Server {
    listeners: Vec<RawFd>,
    methods: Arc<HashMap<String, Box<dyn MethodHandler + Send + Sync>>>,
    domain: Option<Domain>,
    disconnect_tx: Option<watch::Sender<i32>>,
    all_conn_done_rx: Option<Receiver<i32>>,
    stop_listen_tx: Option<Sender<Sender<RawFd>>>,
}

impl Default for Server {
    fn default() -> Self {
        Server {
            listeners: Vec::with_capacity(1),
            methods: Arc::new(HashMap::new()),
            domain: None,
            disconnect_tx: None,
            all_conn_done_rx: None,
            stop_listen_tx: None,
        }
    }
}

impl Server {
    pub fn new() -> Server {
        Server::default()
    }

    pub fn bind(mut self, sockaddr: &str) -> Result<Self> {
        if !self.listeners.is_empty() {
            return Err(Error::Others(
                "ttrpc-rust just support 1 sockaddr now".to_string(),
            ));
        }

        let (fd, domain) = common::do_bind(sockaddr)?;
        self.domain = Some(domain);

        common::do_listen(fd)?;
        self.listeners.push(fd);
        Ok(self)
    }

    pub fn set_domain_unix(mut self) -> Self {
        self.domain = Some(Domain::Unix);
        self
    }

    #[cfg(target_os = "linux")]
    pub fn set_domain_vsock(mut self) -> Self {
        self.domain = Some(Domain::Vsock);
        self
    }

    pub fn add_listener(mut self, fd: RawFd) -> Result<Server> {
        self.listeners.push(fd);

        Ok(self)
    }

    pub fn register_service(
        mut self,
        methods: HashMap<String, Box<dyn MethodHandler + Send + Sync>>,
    ) -> Server {
        let mut_methods = Arc::get_mut(&mut self.methods).unwrap();
        mut_methods.extend(methods);
        self
    }

    fn get_listenfd(&self) -> Result<RawFd> {
        if self.listeners.is_empty() {
            return Err(Error::Others("ttrpc-rust not bind".to_string()));
        }

        let listenfd = self.listeners[self.listeners.len() - 1];
        Ok(listenfd)
    }

    pub async fn start(&mut self) -> Result<()> {
        let listenfd = self.get_listenfd()?;

        match self.domain.as_ref() {
            Some(Domain::Unix) => {
                let sys_unix_listener;
                unsafe {
                    sys_unix_listener = SysUnixListener::from_raw_fd(listenfd);
                }
                sys_unix_listener
                    .set_nonblocking(true)
                    .map_err(err_to_others_err!(e, "set_nonblocking error "))?;
                let unix_listener = UnixListener::from_std(sys_unix_listener)
                    .map_err(err_to_others_err!(e, "from_std error "))?;

                let incoming = UnixIncoming::new(unix_listener);

                self.do_start(incoming).await
            }
            #[cfg(target_os = "linux")]
            Some(Domain::Vsock) => {
                let incoming = unsafe { VsockListener::from_raw_fd(listenfd).incoming() };
                self.do_start(incoming).await
            }
            _ => Err(Error::Others(
                "Domain is not set or not supported".to_string(),
            )),
        }
    }

    async fn do_start<I, S>(&mut self, mut incoming: I) -> Result<()>
    where
        I: Stream<Item = std::io::Result<S>> + Unpin + Send + 'static + AsRawFd,
        S: AsyncRead + AsyncWrite + AsRawFd + Send + 'static,
    {
        let methods = self.methods.clone();

        let (disconnect_tx, close_conn_rx) = watch::channel(0);
        self.disconnect_tx = Some(disconnect_tx);

        let (conn_done_tx, all_conn_done_rx) = channel::<i32>(1);
        self.all_conn_done_rx = Some(all_conn_done_rx);

        let (stop_listen_tx, mut stop_listen_rx) = channel(1);
        self.stop_listen_tx = Some(stop_listen_tx);

        spawn(async move {
            loop {
                select! {
                    conn = incoming.next() => {
                        if let Some(conn) = conn {
                            // Accept a new connection
                            match conn {
                                Ok(stream) => {
                                    let fd = stream.as_raw_fd();
                                    // spawn a connection handler, would not block
                                    spawn_connection_handler(
                                        fd,
                                        stream,
                                        methods.clone(),
                                        close_conn_rx.clone(),
                                        conn_done_tx.clone()
                                    ).await;
                                }
                                Err(e) => {
                                    error!("{:?}", e)
                                }
                            }

                        } else {
                            break;
                        }
                    }
                    fd_tx = stop_listen_rx.recv() => {
                        if let Some(fd_tx) = fd_tx {
                            // dup fd to keep the listener open
                            // or the listener will be closed when the incoming was dropped.
                            let dup_fd = unistd::dup(incoming.as_raw_fd()).unwrap();
                            common::set_fd_close_exec(dup_fd).unwrap();
                            drop(incoming);

                            fd_tx.send(dup_fd).await.unwrap();
                            break;
                        }
                    }
                }
            }
            drop(conn_done_tx);
        });
        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        self.stop_listen().await;
        self.disconnect().await;

        Ok(())
    }

    pub async fn disconnect(&mut self) {
        if let Some(tx) = self.disconnect_tx.take() {
            tx.send(1).ok();
        }

        if let Some(mut rx) = self.all_conn_done_rx.take() {
            rx.recv().await;
        }
    }

    pub async fn stop_listen(&mut self) {
        if let Some(tx) = self.stop_listen_tx.take() {
            let (fd_tx, mut fd_rx) = channel(1);
            tx.send(fd_tx).await.unwrap();

            let fd = fd_rx.recv().await.unwrap();
            self.listeners.clear();
            self.listeners.push(fd);
        }
    }
}

async fn spawn_connection_handler<S>(
    fd: RawFd,
    stream: S,
    methods: Arc<HashMap<String, Box<dyn MethodHandler + Send + Sync>>>,
    mut close_conn_rx: watch::Receiver<i32>,
    conn_done_tx: Sender<i32>,
) where
    S: AsyncRead + AsyncWrite + AsRawFd + Send + 'static,
{
    let (req_done_tx, mut all_req_done_rx) = channel::<i32>(1);

    spawn(async move {
        let (mut reader, mut writer) = split(stream);
        let (tx, mut rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = channel(100);
        let (client_disconnected_tx, client_disconnected_rx) = watch::channel(false);

        spawn(async move {
            while let Some(buf) = rx.recv().await {
                if let Err(e) = writer.write_all(&buf).await {
                    error!("write_message got error: {:?}", e);
                }
            }
        });

        loop {
            let tx = tx.clone();
            let methods = methods.clone();
            let req_done_tx2 = req_done_tx.clone();
            let mut client_disconnected_rx2 = client_disconnected_rx.clone();

            select! {
                resp = receive(&mut reader) => {
                    match resp {
                        Ok(message) => {
                            spawn(async move {
                                select! {
                                    _ = handle_request(tx, fd, methods, message) => {}
                                    _ = client_disconnected_rx2.changed() => {}
                                }

                                drop(req_done_tx2);
                            });
                        }
                        Err(e) => {
                            let _ = client_disconnected_tx.send(true);
                            trace!("error {:?}", e);
                            break;
                        }
                    }
                }
                v = close_conn_rx.changed() => {
                    // 0 is the init value of this watch, not a valid signal
                    // is_err means the tx was dropped.
                    if v.is_err() || *close_conn_rx.borrow() != 0 {
                        info!("Stop accepting new connections.");
                        break;
                    }
                }
            }
        }

        drop(req_done_tx);
        all_req_done_rx.recv().await;
        drop(conn_done_tx);
    });
}

async fn do_handle_request(
    fd: RawFd,
    methods: Arc<HashMap<String, Box<dyn MethodHandler + Send + Sync>>>,
    header: MessageHeader,
    body: &[u8],
) -> StdResult<(u32, Vec<u8>), Status> {
    let req = utils::body_to_request(body)?;
    let path = utils::get_path(&req.service, &req.method);
    let method = methods
        .get(&path)
        .ok_or_else(|| get_status(Code::INVALID_ARGUMENT, format!("{} does not exist", &path)))?;

    let ctx = TtrpcContext {
        fd,
        mh: header,
        metadata: context::from_pb(&req.metadata),
        timeout_nano: req.timeout_nano,
    };

    let get_unknown_status_and_log_err = |e| {
        error!("method handle {} got error {:?}", path, &e);
        get_status(Code::UNKNOWN, e)
    };

    if req.timeout_nano == 0 {
        method
            .handler(ctx, req)
            .await
            .map_err(get_unknown_status_and_log_err)
    } else {
        timeout(
            Duration::from_nanos(req.timeout_nano as u64),
            method.handler(ctx, req),
        )
        .await
        .map_err(|_| {
            // Timed out
            error!("method handle {} got error timed out", path);
            get_status(Code::DEADLINE_EXCEEDED, "timeout")
        })
        .and_then(|r| {
            // Handler finished
            r.map_err(get_unknown_status_and_log_err)
        })
    }
}

async fn handle_request(
    tx: Sender<Vec<u8>>,
    fd: RawFd,
    methods: Arc<HashMap<String, Box<dyn MethodHandler + Send + Sync>>>,
    message: (MessageHeader, Vec<u8>),
) {
    let (header, body) = message;
    let stream_id = header.stream_id;

    if header.type_ != MESSAGE_TYPE_REQUEST {
        return;
    }

    match do_handle_request(fd, methods, header, &body).await {
        Ok((stream_id, resp_body)) => {
            if let Err(x) = respond(tx.clone(), stream_id, resp_body).await {
                error!("respond got error {:?}", x);
            }
        }
        Err(status) => {
            if let Err(x) = respond_with_status(tx.clone(), stream_id, status).await {
                error!("respond got error {:?}", x);
            }
        }
    }
}

impl FromRawFd for Server {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self::default().add_listener(fd).unwrap()
    }
}

impl AsRawFd for Server {
    fn as_raw_fd(&self) -> RawFd {
        self.listeners[0]
    }
}
