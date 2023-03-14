// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::fs::File;
use std::io::Result;
use std::mem;
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use nix::sys::select::{select, FdSet};
use vm_memory::ByteValued;

use crate::cache::state::{BlobRangeMap, RangeMap};
use crate::device::{BlobInfo, BlobIoRange, BlobObject};
use crate::remote::connection::Endpoint;
use crate::remote::message::{
    FetchRangeReply, FetchRangeRequest, FetchRangeResult, GetBlobReply, GetBlobRequest, HeaderFlag,
    MsgHeader, MsgValidator, RequestCode,
};

const REQUEST_TIMEOUT_SEC: u64 = 4;
const RANGE_MAP_SHIFT: u64 = 18;
const RANGE_MAP_MASK: u64 = (1 << RANGE_MAP_SHIFT) - 1;

/// Manager to access and cache blob objects managed by remote blob manager.
///
/// A `RemoteBlobMgr` object may be used to access services from a remote blob manager, and cache
/// blob information to improve performance.
pub struct RemoteBlobMgr {
    remote_blobs: Arc<RemoteBlobs>,
    server_connection: Arc<ServerConnection>,
    workdir: String,
}

impl RemoteBlobMgr {
    /// Create a new instance of `RemoteBlobMgr`.
    pub fn new(workdir: String, sock: &str) -> Result<Self> {
        let remote_blobs = Arc::new(RemoteBlobs::new());
        let conn = ServerConnection::new(sock, remote_blobs.clone());

        Ok(RemoteBlobMgr {
            remote_blobs,
            server_connection: Arc::new(conn),
            workdir,
        })
    }

    /// Connect to remote blob manager.
    pub fn connect(&self) -> Result<()> {
        self.server_connection.connect().map(|_| ())
    }

    /// Start to handle communication messages.
    pub fn start(&self) -> Result<()> {
        ServerConnection::start(self.server_connection.clone())
    }

    /// Shutdown the `RemoteblogMgr` instance.
    pub fn shutdown(&self) {
        self.server_connection.close();
        self.remote_blobs.reset();
    }

    /// Ping remote blog manager server.
    pub fn ping(&self) -> Result<()> {
        self.server_connection.call_ping()
    }

    /// Get an `BlobObject` trait object to access the specified blob.
    pub fn get_blob_object(&self, blob_info: &Arc<BlobInfo>) -> Result<Arc<dyn BlobObject>> {
        if let Some(blob) = self.remote_blobs.get_blob(blob_info) {
            return Ok(blob);
        }

        loop {
            let (file, base, token) = self.server_connection.call_get_blob(blob_info)?;
            let file = Arc::new(file);
            let blob = RemoteBlob::new(
                blob_info.clone(),
                self.server_connection.clone(),
                file,
                base,
                token,
                &self.workdir,
            )?;
            let blob = Arc::new(blob);
            if let Some(blob) = self.remote_blobs.add_blob(blob, token) {
                return Ok(blob);
            }
        }
    }
}

struct RemoteBlobs {
    generation: AtomicU32,
    active_blobs: Mutex<Vec<Arc<RemoteBlob>>>,
}

impl RemoteBlobs {
    fn new() -> Self {
        Self {
            generation: AtomicU32::new(1),
            active_blobs: Mutex::new(Vec::new()),
        }
    }

    fn reset(&self) {
        self.active_blobs.lock().unwrap().truncate(0);
    }

    fn add_blob(&self, blob: Arc<RemoteBlob>, token: u64) -> Option<Arc<RemoteBlob>> {
        let mut guard = self.active_blobs.lock().unwrap();
        for b in guard.iter() {
            if blob.blob_info.blob_id() == b.blob_info.blob_id() {
                return Some(b.clone());
            }
        }

        if (token >> 32) as u32 == self.get_generation() {
            guard.push(blob.clone());
            return Some(blob);
        }

        None
    }

    fn get_blob(&self, blob_info: &Arc<BlobInfo>) -> Option<Arc<RemoteBlob>> {
        let guard = self.active_blobs.lock().unwrap();

        for blob in guard.iter() {
            if blob.blob_info.blob_id() == blob_info.blob_id() {
                return Some(blob.clone());
            }
        }

        None
    }

    fn get_generation(&self) -> u32 {
        self.generation.load(Ordering::Acquire)
    }

    fn notify_disconnect(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        for blob in self.active_blobs.lock().unwrap().iter() {
            blob.token.store(0, Ordering::Release);
        }
    }
}

