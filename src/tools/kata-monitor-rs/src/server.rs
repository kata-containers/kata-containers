// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{anyhow, Result};
use bytes::Bytes;
use flate2::write::GzEncoder;
use flate2::Compression;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::header::HeaderValue;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tracing::{error, info, warn};

use crate::config::RuntimeConfig;
use crate::collectos::{filter_metrics_by_family, MetricsCollector};
use crate::cache::SandboxCache;
use crate::metrics::MonitorMetrics;
use crate::client::ShimClient;

type BoxBody = Full<Bytes>;

fn full_body(s: impl Into<Bytes>) -> BoxBody {
    Full::new(s.into())
}

pub struct AppState {
    pub sandbox_cache: Arc<SandboxCache>,
    pub metrics_collector: Arc<MetricsCollector>,
    pub monitor_metrics: Arc<MonitorMetrics>,
    pub runtime_config: RuntimeConfig,
}

pub async fn start_server(
    addr: &str,
    state: Arc<AppState>,
    shutdown: impl std::future::Future<Output = ()>,
) -> Result<()> {
    let socket_addr: SocketAddr = addr
        .parse()
        .map_err(|e| anyhow!("invalid address: {}", e))?;

    let listener = TcpListener::bind(socket_addr).await?;
    info!(addr = %socket_addr, "kata-monitor listening");

    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            _ = &mut shutdown => {
                info!("http server shut down gracefully");
                return Ok(());
            }
            result = listener.accept() => {
                let (stream, _) = result?;
                let io = TokioIo::new(stream);
                let state = state.clone();

                tokio::spawn(async move {
                    let svc = service_fn(move |req| {
                        let state = state.clone();
                        async move { handler_mux(req, state).await }
                    });
                    if let Err(e) = http1::Builder::new().serve_connection(io, svc).await {
                        warn!(error = %e, "connection error");
                    }
                });
            }
        }
    }
}

async fn handler_mux(
    req: Request<Incoming>,
    state: Arc<AppState>,
) -> Result<Response<BoxBody>, hyper::Error> {
    let path = req.uri().path().to_string();

    let result = match (req.method(), path.as_str()) {
        (&Method::GET, "/") => index_handler(&req).await,
        (&Method::GET, "/healthz") => healthz_handler().await,
        (&Method::GET, "/readyz") => healthz_handler().await,
        (&Method::GET, "/metrics") => metrics_handler(&req, &state).await,
        (&Method::GET, "/sandboxes") => sandboxes_handler(&req, &state).await,
        (&Method::GET, "/agent-url") => agent_url_handler(&req, &state).await,
        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(full_body("Not Found\n"))
            .unwrap()),
    };

    match result {
        Ok(resp) => Ok(resp),
        Err(e) => Ok(Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(full_body(format!("{e:?}\n")))
            .unwrap()),
    }
}

const ENDPOINTS: &[(&str, &str)] = &[
    ("/healthz", "Health check endpoint."),
    ("/readyz", "Readiness check endpoint."),
    ("/metrics", "Get metrics from sandboxes."),
    ("/sandboxes", "List all Kata Containers sandboxes."),
    ("/agent-url", "Get sandbox agent URL."),
];

async fn index_handler(req: &Request<Incoming>) -> Result<Response<BoxBody>> {
    if accepts_html(req) {
        let mut body = String::from("<h1>Available HTTP endpoints:</h1>\n<ul>\n");
        for (path, desc) in ENDPOINTS {
            if *path == "/metrics" || *path == "/sandboxes" {
                body.push_str(&format!(
                    "<li><b><a href='{path}'>{path}</a></b>: {desc}</li>\n"
                ));
            } else {
                body.push_str(&format!("<li><b>{path}</b>: {desc}</li>\n"));
            }
        }
        body.push_str("</ul>\n");

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/html")
            .body(full_body(body))?)
    } else {
        let mut body = String::from("Available HTTP endpoints:\n");
        let max_len = ENDPOINTS.iter().map(|(p, _)| p.len()).max().unwrap_or(0);
        for (path, desc) in ENDPOINTS {
            body.push_str(&format!(
                "{:width$} : {}\n",
                path,
                desc,
                width = max_len + 3
            ));
        }

        Ok(Response::builder()
            .status(StatusCode::OK)
            .body(full_body(body))?)
    }
}

