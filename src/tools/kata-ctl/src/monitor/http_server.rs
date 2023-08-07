// Copyright 2022-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::monitor::metrics::get_monitor_metrics;
use crate::sl;
use crate::utils::TIMEOUT;

use anyhow::{anyhow, Context, Result};
use hyper::body;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use shim_interface::shim_mgmt::client::MgmtClient;
use slog::{self, info};
use std::collections::HashMap;
use std::net::SocketAddr;

const ROOT_URI: &str = "/";
const METRICS_URI: &str = "/metrics";

async fn handler_mux(req: Request<Body>) -> Result<Response<Body>> {
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
                .body(Body::from(format!("{:?}\n", e)))
                .map_err(|e| anyhow!("Failed to Build Response {:?}", e))
        },
        Ok,
    )
}

pub async fn http_server_setup(socket_addr: &str) -> Result<()> {
    let addr: SocketAddr = socket_addr
        .parse()
        .context("failed to parse http socket address")?;

    let make_svc =
        make_service_fn(|_conn| async { Ok::<_, anyhow::Error>(service_fn(handler_mux)) });

    Server::bind(&addr).serve(make_svc).await?;

    Ok(())
}

async fn root_uri_handler(_req: Request<Body>) -> Result<Response<Body>> {
    Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(
            r#"Available HTTP endpoints:
    /metrics : Get metrics from sandboxes.
"#,
        ))
        .map_err(|e| anyhow!("Failed to Build Response {:?}", e))
}

async fn metrics_uri_handler(req: Request<Body>) -> Result<Response<Body>> {
    let mut response_body = String::new();

    response_body += &get_monitor_metrics().context("Failed to Get Monitor Metrics")?;

    if let Some(uri_query) = req.uri().query() {
        if let Ok(sandbox_id) = parse_sandbox_id(uri_query) {
            response_body += &get_runtime_metrics(sandbox_id)
                .await
                .context(format!("{}\nFailed to Get Runtime Metrics", response_body))?;
        }
    }

    Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(response_body))
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
    let runtime_metrics = String::from_utf8(body::to_bytes(shim_response).await?.to_vec())
        .context("failed to get runtime_metrics")?;

    Ok(runtime_metrics)
}

async fn not_found_uri_handler(_req: Request<Body>) -> Result<Response<Body>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("NOT FOUND"))
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
                .body(hyper::Body::from(""))
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
                .body(hyper::Body::from(""))
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
                .body(hyper::Body::from(""))
                .unwrap(),
        )
        .await
        .unwrap();

        assert!(not_found_resp.status() == StatusCode::NOT_FOUND);
    }
}
