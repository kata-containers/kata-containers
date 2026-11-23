// Copyright (c) 2022 Alibaba Cloud
// Copyright (c) 2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

// Shim management service, this service starts a management http server on a socket
// and wire certain URL with a corresponding handler. When a command-line interface
// or further shim functions want the information corresponding to this, it can just
// send a GET request to the url, and the info will be in the response

#![allow(dead_code)] // some url's handler are *to be* developed

use std::{fs, os::unix::fs::PermissionsExt, path::Path, sync::Arc};

use anyhow::{Context, Result};
use common::Sandbox;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use shim_interface::{sb_storage_path, SHIM_MGMT_SOCK_NAME};
use tokio::net::{UnixListener, UnixStream};

use super::handlers::handler_mux;

/// The shim management server instance
pub struct MgmtServer {
    /// socket address(with prefix like hvsock://)
    pub s_addr: String,

    /// The sandbox instance
    pub sandbox: Arc<dyn Sandbox>,
}

impl MgmtServer {
    /// construct a new management server
    pub fn new(sid: &str, sandbox: Arc<dyn Sandbox>) -> Result<Self> {
        // make sure the storage path exists, and the socket file will be created in that path
        let kata_path = sb_storage_path();
        fs::create_dir_all(kata_path)
            .context(format!("failed to create kata path directory {kata_path}"))?;

        let s_addr = format!("unix://{kata_path}/{sid}/{SHIM_MGMT_SOCK_NAME}");

        Ok(Self { s_addr, sandbox })
    }

    // TODO(when metrics is supported): write metric addresses to fs
    // TODO(when metrics is supported): register shim metrics
    // TODO(when metrics is supported): register sandbox metrics
    // running management http server in an infinite loop, able to serve concurrent requests
    pub async fn run(self: Arc<Self>) {
        let listener = listener_from_path(self.s_addr.clone()).await.unwrap();
        let self_euid = nix::unistd::geteuid().as_raw();
        // start an infinite loop, which serves the incomming uds stream
        loop {
            let stream = accept_authorized(&listener, |uid| is_authorized_peer(uid, self_euid))
                .await
                .unwrap();
            let io = TokioIo::new(stream);
            let me = self.clone();
            // spawn a light weight thread to multiplex to the handler
            tokio::task::spawn(async move {
                if let Err(err) = http1::Builder::new()
                    .serve_connection(
                        io,
                        service_fn(|request| handler_mux(me.sandbox.clone(), request)),
                    )
                    .await
                {
                    warn!(sl!(), "Failed to serve connection: {:?}", err);
                }
            });
        }
    }
}

// from path, return a unix listener corresponding to that path,
// if the path(socket file) is not created, we create that here
async fn listener_from_path(path: String) -> Result<UnixListener> {
    // create the socket if not present
    let trim_path = path.strip_prefix("unix:").context("trim path")?;
    let file_path = Path::new("/").join(trim_path);
    let file_path = file_path.as_path();
    if let Some(parent_dir) = file_path.parent() {
        fs::create_dir_all(parent_dir).context("create parent dir")?;
    }
    // bind the socket and return the listener
    info!(sl!(), "mgmt-svr: binding to path {}", path);
    let listener = UnixListener::bind(file_path).context("bind address")?;
    // set the socket permissions explicitly instead of relying on the umask
    fs::set_permissions(file_path, fs::Permissions::from_mode(0o600))
        .context("set socket permissions")?;
    Ok(listener)
}

/// Whether a client may use the management API: root, or the user the shim
/// itself runs as.
fn is_authorized_peer(peer_uid: u32, self_euid: u32) -> bool {
    peer_uid == 0 || peer_uid == self_euid
}

