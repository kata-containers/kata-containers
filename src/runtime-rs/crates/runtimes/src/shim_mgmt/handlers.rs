// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

// This defines the handlers corresponding to the url when a request is sent to destined url,
// the handler function should be invoked, and the corresponding data will be in the response

use common::Sandbox;
use hyper::{Body, Method, Request, Response, Result, StatusCode};
use std::sync::Arc;

use super::server::AGENT_URL;

// main router for response, this works as a multiplexer on
// http arrival which invokes the corresponding handler function
pub(crate) async fn handler_mux(
    sandbox: Arc<dyn Sandbox>,
    req: Request<Body>,
) -> Result<Response<Body>> {
    info!(sl!(), "mgmt-svr(mux): recv req {:?}", req);
    match (req.method(), req.uri().path()) {
        (&Method::GET, AGENT_URL) => agent_url_handler(sandbox, req).await,
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
