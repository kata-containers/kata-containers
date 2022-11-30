// Copyright 2021-2022 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::metrics::get_metrics;
use anyhow::{anyhow, Result};
use hyper::body;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use runtimes::MgmtClient;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

const ROOT_URI: &str = "/";
const METRICS_URI: &str = "/metrics";

async fn handler_mux(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    info!(
        sl!(),
        "mgmt-svr(mux): recv req, method: {}, uri: {}",
        req.method(),
        req.uri().path()
    );

    match (req.method(), req.uri().path()) {
        (&Method::GET, ROOT_URI) => root_uri_handler(req).await,
        (&Method::GET, METRICS_URI) => metrics_uri_handler(req).await,
        _ => Ok(not_found_uri_handler(req).await),
    }
}

#[tokio::main]
pub async fn http_server(socket_addr: &str) -> Result<()> {
    let addr: SocketAddr = socket_addr.parse()?;

    let make_svc =
        make_service_fn(|_conn| async { Ok::<_, hyper::Error>(service_fn(handler_mux)) });

    let server = Server::bind(&addr).serve(make_svc);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }

    Ok(())
}

async fn root_uri_handler(_req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(
            r#"Available HTTP endpoints:
    /metrics : Get metrics from sandboxes.
"#,
        ))
        .unwrap())
}

async fn metrics_uri_handler(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let mut response_body = String::new();

    if let Ok(monitor_metrics) = get_metrics().await {
        response_body += &monitor_metrics;
    } else {
        return Ok(Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("failed to get monitor metrics"))
            .unwrap());
    }

    if let Some(uri_query) = req.uri().query() {
        if let Ok(sandbox_id) = parse_sandbox_id(uri_query) {
            let mut runtime_metrics_flag = false;
            let runtime_metrics = get_runtime_metrics(sandbox_id).await;
            if runtime_metrics.is_ok() {
                let runtime_metrics = runtime_metrics.unwrap();
                if !runtime_metrics.is_empty() {
                    response_body += &runtime_metrics;
                    runtime_metrics_flag = true;
                }
            }
            if !runtime_metrics_flag {
                return Ok(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("failed to get runtime metrics"))
                    .unwrap());
            }
        }
    }

    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(response_body))
        .unwrap())
}

async fn get_runtime_metrics(sandbox_id: &str) -> Result<String> {
    // build shim client
    let shim_client = MgmtClient::new(sandbox_id.to_string(), Some(Duration::new(5, 0)))?;

    // get METRICS_URI
    let shim_response = shim_client.get(METRICS_URI).await?;

    // get runtime_metrics
    let runtime_metrics = String::from_utf8(body::to_bytes(shim_response).await?.to_vec())?;

    Ok(runtime_metrics)
}

async fn not_found_uri_handler(_req: Request<Body>) -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("NOT FOUND"))
        .unwrap()
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
