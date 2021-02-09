// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use nix::unistd;
use protobuf::{CodedInputStream, Message};
use std::collections::HashMap;
use std::os::unix::io::RawFd;
use std::sync::Arc;

use crate::asynchronous::stream::{receive, respond, respond_with_status};
use crate::common::{self, Domain, MESSAGE_TYPE_REQUEST};
use crate::error::{get_status, Error, Result};
use crate::r#async::{MethodHandler, TtrpcContext};
use crate::ttrpc::{Code, Request};
use crate::MessageHeader;
use futures::StreamExt as _;
use std::marker::Unpin;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::os::unix::net::UnixListener as SysUnixListener;
use tokio::{
    self,
    io::split,
    net::UnixListener,
    prelude::*,
    stream::Stream,
    sync::mpsc::{channel, Receiver, Sender},
    sync::watch,
};
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

    pub fn bind(mut self, host: &str) -> Result<Self> {
        if !self.listeners.is_empty() {
            return Err(Error::Others(
                "ttrpc-rust just support 1 host now".to_string(),
            ));
        }

        let (fd, domain) = common::do_bind(host)?;
        self.domain = Some(domain);

        common::do_listen(fd)?;
        self.listeners.push(fd);
        Ok(self)
    }

    pub fn set_domain_unix(mut self) -> Self {
        self.domain = Some(Domain::Unix);
        self
    }

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
                let unix_listener = UnixListener::from_std(sys_unix_listener).unwrap();

                self.do_start(listenfd, unix_listener).await
            }
            Some(Domain::Vsock) => {
                let incoming;
                unsafe {
                    incoming = VsockListener::from_raw_fd(listenfd).incoming();
                }

                self.do_start(listenfd, incoming).await
            }
            _ => Err(Error::Others("Domain is not set".to_string())),
        }
    }

    pub async fn do_start<I, S>(&mut self, listenfd: RawFd, mut incoming: I) -> Result<()>
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

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    conn = incoming.next() => {
                        if let Some(conn) = conn {
                            // Accept a new connection
                            let methods = methods.clone();
                            match conn {
                                Ok(stream) => {
                                    let fd = stream.as_raw_fd();
                                    if let Err(e) = common::set_fd_close_exec(fd) {
                                        error!("{:?}", e);
                                        continue;
                                    }

                                    let mut close_conn_rx = close_conn_rx.clone();

                                    let (req_done_tx, mut all_req_done_rx) = channel::<i32>(1);
                                    let conn_done_tx2 = conn_done_tx.clone();

                                    // The connection handler
                                    tokio::spawn(async move {
                                        let (mut reader, mut writer) = split(stream);
                                        let (tx, mut rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = channel(100);

                                        tokio::spawn(async move {
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

                                            tokio::select! {
                                                resp = receive(&mut reader) => {
                                                    match resp {
                                                        Ok(message) => {
                                                            tokio::spawn(async move {
                                                                handle_request(tx, listenfd, methods, message).await;
                                                                drop(req_done_tx2);
                                                            });
                                                        }
                                                        Err(e) => {
                                                            trace!("error {:?}", e);
                                                            break;
                                                        }
                                                    }
                                                }
                                                v = close_conn_rx.recv() => {
                                                    // 0 is the init value of this watch, not a valid signal
                                                    // is_none means the tx was dropped.
                                                    if v.is_none() || v.unwrap() != 0 {
                                                        info!("Stop accepting new connections.");
                                                        break;
                                                    }
                                                }
                                            }
                                        }

                                        drop(req_done_tx);
                                        all_req_done_rx.recv().await;
                                        drop(conn_done_tx2);
                                    });
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
                        if let Some(mut fd_tx) = fd_tx {
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
            tx.broadcast(1).ok();
        }

        if let Some(mut rx) = self.all_conn_done_rx.take() {
            rx.recv().await;
        }
    }

    pub async fn stop_listen(&mut self) {
        if let Some(mut tx) = self.stop_listen_tx.take() {
            let (fd_tx, mut fd_rx) = channel(1);
            tx.send(fd_tx).await.unwrap();

            let fd = fd_rx.recv().await.unwrap();
            self.listeners.clear();
            self.listeners.push(fd);
        }
    }
}

async fn handle_request(
    tx: Sender<Vec<u8>>,
    fd: RawFd,
    methods: Arc<HashMap<String, Box<dyn MethodHandler + Send + Sync>>>,
    message: (MessageHeader, Vec<u8>),
) {
    let (header, body) = message;
    if header.type_ != MESSAGE_TYPE_REQUEST {
        return;
    }

    let mut req = Request::new();
    let merge_result;
    {
        let mut s = CodedInputStream::from_bytes(&body);
        merge_result = req.merge_from(&mut s);
    }

    if merge_result.is_err() {
        let status = get_status(Code::INVALID_ARGUMENT, "".to_string());

        if let Err(x) = respond_with_status(tx.clone(), header.stream_id, status).await {
            error!("respond get error {:?}", x);
        }
    }
    trace!("Got Message request {:?}", req);

    let path = format!("/{}/{}", req.service, req.method);
    if let Some(x) = methods.get(&path) {
        let method = x;
        let ctx = TtrpcContext { fd, mh: header };

        match method.handler(ctx, req).await {
            Ok((stream_id, body)) => {
                if let Err(x) = respond(tx.clone(), stream_id, body).await {
                    error!("respond get error {:?}", x);
                }
            }
            Err(e) => {
                error!("method handle {} get error {:?}", path, e);
            }
        }
    } else {
        let status = get_status(Code::INVALID_ARGUMENT, format!("{} does not exist", path));
        if let Err(e) = respond_with_status(tx, header.stream_id, status).await {
            error!("respond get error {:?}", e);
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
