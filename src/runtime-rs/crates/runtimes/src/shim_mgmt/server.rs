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

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{anyhow, Context, Result};
use common::Sandbox;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use shim_interface::{sb_storage_path, SHIM_MGMT_SOCK_NAME};
use tokio::net::UnixListener;

use super::handlers::handler_mux;

fn socket_path(sid: &str) -> PathBuf {
    Path::new(sb_storage_path())
        .join(sid)
        .join(SHIM_MGMT_SOCK_NAME)
}

/// The socket is only removed by a completed sandbox cleanup, so a leftover
/// one means a previous shim for this ID is still running or failed to clean
/// up. Its state must not be reused, so the socket is never removed here.
pub fn ensure_no_leftover_sandbox(sid: &str) -> Result<()> {
    ensure_no_leftover_socket(&socket_path(sid), sid)
}

fn ensure_no_leftover_socket(path: &Path, sid: &str) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(anyhow!(
            "{} exists: a previous shim for sandbox {sid} is still running or did not \
             finish its cleanup",
            path.display()
        )),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("check {}", path.display())),
    }
}

/// Whether a running shim still serves the management socket of the sandbox.
///
/// A shim stops serving it before it stops its task server, and the kernel
/// marks a dying shim as exiting before it closes its descriptors. So once
/// containerd has seen the shim go away, the socket is either not served or
/// served by a process that is exiting.
pub fn shim_serves_sandbox(sid: &str) -> bool {
    socket_served_by_live_process(&socket_path(sid), Path::new("/proc"))
}

// PF_EXITING from include/linux/sched.h.
const PF_EXITING: u64 = 0x4;

fn socket_served_by_live_process(path: &Path, proc_root: &Path) -> bool {
    let Ok(stream) = std::os::unix::net::UnixStream::connect(path) else {
        return false;
    };
    // The credentials of a connected stream socket are those of the process
    // that called listen().
    let Ok(cred) =
        nix::sys::socket::getsockopt(&stream, nix::sys::socket::sockopt::PeerCredentials)
    else {
        return true;
    };
    match fs::read_to_string(proc_root.join(cred.pid().to_string()).join("stat")) {
        Ok(stat) => !process_is_exiting(&stat),
        Err(_) => false,
    }
}

fn process_is_exiting(stat: &str) -> bool {
    // The command name can contain spaces and parentheses, the fields after it cannot.
    let Some((_, fields)) = stat.rsplit_once(')') else {
        return false;
    };
    let fields: Vec<&str> = fields.split_whitespace().collect();
    let state = fields.first().copied().unwrap_or_default();
    let flags = fields
        .get(6)
        .and_then(|f| f.parse::<u64>().ok())
        .unwrap_or_default();
    matches!(state, "Z" | "X" | "x") || flags & PF_EXITING != 0
}

/// The shim management server instance
pub struct MgmtServer {
    listener: UnixListener,

    /// The sandbox instance
    sandbox: Arc<dyn Sandbox>,
}

impl MgmtServer {
    /// Binds the management socket, failing if a previous shim left one.
    pub fn new(sid: &str, sandbox: Arc<dyn Sandbox>) -> Result<Self> {
        let path = socket_path(sid);
        if let Some(parent_dir) = path.parent() {
            fs::create_dir_all(parent_dir)
                .with_context(|| format!("create {}", parent_dir.display()))?;
        }
        ensure_no_leftover_sandbox(sid)?;
        info!(sl!(), "mgmt-svr: binding to path {}", path.display());
        let listener =
            UnixListener::bind(&path).with_context(|| format!("bind {}", path.display()))?;

        Ok(Self { listener, sandbox })
    }

    // TODO(when metrics is supported): write metric addresses to fs
    // TODO(when metrics is supported): register shim metrics
    // TODO(when metrics is supported): register sandbox metrics
    // running management http server in an infinite loop, able to serve concurrent requests
    pub async fn run(self) {
        let Self { listener, sandbox } = self;
        // start an infinite loop, which serves the incomming uds stream
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let io = TokioIo::new(stream);
            let sandbox = sandbox.clone();
            // spawn a light weight thread to multiplex to the handler
            tokio::task::spawn(async move {
                if let Err(err) = http1::Builder::new()
                    .serve_connection(
                        io,
                        service_fn(|request| handler_mux(sandbox.clone(), request)),
                    )
                    .await
                {
                    warn!(sl!(), "Failed to serve connection: {:?}", err);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leftover_socket_is_reported_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SHIM_MGMT_SOCK_NAME);
        ensure_no_leftover_socket(&path, "sid").unwrap();

        let _listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        let err = ensure_no_leftover_socket(&path, "sid").unwrap_err();
        assert!(err.to_string().contains("did not finish its cleanup"));
        assert!(path.exists());
    }

    #[test]
    fn test_socket_is_served_only_by_a_live_listener() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SHIM_MGMT_SOCK_NAME);
        let proc_root = Path::new("/proc");
        assert!(!socket_served_by_live_process(&path, proc_root));

        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        assert!(socket_served_by_live_process(&path, proc_root));

        // The listener's process is reported as exiting.
        let fake_proc = dir.path().join("proc");
        let pid_dir = fake_proc.join(std::process::id().to_string());
        fs::create_dir_all(&pid_dir).unwrap();
        fs::write(
            pid_dir.join("stat"),
            "1 (containerd-shim) R 1 1 1 0 -1 4194372 0 0 0 0",
        )
        .unwrap();
        assert!(!socket_served_by_live_process(&path, &fake_proc));

        drop(listener);
        assert!(path.exists());
        assert!(!socket_served_by_live_process(&path, proc_root));
    }

    #[test]
    fn test_process_is_exiting() {
        assert!(!process_is_exiting("7 (a) b) S 1 1 1 0 -1 4194560 0"));
        assert!(process_is_exiting("7 (a) b) S 1 1 1 0 -1 4194564 0"));
        assert!(process_is_exiting("7 (shim) Z 1 1 1 0 -1 4194560 0"));
        assert!(!process_is_exiting("garbage"));
    }
}