async fn healthz_handler() -> Result<Response<BoxBody>> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(full_body("ok\n"))?)
}

async fn metrics_handler(
    req: &Request<Incoming>,
    state: &Arc<AppState>,
) -> Result<Response<BoxBody>> {
    let start = Instant::now();
    state.monitor_metrics.scrape_count.inc();

    let params = parse_query(req.uri().query().unwrap_or(""));

    if let Some(sandbox_id) = params.get("sandbox") && !sandbox_id.is_empty() {
        match state.metrics_collector.collect_single(sandbox_id).await {
            Ok(metrics) => {
                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .body(full_body(metrics))?);
            }
            Err(e) => {
                return Ok(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(full_body(e.to_string()))?);
            }
        }
    }

    let mut response_body = String::new();

    let filter_families: Vec<&str> = params
        .get("filter_family")
        .map(|f| f.split(',').collect())
        .unwrap_or_default();

    if filter_families.is_empty() {
        state
            .monitor_metrics
            .running_shim_count
            .set(state.sandbox_cache.count().await as f64);
        if let Ok(self_metrics) = state.monitor_metrics.encode() {
            response_body.push_str(&self_metrics);
        }
    }

    match state.metrics_collector.collect_all().await {
        Ok(sandbox_metrics) => {
            if filter_families.is_empty() {
                response_body.push_str(&sandbox_metrics);
            } else {
                let filtered = filter_metrics_by_family(&sandbox_metrics, &filter_families);
                response_body.push_str(&filtered);
            }
        }
        Err(e) => {
            error!(error = %e, "failed to aggregate sandbox metrics");
            state.monitor_metrics.scrape_failed_count.inc();
        }
    }

    let duration_ms = start.elapsed().as_millis() as f64;
    state
        .monitor_metrics
        .scrape_duration_ms
        .observe(duration_ms);

    if accepts_gzip(req) {
        let compressed = gzip_compress(response_body.as_bytes());
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/plain; charset=utf-8")
            .header("Content-Encoding", "gzip")
            .body(full_body(compressed))?)
    } else {
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(full_body(response_body))?)
    }
}

async fn sandboxes_handler(
    req: &Request<Incoming>,
    state: &Arc<AppState>,
) -> Result<Response<BoxBody>> {
    let sandboxes = state.sandbox_cache.get_all().await;

    if accepts_html(req) {
        let mut body = String::from("<h1>Sandbox list</h1>\n<ul>\n");
        for s in &sandboxes {
            body.push_str(&format!(
                "<li>{s}: <a href='/metrics?sandbox={s}'>metrics</a>, \
                 <a href='/agent-url?sandbox={s}'>agent-url</a></li>\n"
            ));
        }
        body.push_str("</ul>\n");

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/html")
            .body(full_body(body))?)
    } else {
        let body: String = sandboxes.iter().map(|s| format!("{s}\n")).collect();
        Ok(Response::builder()
            .status(StatusCode::OK)
            .body(full_body(body))?)
    }
}

async fn agent_url_handler(
    req: &Request<Incoming>,
    state: &Arc<AppState>,
) -> Result<Response<BoxBody>> {
    let params = parse_query(req.uri().query().unwrap_or(""));
    let sandbox_id = params
        .get("sandbox")
        .ok_or_else(|| anyhow!("sandbox parameter required"))?;

    let socket_path = state.runtime_config.socket_path(sandbox_id);
    let client = ShimClient::new(socket_path, std::time::Duration::from_secs(3));

    match client.get("/agent-url").await {
        Ok(data) => Ok(Response::builder()
            .status(StatusCode::OK)
            .body(full_body(data))?),
        Err(e) => Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(full_body(format!("{e}\n")))?),
    }
}

fn parse_query(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter(|s| !s.is_empty())
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next().unwrap_or("");
            Some((key.to_string(), value.to_string()))
        })
        .collect()
}

fn accepts_html(req: &Request<Incoming>) -> bool {
    req.headers()
        .get_all("accept")
        .iter()
        .any(|v: &HeaderValue| v.to_str().unwrap_or("").contains("text/html"))
}

fn accepts_gzip(req: &Request<Incoming>) -> bool {
    req.headers()
        .get("accept-encoding")
        .and_then(|v: &HeaderValue| v.to_str().ok())
        .unwrap_or("")
        .contains("gzip")
}

fn gzip_compress(data: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}
