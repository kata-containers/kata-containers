// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

// This defines the handlers corresponding to the url when a request is sent to destined url,
// the handler function should be invoked, and the corresponding data will be in the response

use crate::shim_metrics::get_shim_metrics;
use agent::ResizeVolumeRequest;
use anyhow::{anyhow, Context, Result};
use common::Sandbox;
use hyper::{Body, Method, Request, Response, StatusCode};
use std::sync::Arc;
use url::Url;

use shim_interface::shim_mgmt::{
    AGENT_URL, DIRECT_VOLUME_PATH_KEY, DIRECT_VOLUME_RESIZE_URL, DIRECT_VOLUME_STATS_URL,
    IP6_TABLE_URL, IP_TABLE_URL, METRICS_URL,
};

// main router for response, this works as a multiplexer on
// http arrival which invokes the corresponding handler function
pub(crate) async fn handler_mux(
    sandbox: Arc<dyn Sandbox>,
    req: Request<Body>,
) -> Result<Response<Body>> {
    info!(
        sl!(),
        "mgmt-svr(mux): recv req, method: {}, uri: {}",
        req.method(),
        req.uri().path()
    );
    match (req.method(), req.uri().path()) {
        (&Method::GET, AGENT_URL) => agent_url_handler(sandbox, req).await,
        (&Method::PUT, IP_TABLE_URL) | (&Method::GET, IP_TABLE_URL) => {
            ip_table_handler(sandbox, req).await
        }
        (&Method::PUT, IP6_TABLE_URL) | (&Method::GET, IP6_TABLE_URL) => {
            ipv6_table_handler(sandbox, req).await
        }
        (&Method::POST, DIRECT_VOLUME_STATS_URL) => direct_volume_stats_handler(sandbox, req).await,
        (&Method::POST, DIRECT_VOLUME_RESIZE_URL) => {
            direct_volume_resize_handler(sandbox, req).await
        }
        (&Method::GET, METRICS_URL) => metrics_url_handler(sandbox, req).await,
        _ => Ok(not_found(req).await),
    }
}

// url not found
async fn not_found(_req: Request<Body>) -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("URL NOT FOUND"))
        .unwrap()
}

// returns the url for agent
async fn agent_url_handler(
    sandbox: Arc<dyn Sandbox>,
    _req: Request<Body>,
) -> Result<Response<Body>> {
    let agent_sock = sandbox
        .agent_sock()
        .await
        .unwrap_or_else(|_| String::from(""));
    Ok(Response::new(Body::from(agent_sock)))
}

/// the ipv4 handler of iptable operation
async fn ip_table_handler(sandbox: Arc<dyn Sandbox>, req: Request<Body>) -> Result<Response<Body>> {
    generic_ip_table_handler(sandbox, req, false).await
}

/// the ipv6 handler of iptable operation
async fn ipv6_table_handler(
    sandbox: Arc<dyn Sandbox>,
    req: Request<Body>,
) -> Result<Response<Body>> {
    generic_ip_table_handler(sandbox, req, true).await
}

/// the generic iptable handler, for both ipv4 and ipv6
/// this requires iptables-series binaries to be inside guest rootfs
async fn generic_ip_table_handler(
    sandbox: Arc<dyn Sandbox>,
    req: Request<Body>,
    is_ipv6: bool,
) -> Result<Response<Body>> {
    info!(sl!(), "handler: iptable  ipv6?: {}", is_ipv6);
    match *req.method() {
        Method::GET => match sandbox.get_iptables(is_ipv6).await {
            Ok(data) => {
                let body = Body::from(data);
                Response::builder().body(body).map_err(|e| anyhow!(e))
            }
            _ => Err(anyhow!("Failed to get iptable")),
        },

        Method::PUT => {
            let data = hyper::body::to_bytes(req.into_body()).await?;
            match sandbox.set_iptables(is_ipv6, data.to_vec()).await {
                Ok(resp_data) => Response::builder()
                    .body(Body::from(resp_data))
                    .map_err(|e| anyhow!(e)),
                _ => Err(anyhow!("Failed to set iptable")),
            }
        }

        _ => Err(anyhow!("IP Tables only takes PUT and GET")),
    }
}

async fn direct_volume_stats_handler(
    sandbox: Arc<dyn Sandbox>,
    req: Request<Body>,
) -> Result<Response<Body>> {
    let params = Url::parse(&req.uri().to_string())
        .map_err(|e| anyhow!(e))?
        .query_pairs()
        .into_owned()
        .collect::<std::collections::HashMap<String, String>>();
    let volume_path = params
        .get(DIRECT_VOLUME_PATH_KEY)
        .context("shim-mgmt: volume path key not found in request params")?;
    let result = sandbox.direct_volume_stats(volume_path).await;
    match result {
        Ok(stats) => Ok(Response::new(Body::from(stats))),
        _ => Err(anyhow!("handler: Failed to get volume stats")),
    }
}

async fn direct_volume_resize_handler(
    sandbox: Arc<dyn Sandbox>,
    req: Request<Body>,
) -> Result<Response<Body>> {
    let body = hyper::body::to_bytes(req.into_body()).await?;

    // unserialize json body into resizeRequest struct
    let resize_req: ResizeVolumeRequest =
        serde_json::from_slice(&body).context("shim-mgmt: deserialize resizeRequest failed")?;
    let result = sandbox.direct_volume_resize(resize_req).await;

    match result {
        Ok(_) => Ok(Response::new(Body::from(""))),
        _ => Err(anyhow!("handler: Failed to resize volume")),
    }
}

// returns the url for metrics
async fn metrics_url_handler(
    sandbox: Arc<dyn Sandbox>,
    _req: Request<Body>,
) -> Result<Response<Body>> {
    // get metrics from agent, hypervisor, and shim
    let agent_metrics = sandbox.agent_metrics().await.unwrap_or_default();
    let hypervisor_metrics = sandbox.hypervisor_metrics().await.unwrap_or_default();
    let shim_metrics = get_shim_metrics().unwrap_or_default();

    Ok(Response::new(Body::from(format!(
        "{}{}{}",
        agent_metrics, hypervisor_metrics, shim_metrics
    ))))
}
