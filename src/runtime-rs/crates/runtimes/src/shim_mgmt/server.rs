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
}