/// Struct to access and cache blob object managed by remote blob manager.
///
/// The `RemoteBlob` structure acts as a proxy to access a blob managed by remote blob manager.
/// It has a separate data plane and control plane. A file descriptor will be received from the
/// remote blob manager, so all data access requests will be served by directly access the file
/// descriptor. And a communication channel will be used to communicate control message between
/// the client and the remote blob manager. To improve control plane performance, it may cache
/// blob metadata and chunk map to avoid unnecessary control messages.
struct RemoteBlob {
    blob_info: Arc<BlobInfo>,
    conn: Arc<ServerConnection>,
    map: Arc<BlobRangeMap>,
    file: Arc<File>,
    base: u64,
    token: AtomicU64,
}

impl RemoteBlob {
    /// Create a new instance of `RemoteBlob`.
    fn new(
        blob_info: Arc<BlobInfo>,
        conn: Arc<ServerConnection>,
        file: Arc<File>,
        base: u64,
        token: u64,
        work_dir: &str,
    ) -> Result<Self> {
        let blob_path = format!("{}/{}", work_dir, blob_info.blob_id());
        let count = (blob_info.uncompressed_size() + RANGE_MAP_MASK) >> RANGE_MAP_SHIFT;
        let map = BlobRangeMap::new(&blob_path, count as u32, RANGE_MAP_SHIFT as u32)?;
        debug_assert!(count <= u32::MAX as u64);

        Ok(RemoteBlob {
            blob_info,
            map: Arc::new(map),
            conn,
            file,
            base,
            token: AtomicU64::new(token),
        })
    }
}

impl AsRawFd for RemoteBlob {
    fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}

impl BlobObject for RemoteBlob {
    fn base_offset(&self) -> u64 {
        self.base
    }

    fn is_all_data_ready(&self) -> bool {
        self.map.is_range_all_ready()
    }

    fn fetch_range_compressed(&self, _offset: u64, _size: u64) -> Result<usize> {
        Err(enosys!())
    }

    fn fetch_range_uncompressed(&self, offset: u64, size: u64) -> Result<usize> {
        match self.map.is_range_ready(offset, size) {
            Ok(true) => Ok(0),
            _ => self.conn.call_fetch_range(self, offset, size),
        }
    }

    fn prefetch_chunks(&self, _range: &BlobIoRange) -> Result<usize> {
        Err(enosys!())
    }
}

#[derive(Debug, Eq, PartialEq)]
enum RequestStatus {
    Waiting,
    Reconnect,
    Timeout,
    Finished,
}

#[allow(dead_code)]
enum RequestResult {
    None,
    Reconnect,
    Noop,
    GetBlob(u32, u64, u64, Option<File>),
    FetchRange(u32, u64),
}

struct Request {
    tag: u64,
    condvar: Condvar,
    state: Mutex<(RequestStatus, RequestResult)>,
}

impl Request {
    fn new(tag: u64) -> Self {
        Request {
            tag,
            condvar: Condvar::new(),
            state: Mutex::new((RequestStatus::Waiting, RequestResult::None)),
        }
    }

    fn wait_for_result(&self) {
        let mut guard = self.state.lock().unwrap();

        while guard.0 == RequestStatus::Waiting {
            let res = self
                .condvar
                .wait_timeout(guard, Duration::from_secs(REQUEST_TIMEOUT_SEC))
                .unwrap();
            let tor = res.1;

            guard = res.0;
            if guard.0 == RequestStatus::Finished || guard.0 == RequestStatus::Reconnect {
                return;
            } else if tor.timed_out() {
                guard.0 = RequestStatus::Timeout;
            }
        }
    }

    fn set_result(&self, result: RequestResult) {
        let mut guard = self.state.lock().unwrap();

        match guard.0 {
            RequestStatus::Waiting | RequestStatus::Timeout | RequestStatus::Reconnect => {
                guard.1 = result;
                guard.0 = RequestStatus::Finished;
                self.condvar.notify_all();
            }
            RequestStatus::Finished => {
                debug!("received duplicated reply");
            }
        }
    }
}

/// Struct to maintain state for a connection to remote blob manager.
struct ServerConnection {
    sock: String,
    tag: AtomicU64,
    exiting: AtomicBool,
    conn: Mutex<Option<Endpoint>>,
    ready: Condvar,
    requests: Mutex<HashMap<u64, Arc<Request>>>,
    remote_blobs: Arc<RemoteBlobs>,
}

impl ServerConnection {
    fn new(sock: &str, remote_blobs: Arc<RemoteBlobs>) -> Self {
        ServerConnection {
            sock: sock.to_owned(),
            tag: AtomicU64::new(1),
            exiting: AtomicBool::new(false),
            conn: Mutex::new(None),
            ready: Condvar::new(),
            requests: Mutex::new(HashMap::new()),
            remote_blobs,
        }
    }

