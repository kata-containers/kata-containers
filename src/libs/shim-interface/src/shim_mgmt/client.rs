#![allow(dead_code)]
// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

// Defines the general client functions used by other components acting like
// clients. To be specific, a client first connect to the socket, then send
// request to destined URL, and finally handle the request(or not)

use std::{path::Path, path::PathBuf, time::Duration};

use crate::mgmt_socket_addr;
use anyhow::{anyhow, Context, Result};
use hyper::{Body, Client, Method, Request, Response};
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
    pub fn new(sid: &str, timeout: Option<Duration>) -> Result<Self> {
        let unix_socket_path = mgmt_socket_addr(sid).context("Failed to get unix socket path")?;
        let s_addr = unix_socket_path
            .strip_prefix("unix:")
            .context("failed to strip prefix")?;
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
        let req = Request::builder()
            .method(Method::GET)
            .uri(url)
            .body(Body::empty())?;
        self.send_request(req).await
    }

    /// The HTTP Post method for client
    pub async fn post(
        &self,
        uri: &str,
        content_type: &str,
        content: &str,
    ) -> Result<Response<Body>> {
        let url: hyper::Uri = Uri::new(&self.sock_path, uri).into();

        // build body from content
        let body = Body::from(content.to_string());
        let req = Request::builder()
            .method(Method::POST)
            .uri(url)
            .header("content-type", content_type)
            .body(body)?;
        self.send_request(req).await
    }

    /// The http PUT method for client
    pub async fn put(&self, uri: &str, data: Vec<u8>) -> Result<Response<Body>> {
        let url: hyper::Uri = Uri::new(&self.sock_path, uri).into();
        let req = Request::builder()
            .method(Method::PUT)
            .uri(url)
            .body(Body::from(data))?;
        self.send_request(req).await
    }

    async fn send_request(&self, req: Request<Body>) -> Result<Response<Body>> {
        let msg = format!("Request ({:?}) to uri {:?}", req.method(), req.uri());
        let resp = self.client.request(req);
        match self.timeout {
            Some(timeout) => match tokio::time::timeout(timeout, resp).await {
                Ok(result) => result.map_err(|e| anyhow!(e)),
                Err(_) => Err(anyhow!("{:?} timeout after {:?}", msg, self.timeout)),
            },
            // if client timeout is not set, request waits with no deadline
            None => resp.await.context(format!("{:?} failed", msg)),
        }
    }
}
