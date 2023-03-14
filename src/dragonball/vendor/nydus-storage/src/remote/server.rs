// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::io::Result;
use std::mem;
use std::net::Shutdown;
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use vm_memory::ByteValued;

use crate::remote::client::RemoteBlobMgr;
use crate::remote::connection::{Endpoint, Listener};
use crate::remote::message::{
    FetchRangeReply, FetchRangeRequest, GetBlobReply, GetBlobRequest, MsgHeader, MsgValidator,
    RequestCode,
};

/// Remote blob manager client connection and state.
pub struct ClientConnection {
    conn: Mutex<Endpoint>,
    exiting: AtomicBool,
    id: u64,
    state: ServerState,
    token: AtomicU32,
    uds: UnixStream,
}

impl ClientConnection {
    fn new(server: ServerState, id: u64, sock: UnixStream) -> Result<Self> {
        let uds = sock.try_clone()?;

        if id > u32::MAX as u64 {
            return Err(einval!("ran out of connection id"));
        }

        Ok(Self {
            conn: Mutex::new(Endpoint::from_stream(sock)),
            exiting: AtomicBool::new(false),
            id,
            state: server,
            token: AtomicU32::new(1),
            uds,
        })
    }

    fn shutdown(&self) {
        if !self.exiting.swap(true, Ordering::AcqRel) {
            let _ = self.uds.shutdown(Shutdown::Both);
        }
    }

    /// Close the connection.
    pub fn close(&self) {
        let id = self.id;
        let entry = self.state.lock_clients().remove(&id);

        if let Some(conn) = entry {
            conn.shutdown();
        }
    }

    /// Get a unique identifier for the client connection.
    pub fn id(&self) -> u32 {
        self.id as u32
    }

    fn handle_message(&self) -> Result<bool> {
        if self.exiting.load(Ordering::Acquire) {
            return Ok(false);
        }

        let mut guard = self.lock_conn();
        let (mut hdr, _files) = guard.recv_header().map_err(|e| eio!(format!("{}", e)))?;
        match hdr.get_code() {
            RequestCode::Noop => self.handle_noop(&mut hdr, guard)?,
            RequestCode::GetBlob => self.handle_get_blob(&mut hdr, guard)?,
            RequestCode::FetchRange => self.handle_fetch_range(&mut hdr, guard)?,
            cmd => {
                let msg = format!("unknown request command {}", u32::from(cmd));
                return Err(einval!(msg));
            }
        }

        Ok(true)
    }

    fn handle_noop(&self, hdr: &mut MsgHeader, mut guard: MutexGuard<Endpoint>) -> Result<()> {
        let size = hdr.get_size() as usize;
        if !hdr.is_valid() || size != 0 {
            return Err(eio!("invalid noop request message"));
        }

        hdr.set_reply(true);
        guard.send_header(hdr, None).map_err(|_e| eio!())
    }

    fn handle_get_blob(&self, hdr: &mut MsgHeader, mut guard: MutexGuard<Endpoint>) -> Result<()> {
        let size = hdr.get_size() as usize;
        if !hdr.is_valid() || size != mem::size_of::<GetBlobRequest>() {
            return Err(eio!("invalid get blob request message"));
        }

        let (sz, data) = guard.recv_data(size).map_err(|e| eio!(format!("{}", e)))?;
        if sz != size || data.len() != size {
            return Err(einval!("invalid get blob request message"));
        }
        drop(guard);

        let mut msg = GetBlobRequest::default();
        msg.as_mut_slice().copy_from_slice(&data);

        // TODO
        let token = self.token.fetch_add(1, Ordering::AcqRel) as u64;
        let gen = (msg.generation as u64) << 32;
        let reply = GetBlobReply::new(gen | token, 0, libc::ENOSYS as u32);

        let mut guard = self.lock_conn();
        hdr.set_reply(true);
        guard.send_message(hdr, &reply, None).map_err(|_e| eio!())
    }

