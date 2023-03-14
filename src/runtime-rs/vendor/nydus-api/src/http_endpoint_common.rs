// Copyright 2020 Ant Group. All rights reserved.
// Copyright © 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::http::{
    error_response, extract_query_part, parse_body, success_response, translate_status_code,
    ApiError, ApiRequest, ApiResponse, ApiResponsePayload, EndpointHandler, HttpError, HttpResult,
};
use dbs_uhttp::{Method, Request, Response};

// Convert an ApiResponse to a HTTP response.
//
// API server has successfully processed the request, but can't fulfill that. Therefore,
// a `error_response` is generated whose status code is 4XX or 5XX. With error response,
// it still returns Ok(error_response) to http request handling framework, which means
// nydusd api server receives the request and try handle it, even the request can't be fulfilled.
fn convert_to_response<O: FnOnce(ApiError) -> HttpError>(api_resp: ApiResponse, op: O) -> Response {
    match api_resp {
        Ok(r) => {
            use ApiResponsePayload::*;
            match r {
                Empty => success_response(None),
                Events(d) => success_response(Some(d)),
                BackendMetrics(d) => success_response(Some(d)),
                BlobcacheMetrics(d) => success_response(Some(d)),
                _ => panic!("Unexpected response message from API service"),
            }
        }
        Err(e) => {
            let status_code = translate_status_code(&e);
            error_response(op(e), status_code)
        }
    }
}
// Global daemon control requests.
/// Start the daemon.
pub struct StartHandler {}
impl EndpointHandler for StartHandler {
    fn handle_request(
        &self,
        req: &Request,
        kicker: &dyn Fn(ApiRequest) -> ApiResponse,
    ) -> HttpResult {
        match (req.method(), req.body.as_ref()) {
            (Method::Put, None) => {
                let r = kicker(ApiRequest::Start);
                Ok(convert_to_response(r, HttpError::Configure))
            }
            _ => Err(HttpError::BadRequest),
        }
    }
}

/// Stop the daemon.
pub struct ExitHandler {}
impl EndpointHandler for ExitHandler {
    fn handle_request(
        &self,
        req: &Request,
        kicker: &dyn Fn(ApiRequest) -> ApiResponse,
    ) -> HttpResult {
        match (req.method(), req.body.as_ref()) {
            (Method::Put, None) => {
                let r = kicker(ApiRequest::Exit);
                Ok(convert_to_response(r, HttpError::Configure))
            }
            _ => Err(HttpError::BadRequest),
        }
    }
}

/// Get daemon global events.
pub struct EventsHandler {}
impl EndpointHandler for EventsHandler {
    fn handle_request(
        &self,
        req: &Request,
        kicker: &dyn Fn(ApiRequest) -> ApiResponse,
    ) -> HttpResult {
        match (req.method(), req.body.as_ref()) {
            (Method::Get, None) => {
                let r = kicker(ApiRequest::GetEvents);
                Ok(convert_to_response(r, HttpError::Events))
            }
            _ => Err(HttpError::BadRequest),
        }
    }
}

// Metrics related requests.
/// Get storage backend metrics.
pub struct MetricsBackendHandler {}
impl EndpointHandler for MetricsBackendHandler {
    fn handle_request(
        &self,
        req: &Request,
        kicker: &dyn Fn(ApiRequest) -> ApiResponse,
    ) -> HttpResult {
        match (req.method(), req.body.as_ref()) {
            (Method::Get, None) => {
                let id = extract_query_part(req, "id");
                let r = kicker(ApiRequest::ExportBackendMetrics(id));
                Ok(convert_to_response(r, HttpError::BackendMetrics))
            }
            _ => Err(HttpError::BadRequest),
        }
    }
}

/// Get blob cache metrics.
pub struct MetricsBlobcacheHandler {}
impl EndpointHandler for MetricsBlobcacheHandler {
    fn handle_request(
        &self,
        req: &Request,
        kicker: &dyn Fn(ApiRequest) -> ApiResponse,
    ) -> HttpResult {
        match (req.method(), req.body.as_ref()) {
            (Method::Get, None) => {
                let id = extract_query_part(req, "id");
                let r = kicker(ApiRequest::ExportBlobcacheMetrics(id));
                Ok(convert_to_response(r, HttpError::BlobcacheMetrics))
            }
            _ => Err(HttpError::BadRequest),
        }
    }
}

/// Mount a filesystem.
pub struct MountHandler {}
impl EndpointHandler for MountHandler {
    fn handle_request(
        &self,
        req: &Request,
        kicker: &dyn Fn(ApiRequest) -> ApiResponse,
    ) -> HttpResult {
        let mountpoint = extract_query_part(req, "mountpoint").ok_or_else(|| {
            HttpError::QueryString("'mountpoint' should be specified in query string".to_string())
        })?;
        match (req.method(), req.body.as_ref()) {
            (Method::Post, Some(body)) => {
                let cmd = parse_body(body)?;
                let r = kicker(ApiRequest::Mount(mountpoint, cmd));
                Ok(convert_to_response(r, HttpError::Mount))
            }
            (Method::Put, Some(body)) => {
                let cmd = parse_body(body)?;
                let r = kicker(ApiRequest::Remount(mountpoint, cmd));
                Ok(convert_to_response(r, HttpError::Mount))
            }
            (Method::Delete, None) => {
                let r = kicker(ApiRequest::Umount(mountpoint));
                Ok(convert_to_response(r, HttpError::Mount))
            }
            _ => Err(HttpError::BadRequest),
        }
    }
}

/// Send fuse fd to new daemon.
pub struct SendFuseFdHandler {}
impl EndpointHandler for SendFuseFdHandler {
    fn handle_request(
        &self,
        req: &Request,
        kicker: &dyn Fn(ApiRequest) -> ApiResponse,
    ) -> HttpResult {
        match (req.method(), req.body.as_ref()) {
            (Method::Put, None) => {
                let r = kicker(ApiRequest::SendFuseFd);
                Ok(convert_to_response(r, HttpError::Upgrade))
            }
            _ => Err(HttpError::BadRequest),
        }
    }
}

/// Take over fuse fd from old daemon instance.
pub struct TakeoverFuseFdHandler {}
impl EndpointHandler for TakeoverFuseFdHandler {
    fn handle_request(
        &self,
        req: &Request,
        kicker: &dyn Fn(ApiRequest) -> ApiResponse,
    ) -> HttpResult {
        match (req.method(), req.body.as_ref()) {
            (Method::Put, None) => {
                let r = kicker(ApiRequest::TakeoverFuseFd);
                Ok(convert_to_response(r, HttpError::Upgrade))
            }
            _ => Err(HttpError::BadRequest),
        }
    }
}