/// Accept connections until one comes from a peer that `authorized` allows,
/// independently of the permissions on the socket file. Rejected connections
/// are closed by dropping them.
async fn accept_authorized(
    listener: &UnixListener,
    authorized: impl Fn(u32) -> bool,
) -> std::io::Result<UnixStream> {
    loop {
        let (stream, _) = listener.accept().await?;
        match stream.peer_cred() {
            Ok(cred) if authorized(cred.uid()) => return Ok(stream),
            Ok(cred) => warn!(
                sl!(),
                "mgmt-svr: rejecting connection from unauthorized peer, uid {} pid {:?}",
                cred.uid(),
                cred.pid()
            ),
            Err(err) => warn!(
                sl!(),
                "mgmt-svr: rejecting connection, failed to get peer credentials: {:?}", err
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::net::UnixStream as StdUnixStream,
        sync::atomic::{AtomicBool, AtomicU32, Ordering},
        time::Duration,
    };
    use test_utils::skip_if_not_root;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const TIMEOUT: Duration = Duration::from_secs(5);

    #[test]
    fn test_is_authorized_peer() {
        // (peer uid, shim euid, authorized)
        let tests = [
            (0, 0, true),
            (1000, 0, false),
            (1000, 1000, true),
            (0, 1000, true),
            (1001, 1000, false),
        ];

        for (i, &(peer_uid, self_euid, authorized)) in tests.iter().enumerate() {
            assert_eq!(
                is_authorized_peer(peer_uid, self_euid),
                authorized,
                "test[{i}]: peer uid {peer_uid}, shim euid {self_euid}"
            );
        }
    }

    #[tokio::test]
    async fn test_listener_from_path_sets_mode_0600() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SHIM_MGMT_SOCK_NAME);

        let _listener = listener_from_path(format!("unix://{}", path.display()))
            .await
            .unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    /// Check that the server closes `stream` without answering a request sent
    /// over it.
    async fn assert_closed_without_response(mut stream: UnixStream) {
        // The write may fail if the server has already closed the connection.
        let _ = stream.write_all(b"GET / HTTP/1.0\r\n\r\n").await;

        let mut resp = Vec::new();
        let read = tokio::time::timeout(TIMEOUT, stream.read_to_end(&mut resp)).await;

        assert!(read.is_ok(), "server must close a rejected connection");
        assert!(resp.is_empty(), "rejected connection must not be served");
    }

    #[tokio::test]
    async fn test_accept_authorized_rejects_and_keeps_accepting() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SHIM_MGMT_SOCK_NAME);
        let listener = UnixListener::bind(&path).unwrap();

        let allow = Arc::new(AtomicBool::new(false));
        let seen_uid = Arc::new(AtomicU32::new(u32::MAX));
        let accept = {
            let (allow, seen_uid) = (allow.clone(), seen_uid.clone());
            tokio::spawn(async move {
                accept_authorized(&listener, |uid| {
                    seen_uid.store(uid, Ordering::SeqCst);
                    allow.load(Ordering::SeqCst)
                })
                .await
            })
        };

        let rejected = UnixStream::connect(&path).await.unwrap();
        assert_closed_without_response(rejected).await;
        assert_eq!(
            seen_uid.load(Ordering::SeqCst),
            nix::unistd::geteuid().as_raw(),
            "policy must see the peer's effective UID"
        );

        allow.store(true, Ordering::SeqCst);
        let _client = UnixStream::connect(&path).await.unwrap();
        tokio::time::timeout(TIMEOUT, accept)
            .await
            .expect("must keep accepting after rejecting a connection")
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn test_accept_authorized_accepts_same_user() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SHIM_MGMT_SOCK_NAME);
        let listener = UnixListener::bind(&path).unwrap();
        let self_euid = nix::unistd::geteuid().as_raw();

        let accept = tokio::spawn(async move {
            accept_authorized(&listener, |uid| is_authorized_peer(uid, self_euid)).await
        });

        // The client runs as the same user as the server, whether that is root
        // or not, so the real policy lets it in.
        let _client = UnixStream::connect(&path).await.unwrap();
        tokio::time::timeout(TIMEOUT, accept)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }

    /// Connect to `path` with the effective UID set to `uid`. Only the helper
    /// thread's UID is changed, through the raw syscall (the libc wrapper
    /// changes every thread), so the rest of the test process stays root.
    fn connect_as_user(path: &Path, uid: u32) -> StdUnixStream {
        let path = path.to_owned();
        std::thread::spawn(move || {
            use nix::libc::{c_long, syscall, SYS_setresuid};

            // SAFETY: setresuid takes no pointers; -1 leaves an ID unchanged.
            let ret = unsafe { syscall(SYS_setresuid, -1 as c_long, uid as c_long, -1 as c_long) };
            assert_eq!(ret, 0, "setresuid: {}", std::io::Error::last_os_error());

            let stream = StdUnixStream::connect(&path).unwrap();
            stream.set_nonblocking(true).unwrap();
            stream
        })
        .join()
        .unwrap()
    }

    #[tokio::test]
    async fn test_accept_authorized_rejects_other_user() {
        skip_if_not_root!();

        // Make the socket reachable by every user, as if its permissions had
        // drifted, so that only the credential check stands in the way.
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        let path = dir.path().join(SHIM_MGMT_SOCK_NAME);
        let listener = UnixListener::bind(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o777)).unwrap();

        let self_euid = nix::unistd::geteuid().as_raw();
        let accept = tokio::spawn(async move {
            accept_authorized(&listener, |uid| is_authorized_peer(uid, self_euid)).await
        });

        const NOBODY: u32 = 65534;
        let rejected = UnixStream::from_std(connect_as_user(&path, NOBODY)).unwrap();
        assert_closed_without_response(rejected).await;

        let _root = UnixStream::connect(&path).await.unwrap();
        tokio::time::timeout(TIMEOUT, accept)
            .await
            .expect("root must still be accepted")
            .unwrap()
            .unwrap();
    }
}