    fn handle_fetch_range(
        &self,
        hdr: &mut MsgHeader,
        mut guard: MutexGuard<Endpoint>,
    ) -> Result<()> {
        let size = hdr.get_size() as usize;
        if !hdr.is_valid() || size != mem::size_of::<FetchRangeRequest>() {
            return Err(eio!("invalid fetch range request message"));
        }

        let (sz, data) = guard.recv_data(size).map_err(|e| eio!(format!("{}", e)))?;
        if sz != size || data.len() != size {
            return Err(einval!("invalid fetch range request message"));
        }
        drop(guard);

        // TODO
        let mut msg = FetchRangeRequest::default();
        msg.as_mut_slice().copy_from_slice(&data);

        let reply = FetchRangeReply::new(0, msg.count, 0);

        let mut guard = self.lock_conn();
        hdr.set_reply(true);
        guard.send_message(hdr, &reply, None).map_err(|_e| eio!())
    }

    fn lock_conn(&self) -> MutexGuard<Endpoint> {
        // Do not expect poisoned lock.
        self.conn.lock().unwrap()
    }
}

impl AsRawFd for ClientConnection {
    fn as_raw_fd(&self) -> RawFd {
        let guard = self.lock_conn();

        guard.as_raw_fd()
    }
}

#[derive(Clone)]
struct ServerState {
    active_workers: Arc<AtomicU64>,
    clients: Arc<Mutex<HashMap<u64, Arc<ClientConnection>>>>,
}

