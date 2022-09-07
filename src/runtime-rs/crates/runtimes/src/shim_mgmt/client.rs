// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#![allow(dead_code)]

// Defines the general client functions used by other components acting like
// clients. To be specific, a client first connect to the socket, then send
// request to destined URL, and finally handle the request(or not)

use std::{path::Path, path::PathBuf, time::Duration};

use super::server::mgmt_socket_addr;
use anyhow::{Context, Result};
use hyper::{Body, Client, Method, Request, Response};
use hyperlocal::{UnixClientExt, UnixConnector, Uri};

/// Shim management client with timeout
pub struct MgmtClient {
    /// The socket *file path* on host file system
    s_path: PathBuf,

    /// The http client connect to the long standing shim mgmt server
    client: Client<UnixConnector, Body>,

    /// timeout value for each dial
    timeout: Option<Duration>,
}

impl MgmtClient {
    /// Construct a new client connecting to shim mgmt server
    pub fn new(sid: String, timeout: Option<Duration>) -> Result<Self> {
        let unix_socket_path = mgmt_socket_addr(sid);
        let s_addr = unix_socket_path
            .strip_prefix("unix:")
            .context("failed to strix prefix")?;
        let s_path = Path::new("/").join(s_addr).as_path().to_owned();
        let client = Client::unix();
        Ok(Self {
            s_path,
            client,
            timeout,
        })
    }

    /// The http GET method for client, return a raw response. Further handling should be done by caller.
    /// Parameter uri should be like "/agent-url" etc.
    pub async fn get(&self, uri: &str) -> Result<Response<Body>> {
        let url: hyper::Uri = Uri::new(&self.s_path, uri).into();
        let response = self.client.get(url).await.context("failed to GET")?;
        Ok(response)
    }

    /// The http PUT method for client
    pub async fn put(&self, uri: &str) -> Result<Response<Body>> {
        let url: hyper::Uri = Uri::new(&self.s_path, uri).into();
        let req = Request::builder()
            .method(Method::PUT)
            .uri(url)
            .body(Body::from(""))
            .expect("request builder");
        let response = self.client.request(req).await?;
        Ok(response)
    }

    /// The http POST method for client
    pub async fn post(&self, uri: &str) -> Result<Response<Body>> {
        let url: hyper::Uri = Uri::new(&self.s_path, uri).into();
        let req = Request::builder()
            .method(Method::POST)
            .uri(url)
            .body(Body::from(""))
            .expect("request builder");
        let response = self.client.request(req).await?;
        Ok(response)
    }
}