    fn connect(&self) -> Result<bool> {
        let mut guard = self.get_connection()?;
        if guard.is_some() {
            return Ok(false);
        }

        match Endpoint::connect(&self.sock) {
            Ok(v) => {
                *guard = Some(v);
                Ok(true)
            }
            Err(e) => {
                error!("cannot connect to remote blob manager, {}", e);
                Err(eio!())
            }
        }
    }

    fn close(&self) {
        if !self.exiting.swap(true, Ordering::AcqRel) {
            self.disconnect();
        }
    }

    fn start(client: Arc<ServerConnection>) -> Result<()> {
        std::thread::spawn(move || loop {
            // Ensure connection is ready.
            match client.get_connection() {
                Ok(guard) => {
                    if guard.is_none() {
                        drop(client.ready.wait(guard));
                    } else {
                        drop(guard);
                    }
                }
                Err(_) => continue,
            }

            let _ = client.handle_reply();
        });

        Ok(())
    }

    // Only works for single-threaded context.
    fn handle_reply(&self) -> Result<()> {
        let mut nr;
        let mut rfd = FdSet::new();
        let mut efd = FdSet::new();

        loop {
            {
                rfd.clear();
                efd.clear();
                match self.get_connection()?.as_ref() {
                    None => return Err(eio!()),
                    Some(conn) => {
                        rfd.insert(conn.as_raw_fd());
                        efd.insert(conn.as_raw_fd());
                        nr = conn.as_raw_fd() + 1;
                    }
                }
            }
            let _ = select(nr, Some(&mut rfd), None, Some(&mut efd), None)
                .map_err(|e| eother!(format!("{}", e)))?;

            let mut guard = self.get_connection()?;
            let (hdr, files) = match guard.as_mut() {
                None => return Err(eio!()),
                Some(conn) => conn.recv_header().map_err(|_e| eio!())?,
            };
            if !hdr.is_valid() {
                return Err(einval!());
            }
            let body_size = hdr.get_size() as usize;

            match hdr.get_code() {
                RequestCode::MaxCommand => return Err(eother!()),
                RequestCode::Noop => self.handle_result(hdr.get_tag(), RequestResult::Noop),
                RequestCode::GetBlob => {
                    self.handle_get_blob_reply(guard, &hdr, body_size, files)?;
                }
                RequestCode::FetchRange => {
                    self.handle_fetch_range_reply(guard, &hdr, body_size, files)?;
                }
            }
        }
    }