impl ServerState {
    fn new() -> Self {
        Self {
            active_workers: Arc::new(AtomicU64::new(0)),
            clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn add(&self, id: u64, client: Arc<ClientConnection>) {
        self.lock_clients().insert(id, client);
    }

    fn remove(&self, id: u64) {
        self.lock_clients().remove(&id);
    }

    fn lock_clients(&self) -> MutexGuard<HashMap<u64, Arc<ClientConnection>>> {
        // Do not expect poisoned lock here.
        self.clients.lock().unwrap()
    }
}

/// Blob server to accept connections from clients.
pub struct Server {
    sock: String,
    next_id: AtomicU64,
    exiting: AtomicBool,
    listener: Listener,
    state: ServerState,
}

impl Server {
    /// Create a new instance of `Server` to accept connections from clients.
    pub fn new(sock: &str) -> Result<Self> {
        let listener = Listener::new(sock, true).map_err(|_e| eio!())?;

        Ok(Server {
            sock: sock.to_owned(),
            next_id: AtomicU64::new(1024),
            exiting: AtomicBool::new(false),
            listener,
            state: ServerState::new(),
        })
    }

    /// Start a worker thread to handle incoming connections from clients.
    pub fn start(server: Arc<Server>) -> Result<()> {
        server
            .listener
            .set_nonblocking(false)
            .map_err(|_e| eio!())?;

        std::thread::spawn(move || {
            server.state.active_workers.fetch_add(1, Ordering::Acquire);

            'listen: loop {
                if server.exiting.load(Ordering::Acquire) {
                    break 'listen;
                }

                match server.listener.accept() {
                    Ok(Some(sock)) => {
                        let id = server.next_id.fetch_add(1, Ordering::AcqRel);
                        let client = match ClientConnection::new(server.state.clone(), id, sock) {
                            Ok(v) => v,
                            Err(e) => {
                                warn!("failed to duplicate unix domain socket, {}", e);
                                break 'listen;
                            }
                        };
                        let client = Arc::new(client);

                        client.state.add(id, client.clone());
                        std::thread::spawn(move || {
                            client.state.active_workers.fetch_add(1, Ordering::AcqRel);
                            loop {
                                if let Err(e) = client.handle_message() {
                                    warn!("failed to handle request, {}", e);
                                    break;
                                }
                            }
                            client.state.active_workers.fetch_sub(1, Ordering::AcqRel);
                            client.state.remove(client.id);
                            client.shutdown();
                        });
                    }
                    Ok(None) => {}
                    Err(e) => {
                        error!("failed to accept connection, {}", e);
                        break 'listen;
                    }
                }
            }

            server.state.active_workers.fetch_sub(1, Ordering::AcqRel);
        });

        Ok(())
    }

    /// Shutdown the listener and all active client connections.
    pub fn stop(&self) {
        if !self.exiting.swap(true, Ordering::AcqRel) {
            if self.state.active_workers.load(Ordering::Acquire) > 0 {
                // Hacky way to wake up the listener threads from accept().
                let client = RemoteBlobMgr::new("".to_owned(), &self.sock).unwrap();
                let _ = client.connect();
            }

            let mut guard = self.state.lock_clients();
            for (_token, client) in guard.iter() {
                client.shutdown();
            }
            guard.clear();
        }
    }

    /// Close the client connection with `id`.
    pub fn close_connection(&self, id: u32) {
        let id = id as u64;
        let entry = self.state.lock_clients().remove(&id);

        if let Some(conn) = entry {
            conn.shutdown();
        }
    }

    pub fn handle_event(&self, id: u32) -> Result<()> {
        let id64 = id as u64;
        let conn = self.state.lock_clients().get(&id64).cloned();

        if let Some(c) = conn {
            match c.handle_message() {
                Ok(true) => Ok(()),
                Ok(false) => Err(eother!("client connection is shutting down")),
                Err(e) => Err(e),
            }
        } else {
            Err(enoent!("client connect doesn't exist"))
        }
    }

    /// Accept one incoming connection from client.
    pub fn handle_incoming_connection(&self) -> Result<Option<Arc<ClientConnection>>> {
        if self.exiting.load(Ordering::Acquire) {
            return Err(eio!("server shutdown"));
        }

        match self.listener.accept() {
            Err(e) => Err(eio!(format!("failed to accept incoming connection, {}", e))),
            Ok(None) => Ok(None),
            Ok(Some(sock)) => {
                let id = self.next_id.fetch_add(1, Ordering::AcqRel);
                if id <= u32::MAX as u64 {
                    let client = Arc::new(ClientConnection::new(self.state.clone(), id, sock)?);
                    client.state.add(id, client.clone());
                    Ok(Some(client))
                } else {
                    // Running out of connection id, reject the incoming connection.
                    Ok(None)
                }
            }
        }
    }
}

impl AsRawFd for Server {
    fn as_raw_fd(&self) -> RawFd {
        self.listener.as_raw_fd()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use vmm_sys_util::tempdir::TempDir;

    #[test]
    #[ignore]
    fn test_new_server() {
        let tmpdir = TempDir::new().unwrap();
        let sock = tmpdir.as_path().to_str().unwrap().to_owned() + "/test_sock1";
        let server = Arc::new(Server::new(&sock).unwrap());

        assert_eq!(server.state.active_workers.load(Ordering::Relaxed), 0);
        Server::start(server.clone()).unwrap();
        std::thread::sleep(Duration::from_secs(1));
        assert_eq!(server.state.active_workers.load(Ordering::Relaxed), 1);

        let client = RemoteBlobMgr::new("".to_owned(), &server.sock).unwrap();
        client.connect().unwrap();
        std::thread::sleep(Duration::from_secs(1));
        assert_eq!(server.state.active_workers.load(Ordering::Relaxed), 2);
        client.shutdown();
        std::thread::sleep(Duration::from_secs(1));
        assert_eq!(server.state.active_workers.load(Ordering::Relaxed), 1);
        assert_eq!(server.state.clients.lock().unwrap().len(), 0);

        let client = RemoteBlobMgr::new("".to_owned(), &server.sock).unwrap();
        client.connect().unwrap();
        std::thread::sleep(Duration::from_secs(1));
        assert_eq!(server.state.active_workers.load(Ordering::Relaxed), 2);
        let client = Arc::new(client);
        client.start().unwrap();
        client.ping().unwrap();

        server.stop();
        std::thread::sleep(Duration::from_secs(1));
        assert_eq!(server.state.active_workers.load(Ordering::Relaxed), 0);
    }

    #[test]
    #[ignore]
    fn test_reconnect() {
        let tmpdir = TempDir::new().unwrap();
        let sock = tmpdir.as_path().to_str().unwrap().to_owned() + "/test_sock1";

        let server = Arc::new(Server::new(&sock).unwrap());
        Server::start(server.clone()).unwrap();

        let client = RemoteBlobMgr::new("".to_owned(), &server.sock).unwrap();
        client.connect().unwrap();
        std::thread::sleep(Duration::from_secs(4));
        client.start().unwrap();
        client.ping().unwrap();

        server.stop();
        std::thread::sleep(Duration::from_secs(4));
        let starttime = Instant::now();
        /* give 10secs more to try */
        while starttime.elapsed() < Duration::from_secs(10) {
            if server.state.active_workers.load(Ordering::Relaxed) == 0 {
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        assert_eq!(server.state.active_workers.load(Ordering::Relaxed), 0);
        drop(server);

        let server = Arc::new(Server::new(&sock).unwrap());
        Server::start(server).unwrap();
        client.ping().unwrap();
    }
}
