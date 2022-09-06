// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

// This defines the handler corresponding to the url
// when a request is sent to destined url, the handler
// function should be invoked, and the corresponding
// data will be in the response's body
//
// NOTE: ALL HANDLER SHOULD BE ASYNC UNDER ROUTERIFY

use hyper::{Body, Method, Request, Response, Result, StatusCode};

use super::server::AGENT_URL;

// main router for response, this works as a multiplexer on
// http arrival which invokes the corresponding handler function
pub(crate) async fn handler_mux(req: Request<Body>) -> Result<Response<Body>> {
    info!(sl!(), "mgmt-svr(mux): recv req {:?}", req);
    match (req.method(), req.uri().path()) {
        (&Method::GET, AGENT_URL) => agent_url_handler(req).await,
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
async fn agent_url_handler(_req: Request<Body>) -> Result<Response<Body>> {
    // todo
    Ok(Response::new(Body::from("")))
}
