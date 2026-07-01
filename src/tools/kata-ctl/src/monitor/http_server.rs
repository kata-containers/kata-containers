// Copyright 2022-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::monitor::metrics::get_monitor_metrics;
use crate::sl;
use crate::utils::TIMEOUT;

use anyhow::{anyhow, Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Body;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder as AutoBuilder;
use shim_interface::shim_mgmt::client::MgmtClient;
use slog::{self, info};
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::net::TcpListener;

const ROOT_URI: &str = "/";
const METRICS_URI: &str = "/metrics";

async fn handler_mux<B>(req: Request<B>) -> Result<Response<Full<Bytes>>>
where
    B: Body + Send + 'static,
    B::Data: Send,
    B::Error: std::error::Error + Send + Sync,
{
    info!(
        sl!(),
        "mgmt-svr(mux): recv req, method: {}, uri: {}",
        req.method(),
        req.uri().path()
    );

    match (req.method(), req.uri().path()) {
        (&Method::GET, ROOT_URI) => root_uri_handler(req).await,
        (&Method::GET, METRICS_URI) => metrics_uri_handler(req).await,
        _ => not_found_uri_handler(req).await,
    }
    .map_or_else(
        |e| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::new(Bytes::from(format!("{e:?}\n"))))
                .map_err(|e| anyhow!("Failed to Build Response {:?}", e))
        },
        Ok,
    )
}

pub async fn http_server_setup(socket_addr: &str) -> Result<()> {
    let addr: SocketAddr = socket_addr
        .parse()
        .context("failed to parse http socket address")?;

    let listener = TcpListener::bind(addr)
        .await
        .context("failed to bind TCP listener")?;

    let builder = AutoBuilder::new(TokioExecutor::new());

    loop {
        let (stream, _) = listener
            .accept()
            .await
            .context("failed to accept connection")?;
        let io = TokioIo::new(stream);
        let builder = builder.clone();
        tokio::spawn(async move {
            if let Err(e) = builder.serve_connection(io, service_fn(handler_mux)).await {
                info!(sl!(), "http server connection error: {:?}", e);
            }
        });
    }
}

async fn root_uri_handler<B>(_req: Request<B>) -> Result<Response<Full<Bytes>>> {
    Response::builder()
        .status(StatusCode::OK)
        .body(Full::new(Bytes::from(
            r#"Available HTTP endpoints:
    /metrics : Get metrics from sandboxes.
"#,
        )))
        .map_err(|e| anyhow!("Failed to Build Response {:?}", e))
}

async fn metrics_uri_handler<B>(req: Request<B>) -> Result<Response<Full<Bytes>>> {
    let mut response_body = String::new();

    response_body += &get_monitor_metrics().context("Failed to Get Monitor Metrics")?;

    if let Some(uri_query) = req.uri().query() {
        if let Ok(sandbox_id) = parse_sandbox_id(uri_query) {
            response_body += &get_runtime_metrics(sandbox_id)
                .await
                .context(format!("{response_body}\nFailed to Get Runtime Metrics"))?;
        }
    }

    Response::builder()
        .status(StatusCode::OK)
        .body(Full::new(Bytes::from(response_body)))
        .map_err(|e| anyhow!("Failed to Build Response {:?}", e))
}

async fn get_runtime_metrics(sandbox_id: &str) -> Result<String> {
    // build shim client
    let shim_client =
        MgmtClient::new(sandbox_id, Some(TIMEOUT)).context("failed to build shim mgmt client")?;

    // get METRICS_URI
    let shim_response = shim_client
        .get(METRICS_URI)
        .await
        .context("failed to get METRICS_URI")?;

    // get runtime_metrics
    let runtime_metrics = String::from_utf8(
        shim_response
            .into_body()
            .collect()
            .await?
            .to_bytes()
            .to_vec(),
    )
    .context("failed to get runtime_metrics")?;

    Ok(runtime_metrics)
}

async fn not_found_uri_handler<B>(_req: Request<B>) -> Result<Response<Full<Bytes>>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Full::new(Bytes::from("NOT FOUND")))
        .map_err(|e| anyhow!("Failed to Build Response {:?}", e))
}

fn parse_sandbox_id(uri: &str) -> Result<&str> {
    let uri_pairs: HashMap<_, _> = uri
        .split_whitespace()
        .map(|s| s.split_at(s.find('=').unwrap_or(0)))
        .map(|(key, val)| (key, &val[1..]))
        .collect();

    match uri_pairs.get("sandbox") {
        Some(sid) => Ok(sid.to_owned()),
        None => Err(anyhow!("params sandbox not found")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sandbox_id() {
        assert!(parse_sandbox_id("sandbox=demo_sandbox").unwrap() == "demo_sandbox");
        assert!(parse_sandbox_id("foo=bar").is_err());
    }

    #[tokio::test]
    async fn test_root_uri_handler() {
        let root_resp = handler_mux(
            Request::builder()
                .method("GET")
                .uri("/")
                .body(Full::new(Bytes::new()))
                .unwrap(),
        )
        .await
        .unwrap();

        assert!(root_resp.status() == StatusCode::OK);
    }

    #[tokio::test]
    async fn test_metrics_uri_handler() {
        let metrics_resp = handler_mux(
            Request::builder()
                .method("GET")
                .uri("/metrics?sandbox=demo_sandbox")
                .body(Full::new(Bytes::new()))
                .unwrap(),
        )
        .await
        .unwrap();

        assert!(metrics_resp.status() == StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_not_found_uri_handler() {
        let not_found_resp = handler_mux(
            Request::builder()
                .method("POST")
                .uri("/metrics?sandbox=demo_sandbox")
                .body(Full::new(Bytes::new()))
                .unwrap(),
        )
        .await
        .unwrap();

        assert!(not_found_resp.status() == StatusCode::NOT_FOUND);
    }
}
