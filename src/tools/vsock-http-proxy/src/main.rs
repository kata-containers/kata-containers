// Copyright (c) 2026 The Kata Containers Authors
//
// SPDX-License-Identifier: Apache-2.0

//! vsock-http-proxy bridges a guest-side HTTP service to a virtio-vsock port
//! so that host-side clients can reach in-guest HTTP services without requiring
//! a routable network inside the VM.
//!
//! Each instance handles a single `vsock_port:http_port` mapping supplied via
//! `--port=<vsock_port>:<http_port>`.

use std::convert::Infallible;
use std::str::FromStr as _;

use anyhow::{anyhow, Context as _, Result};
use clap::Parser;
use hyper::client::HttpConnector;
use hyper::service::service_fn;
use hyper::{Body, Client, Request, Response, Uri};
use tokio_vsock::VsockListener;

/// Bridge a guest-side HTTP service to a virtio-vsock port.
///
/// Listens for VSOCK connections from the host on vsock_port and
/// reverse-proxies each HTTP request to 127.0.0.1:http_port inside the guest.
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Port mapping as vsock_port:http_port.
    #[clap(long, value_name = "VSOCK_PORT:HTTP_PORT")]
    port: String,
}

#[derive(Clone, Copy, Debug)]
struct PortMapping {
    vsock_port: u32,
    http_port:  u16,
}

impl std::fmt::Display for PortMapping {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.vsock_port, self.http_port)
    }
}

fn parse_mapping(s: &str) -> Result<PortMapping> {
    let (vsock_str, http_str) = s
        .split_once(':')
        .ok_or_else(|| anyhow!("mapping {s:?}: expected vsock_port:http_port"))?;

    let vsock_port = vsock_str
        .parse::<u32>()
        .with_context(|| format!("mapping {s:?}: invalid vsock port"))?;
    anyhow::ensure!(vsock_port != 0, "mapping {s:?}: vsock port 0 is invalid");

    let http_port = http_str
        .parse::<u16>()
        .with_context(|| format!("mapping {s:?}: invalid http port"))?;
    anyhow::ensure!(http_port != 0, "mapping {s:?}: http port 0 is invalid");

    Ok(PortMapping {
        vsock_port,
        http_port,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let mapping = parse_mapping(&args.port)?;

    if let Err(e) = serve_mapping(mapping).await {
        eprintln!("vsock-http-proxy: mapping {mapping}: {e:#}");
    }
    Ok(())
}

/// Bind a VSOCK listener and reverse-proxy HTTP requests to the configured
/// local port.  One `tokio::task` is spawned per accepted connection.
async fn serve_mapping(mapping: PortMapping) -> Result<()> {
    // VMADDR_CID_ANY (u32::MAX) accepts connections from any CID, which
    // is correct for a guest-side listener: the host connects using the
    // guest's assigned CID.
    let mut listener = VsockListener::bind(libc::VMADDR_CID_ANY, mapping.vsock_port)
        .with_context(|| format!("listen on vsock port {}", mapping.vsock_port))?;

    eprintln!(
        "vsock-http-proxy: vsock port {} → http 127.0.0.1:{}",
        mapping.vsock_port, mapping.http_port
    );

    // A single Client per mapping reuses TCP connections to the local
    // HTTP service across requests.
    let client: Client<HttpConnector> = Client::new();

    loop {
        let (stream, _addr) = listener.accept().await.context("accept")?;
        let client = client.clone();
        let http_port = mapping.http_port;

        tokio::spawn(async move {
            let svc = service_fn(move |req| proxy_request(req, http_port, client.clone()));
            if let Err(e) = hyper::server::conn::Http::new()
                .serve_connection(stream, svc)
                .await
            {
                eprintln!("vsock-http-proxy: connection error: {e:#}");
            }
        });
    }
}

/// Forward a single HTTP request to `127.0.0.1:http_port`, preserving the
/// method, path, headers, and body.  Returns a 502 on upstream failure so
/// the caller always gets a well-formed HTTP response.
async fn proxy_request(
    req: Request<Body>,
    http_port: u16,
    client: Client<HttpConnector>,
) -> Result<Response<Body>, Infallible> {
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    let target = format!("http://127.0.0.1:{http_port}{path_and_query}");
    let uri = Uri::from_str(&target).expect("constructed URI is always valid");

    let mut builder = Request::builder().method(req.method()).uri(uri);
    if let Some(hdrs) = builder.headers_mut() {
        hdrs.clone_from(req.headers());
    }
    let proxied = builder
        .body(req.into_body())
        .expect("forwarded body is always valid");

    match client.request(proxied).await {
        Ok(resp) => Ok(resp),
        Err(e) => {
            eprintln!("vsock-http-proxy: upstream error: {e:#}");
            Ok(Response::builder()
                .status(502)
                .body(Body::from(format!("Bad Gateway: {e}")))
                .expect("502 response is always valid"))
        }
    }
}