    fn call_ping(&self) -> Result<()> {
        'next_iter: loop {
            let req = self.create_request();
            let hdr = MsgHeader::new(
                req.tag,
                RequestCode::Noop,
                HeaderFlag::NEED_REPLY.bits(),
                0u32,
            );
            let msg = [0u8; 0];

            self.send_msg(&hdr, &msg)?;
            match self.wait_for_result(&req)? {
                RequestResult::Noop => return Ok(()),
                RequestResult::Reconnect => continue 'next_iter,
                _ => return Err(eother!()),
            }
        }
    }

    fn call_get_blob(&self, blob_info: &Arc<BlobInfo>) -> Result<(File, u64, u64)> {
        if blob_info.blob_id().len() >= 256 {
            return Err(einval!("blob id is too large"));
        }

        'next_iter: loop {
            let req = self.create_request();
            let hdr = MsgHeader::new(
                req.tag,
                RequestCode::GetBlob,
                HeaderFlag::NEED_REPLY.bits(),
                std::mem::size_of::<GetBlobRequest>() as u32,
            );
            let generation = self.remote_blobs.get_generation();
            let msg = GetBlobRequest::new(generation, blob_info.blob_id());

            self.send_msg(&hdr, &msg)?;
            match self.wait_for_result(&req)? {
                RequestResult::GetBlob(result, token, base, file) => {
                    if result != 0 {
                        return Err(std::io::Error::from_raw_os_error(result as i32));
                    } else if (token >> 32) as u32 != self.remote_blobs.get_generation() {
                        continue 'next_iter;
                    } else if let Some(file) = file {
                        return Ok((file, base, token));
                    } else {
                        return Err(einval!());
                    }
                }
                RequestResult::Reconnect => continue 'next_iter,
                _ => return Err(eother!()),
            }
        }
    }

    fn call_fetch_range(&self, blob: &RemoteBlob, start: u64, count: u64) -> Result<usize> {
        'next_iter: loop {
            let token = blob.token.load(Ordering::Acquire);
            if (token >> 32) as u32 != self.remote_blobs.get_generation() {
                self.reopen_blob(blob)?;
                continue 'next_iter;
            }

            let req = self.create_request();
            let hdr = MsgHeader::new(
                req.tag,
                RequestCode::FetchRange,
                HeaderFlag::NEED_REPLY.bits(),
                std::mem::size_of::<GetBlobRequest>() as u32,
            );
            let msg = FetchRangeRequest::new(token, start, count);
            self.send_msg(&hdr, &msg)?;
            match self.wait_for_result(&req)? {
                RequestResult::FetchRange(result, size) => {
                    if result == FetchRangeResult::Success as u32 {
                        return Ok(size as usize);
                    } else if result == FetchRangeResult::GenerationMismatch as u32 {
                        continue 'next_iter;
                    } else {
                        return Err(std::io::Error::from_raw_os_error(count as i32));
                    }
                }
                RequestResult::Reconnect => continue 'next_iter,
                _ => return Err(eother!()),
            }
        }
    }

    fn reopen_blob(&self, blob: &RemoteBlob) -> Result<()> {
        'next_iter: loop {
            let req = self.create_request();
            let hdr = MsgHeader::new(
                req.tag,
                RequestCode::GetBlob,
                HeaderFlag::NEED_REPLY.bits(),
                std::mem::size_of::<GetBlobRequest>() as u32,
            );
            let generation = self.remote_blobs.get_generation();
            let msg = GetBlobRequest::new(generation, blob.blob_info.blob_id());

            self.send_msg(&hdr, &msg)?;
            match self.wait_for_result(&req)? {
                RequestResult::GetBlob(result, token, _base, file) => {
                    if result != 0 {
                        return Err(std::io::Error::from_raw_os_error(result as i32));
                    } else if (token >> 32) as u32 != self.remote_blobs.get_generation() {
                        continue 'next_iter;
                    } else if let Some(_file) = file {
                        blob.token.store(token, Ordering::Release);
                        return Ok(());
                    } else {
                        return Err(einval!());
                    }
                }
                RequestResult::Reconnect => continue 'next_iter,
                _ => return Err(eother!()),
            }
        }
    }

    fn get_next_tag(&self) -> u64 {
        self.tag.fetch_add(1, Ordering::AcqRel)
    }

    fn create_request(&self) -> Arc<Request> {
        let tag = self.get_next_tag();
        let request = Arc::new(Request::new(tag));

        self.requests.lock().unwrap().insert(tag, request.clone());

        request
    }

    fn get_connection(&self) -> Result<MutexGuard<Option<Endpoint>>> {
        if self.exiting.load(Ordering::Relaxed) {
            Err(eio!())
        } else {
            Ok(self.conn.lock().unwrap())
        }
    }

    fn send_msg<T: Sized>(&self, hdr: &MsgHeader, msg: &T) -> Result<()> {
        if let Ok(mut guard) = self.get_connection() {
            if let Some(conn) = guard.as_mut() {
                if conn.send_message(hdr, msg, None).is_ok() {
                    return Ok(());
                }
            }
        }

        let start = Instant::now();
        self.disconnect();
        loop {
            self.reconnect();
            if let Ok(mut guard) = self.get_connection() {
                if let Some(conn) = guard.as_mut() {
                    if conn.send_message(hdr, msg, None).is_ok() {
                        return Ok(());
                    }
                }
            }

            self.disconnect();
            if let Some(end) = start.checked_add(Duration::from_secs(REQUEST_TIMEOUT_SEC)) {
                let now = Instant::now();
                if end < now {
                    return Err(eio!());
                }
            } else {
                return Err(eio!());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn reconnect(&self) {
        if let Ok(true) = self.connect() {
            let guard = self.requests.lock().unwrap();
            for entry in guard.iter() {
                let mut state = entry.1.state.lock().unwrap();
                if state.0 == RequestStatus::Waiting {
                    state.0 = RequestStatus::Reconnect;
                    entry.1.condvar.notify_all();
                }
            }
        }
    }

    fn disconnect(&self) {
        self.remote_blobs.notify_disconnect();

        let mut guard = self.conn.lock().unwrap();
        if let Some(conn) = guard.as_mut() {
            conn.close();
        }
        *guard = None;
    }

    fn wait_for_result(&self, request: &Arc<Request>) -> Result<RequestResult> {
        request.wait_for_result();

        let mut guard = self.requests.lock().unwrap();
        match guard.remove(&request.tag) {
            None => Err(enoent!()),
            Some(entry) => {
                let mut guard2 = entry.state.lock().unwrap();
                match guard2.0 {
                    RequestStatus::Waiting => panic!("should not happen"),
                    RequestStatus::Timeout => Err(eio!()),
                    RequestStatus::Reconnect => Ok(RequestResult::Reconnect),
                    RequestStatus::Finished => {
                        let mut val = RequestResult::None;
                        mem::swap(&mut guard2.1, &mut val);
                        Ok(val)
                    }
                }
            }
        }
    }

    fn handle_result(&self, tag: u64, result: RequestResult) {
        let requests = self.requests.lock().unwrap();

        match requests.get(&tag) {
            None => debug!("no request for tag {} found, may have timed out", tag),
            Some(request) => request.set_result(result),
        }
    }

    fn handle_get_blob_reply(
        &self,
        mut guard: MutexGuard<Option<Endpoint>>,
        hdr: &MsgHeader,
        body_size: usize,
        files: Option<Vec<File>>,
    ) -> Result<()> {
        if body_size != mem::size_of::<GetBlobReply>() {
            return Err(einval!());
        }
        let (size, data) = match guard.as_mut() {
            None => return Err(einval!()),
            Some(conn) => conn.recv_data(body_size).map_err(|_e| eio!())?,
        };
        if size != body_size {
            return Err(eio!());
        }
        drop(guard);

        let mut msg = GetBlobReply::new(0, 0, 0);
        msg.as_mut_slice().copy_from_slice(&data);
        if !msg.is_valid() {
            return Err(einval!());
        } else if msg.result != 0 {
            self.handle_result(
                hdr.get_tag(),
                RequestResult::GetBlob(msg.result, msg.token, msg.base, None),
            );
        } else {
            if files.is_none() {
                return Err(einval!());
            }
            // Safe because we have just validated files is not none.
            let mut files = files.unwrap();
            if files.len() != 1 {
                return Err(einval!());
            }
            // Safe because we have just validated files[0] is valid.
            let file = files.pop().unwrap();
            self.handle_result(
                hdr.get_tag(),
                RequestResult::GetBlob(msg.result, msg.token, msg.base, Some(file)),
            );
        }

        Ok(())
    }

    fn handle_fetch_range_reply(
        &self,
        mut guard: MutexGuard<Option<Endpoint>>,
        hdr: &MsgHeader,
        body_size: usize,
        files: Option<Vec<File>>,
    ) -> Result<()> {
        if body_size != mem::size_of::<FetchRangeReply>() || files.is_some() {
            return Err(einval!());
        }
        let (size, data) = match guard.as_mut() {
            None => return Err(einval!()),
            Some(conn) => conn.recv_data(body_size).map_err(|_e| eio!())?,
        };
        if size != body_size {
            return Err(eio!());
        }
        drop(guard);

        let mut msg = FetchRangeReply::new(0, 0, 0);
        msg.as_mut_slice().copy_from_slice(&data);
        if !msg.is_valid() {
            return Err(einval!());
        } else {
            self.handle_result(
                hdr.get_tag(),
                RequestResult::FetchRange(msg.result, msg.count),
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request() {
        let req = Arc::new(Request::new(1));
        let req1 = req.clone();

        assert_eq!(req.tag, 1);
        {
            let guard = req.state.lock().unwrap();
            assert_eq!(guard.0, RequestStatus::Waiting);
            matches!(guard.1, RequestResult::None);
        }

        let (sender, receiver) = std::sync::mpsc::channel::<bool>();
        std::thread::spawn(move || {
            let _ = receiver.recv().unwrap();
            {
                let mut guard = req1.state.lock().unwrap();
                guard.0 = RequestStatus::Reconnect;
            }

            let _ = receiver.recv().unwrap();
            req1.set_result(RequestResult::Reconnect);
        });

        {
            req.wait_for_result();
            let mut guard = req.state.lock().unwrap();
            assert_eq!(guard.0, RequestStatus::Timeout);
            guard.0 = RequestStatus::Waiting;
        }

        sender.send(true).unwrap();
        {
            req.wait_for_result();
            let mut guard = req.state.lock().unwrap();
            assert_eq!(guard.0, RequestStatus::Reconnect);
            guard.0 = RequestStatus::Waiting;
        }

        sender.send(true).unwrap();
        {
            req.wait_for_result();
            let guard = req.state.lock().unwrap();
            assert_eq!(guard.0, RequestStatus::Finished);
            matches!(guard.1, RequestResult::Reconnect);
        }
    }
}
