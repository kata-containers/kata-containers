// Copyright 2020 Ant Group. All rights reserved.
// Copyright © 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

//! Nydus API v1.

use dbs_uhttp::{Method, Request, Response};

use crate::http::{
    error_response, extract_query_part, parse_body, success_response, translate_status_code,
    ApiError, ApiRequest, ApiResponse, ApiResponsePayload, EndpointHandler, HttpError, HttpResult,
};

/// HTTP URI prefix for API v1.
pub const HTTP_ROOT_V1: &str = "/api/v1";

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
                DaemonInfo(d) => success_response(Some(d)),
                FsGlobalMetrics(d) => success_response(Some(d)),
                FsFilesMetrics(d) => success_response(Some(d)),
                FsFilesPatterns(d) => success_response(Some(d)),
                FsBackendInfo(d) => success_response(Some(d)),
                FsInflightMetrics(d) => success_response(Some(d)),
                _ => panic!("Unexpected response message from API service"),
            }
        }
        Err(e) => {
            let status_code = translate_status_code(&e);
            error_response(op(e), status_code)
        }
    }
}

/// Get daemon information and set daemon configuration.
pub struct InfoHandler {}
impl EndpointHandler for InfoHandler {
    fn handle_request(
        &self,
        req: &Request,
        kicker: &dyn Fn(ApiRequest) -> ApiResponse,
    ) -> HttpResult {
        match (req.method(), req.body.as_ref()) {
            (Method::Get, None) => {
                let r = kicker(ApiRequest::GetDaemonInfo);
                Ok(convert_to_response(r, HttpError::DaemonInfo))
            }
            (Method::Put, Some(body)) => {
                let conf = parse_body(body)?;
                let r = kicker(ApiRequest::ConfigureDaemon(conf));
                Ok(convert_to_response(r, HttpError::Configure))
            }
            _ => Err(HttpError::BadRequest),
        }
    }
}

/// Get filesystem backend information.
pub struct FsBackendInfo {}
impl EndpointHandler for FsBackendInfo {
    fn handle_request(
        &self,
        req: &Request,
        kicker: &dyn Fn(ApiRequest) -> ApiResponse,
    ) -> HttpResult {
        match (req.method(), req.body.as_ref()) {
            (Method::Get, None) => {
                let mountpoint = extract_query_part(req, "mountpoint").ok_or_else(|| {
                    HttpError::QueryString(
                        "'mountpoint' should be specified in query string".to_string(),
                    )
                })?;
                let r = kicker(ApiRequest::ExportFsBackendInfo(mountpoint));
                Ok(convert_to_response(r, HttpError::FsBackendInfo))
            }
            _ => Err(HttpError::BadRequest),
        }
    }
}

/// Get filesystem global metrics.
pub struct MetricsFsGlobalHandler {}
impl EndpointHandler for MetricsFsGlobalHandler {
    fn handle_request(
        &self,
        req: &Request,
        kicker: &dyn Fn(ApiRequest) -> ApiResponse,
    ) -> HttpResult {
        match (req.method(), req.body.as_ref()) {
            (Method::Get, None) => {
                let id = extract_query_part(req, "id");
                let r = kicker(ApiRequest::ExportFsGlobalMetrics(id));
                Ok(convert_to_response(r, HttpError::GlobalMetrics))
            }
            _ => Err(HttpError::BadRequest),
        }
    }
}

/// Get filesystem access pattern log.
pub struct MetricsFsAccessPatternHandler {}
impl EndpointHandler for MetricsFsAccessPatternHandler {
    fn handle_request(
        &self,
        req: &Request,
        kicker: &dyn Fn(ApiRequest) -> ApiResponse,
    ) -> HttpResult {
        match (req.method(), req.body.as_ref()) {
            (Method::Get, None) => {
                let id = extract_query_part(req, "id");
                let r = kicker(ApiRequest::ExportFsAccessPatterns(id));
                Ok(convert_to_response(r, HttpError::Pattern))
            }
            _ => Err(HttpError::BadRequest),
        }
    }
}

/// Get filesystem file metrics.
pub struct MetricsFsFilesHandler {}
impl EndpointHandler for MetricsFsFilesHandler {
    fn handle_request(
        &self,
        req: &Request,
        kicker: &dyn Fn(ApiRequest) -> ApiResponse,
    ) -> HttpResult {
        match (req.method(), req.body.as_ref()) {
            (Method::Get, None) => {
                let id = extract_query_part(req, "id");
                let latest_read_files = extract_query_part(req, "latest")
                    .map_or(false, |b| b.parse::<bool>().unwrap_or(false));
                let r = kicker(ApiRequest::ExportFsFilesMetrics(id, latest_read_files));
                Ok(convert_to_response(r, HttpError::FsFilesMetrics))
            }
            _ => Err(HttpError::BadRequest),
        }
    }
}

/// Get information about filesystem inflight requests.
pub struct MetricsFsInflightHandler {}
impl EndpointHandler for MetricsFsInflightHandler {
    fn handle_request(
        &self,
        req: &Request,
        kicker: &dyn Fn(ApiRequest) -> ApiResponse,
    ) -> HttpResult {
        match (req.method(), req.body.as_ref()) {
            (Method::Get, None) => {
                let r = kicker(ApiRequest::ExportFsInflightMetrics);
                Ok(convert_to_response(r, HttpError::InflightMetrics))
            }
            _ => Err(HttpError::BadRequest),
        }
    }
}
