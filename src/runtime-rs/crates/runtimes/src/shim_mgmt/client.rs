// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

// Defines the general client functions used by other components acting like
// clients. To be specific, a client first connect to the socket, then send
// request to destined URL, and finally handle the request(or not)

use std::{path::Path, path::PathBuf, time::Duration};

use super::server::mgmt_socket_addr;
use anyhow::{anyhow, Context, Result};
use hyper::{Body, Client, Response};
use hyperlocal::{UnixClientExt, UnixConnector, Uri};

/// Shim management client with timeout
pub struct MgmtClient {
    /// The socket *file path* on host file system
    sock_path: PathBuf,

    /// The http client connect to the long standing shim mgmt server
    client: Client<UnixConnector, Body>,

    /// Timeout value for each dial, usually 200ms will be enough
    /// For heavier workload, you may want longer timeout
    timeout: Option<Duration>,
}

impl MgmtClient {
    /// Construct a new client connecting to shim mgmt server
    pub fn new(sid: String, timeout: Option<Duration>) -> Result<Self> {
        let unix_socket_path = mgmt_socket_addr(sid);
        let s_addr = unix_socket_path
            .strip_prefix("unix:")
            .context("failed to strix prefix")?;
        let sock_path = Path::new("/").join(s_addr).as_path().to_owned();
        let client = Client::unix();
        Ok(Self {
            sock_path,
            client,
            timeout,
        })
    }

    /// The http GET method for client, return a raw response. Further handling should be done by caller.
    /// Parameter uri should be like "/agent-url" etc.
    pub async fn get(&self, uri: &str) -> Result<Response<Body>> {
        let url: hyper::Uri = Uri::new(&self.sock_path, uri).into();
        let work = self.client.get(url);
        match self.timeout {
            Some(timeout) => match tokio::time::timeout(timeout, work).await {
                Ok(result) => result.map_err(|e| anyhow!(e)),
                Err(_) => Err(anyhow!("TIMEOUT")),
            },
            // if timeout not set, work executes directly
            None => work.await.context("failed to GET"),
        }
    }
}
