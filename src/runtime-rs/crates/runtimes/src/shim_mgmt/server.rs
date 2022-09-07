// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

// Shim management service, this service starts a management http server on a socket
// and wire certain URL with a corresponding handler. When a command-line interface
// or further shim functions want the information corresponding to this, it can just
// send a GET request to the url, and the info will be in the response

#![allow(dead_code)] // some url's handler are *to be* developed

use std::{fs, path::Path, sync::Arc};

use anyhow::{Context, Result};
use common::Sandbox;
use hyper::{server::conn::Http, service::service_fn};
use tokio::net::UnixListener;

use super::handlers::handler_mux;

pub(crate) const DIRECT_VOLUMN_PATH_KEY: &str = "path";
pub(crate) const DIRECT_VOLUMN_STATS_URL: &str = "/direct-volumn/stats";
pub(crate) const DIRECT_VOLUMN_RESIZE_URL: &str = "/direct-volumn/resize";
pub(crate) const AGENT_URL: &str = "/agent-url";
pub(crate) const IP_TABLE_URL: &str = "/iptables";
pub(crate) const IP6_TABLE_URL: &str = "/ip6tables";
pub(crate) const METRICS_URL: &str = "/metrics";

const SHIM_MGMT_SOCK_NAME: &str = "shim-monitor.sock";

/// The shim management server instance
pub struct MgmtServer {
    /// socket address(with prefix like hvsock://)
    pub s_addr: String,

    /// The sandbox instance
    pub sandbox: Arc<dyn Sandbox>,
}

impl MgmtServer {
    /// construct a new management server
    pub fn new(sid: &str, sandbox: Arc<dyn Sandbox>) -> Self {
        Self {
            s_addr: mgmt_socket_addr(sid.to_owned()),
            sandbox,
        }
    }

    // TODO(when metrics is supported): write metric addresses to fs
    // TODO(when metrics is supported): register shim metrics
    // TODO(when metrics is supported): register sandbox metrics
    // running management http server in an infinite loop, able to serve concurrent requests
    pub async fn run(self: Arc<Self>) {
        let listener = listener_from_path(self.s_addr.clone()).await.unwrap();
        // start an infinite loop, which serves the incomming uds stream
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let me = self.clone();
            // spawn a light weight thread to multiplex to the handler
            tokio::task::spawn(async move {
                if let Err(err) = Http::new()
                    .serve_connection(
                        stream,
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

// return sandbox's storage path
pub fn sb_storage_path() -> String {
    String::from("/run/kata")
}

// returns the address of the unix domain socket(UDS) for communication with shim
// management service using http
// normally returns "unix:///run/kata/{sid}/shim_monitor.sock"
pub fn mgmt_socket_addr(sid: String) -> String {
    let p = Path::new(&sb_storage_path())
        .join(sid)
        .join(SHIM_MGMT_SOCK_NAME);
    format!("unix://{}", p.to_string_lossy())
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
    UnixListener::bind(file_path).context("bind address")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mgmt_svr_test_sock_addr() {
        let sid = String::from("414123");
        let addr = mgmt_socket_addr(sid);
        assert_eq!(addr, "unix:///run/kata/414123/shim-monitor.sock");
    }
}
