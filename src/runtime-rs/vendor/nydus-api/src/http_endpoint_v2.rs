// Copyright 2022 Alibaba Cloud. All rights reserved.
// Copyright 2020 Ant Group. All rights reserved.
// Copyright © 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

//! Nydus API v2.

use dbs_uhttp::{Method, Request, Response};

use crate::http::{
    error_response, extract_query_part, parse_body, success_response, translate_status_code,
    ApiError, ApiRequest, ApiResponse, ApiResponsePayload, BlobCacheObjectId, EndpointHandler,
    HttpError, HttpResult,
};

/// HTTP URI prefix for API v2.
pub const HTTP_ROOT_V2: &str = "/api/v2";

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
                BlobObjectList(d) => success_response(Some(d)),
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
pub struct InfoV2Handler {}
impl EndpointHandler for InfoV2Handler {
    fn handle_request(
        &self,
        req: &Request,
        kicker: &dyn Fn(ApiRequest) -> ApiResponse,
    ) -> HttpResult {
        match (req.method(), req.body.as_ref()) {
            (Method::Get, None) => {
                let r = kicker(ApiRequest::GetDaemonInfoV2);
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

/// List blob objects managed by the blob cache manager.
pub struct BlobObjectListHandlerV2 {}
impl EndpointHandler for BlobObjectListHandlerV2 {
    fn handle_request(
        &self,
        req: &Request,
        kicker: &dyn Fn(ApiRequest) -> ApiResponse,
    ) -> HttpResult {
        match (req.method(), req.body.as_ref()) {
            (Method::Get, None) => {
                if let Some(domain_id) = extract_query_part(req, "domain_id") {
                    let blob_id = extract_query_part(req, "blob_id").unwrap_or_default();
                    let param = BlobCacheObjectId { domain_id, blob_id };
                    let r = kicker(ApiRequest::GetBlobObject(param));
                    return Ok(convert_to_response(r, HttpError::GetBlobObjects));
                }
                Err(HttpError::BadRequest)
            }
            (Method::Put, Some(body)) => {
                let conf = parse_body(body)?;
                let r = kicker(ApiRequest::CreateBlobObject(conf));
                Ok(convert_to_response(r, HttpError::CreateBlobObject))
            }
            (Method::Delete, None) => {
                if let Some(domain_id) = extract_query_part(req, "domain_id") {
                    let blob_id = extract_query_part(req, "blob_id").unwrap_or_default();
                    let param = BlobCacheObjectId { domain_id, blob_id };
                    let r = kicker(ApiRequest::DeleteBlobObject(param));
                    return Ok(convert_to_response(r, HttpError::DeleteBlobObject));
                }
                Err(HttpError::BadRequest)
            }
            _ => Err(HttpError::BadRequest),
        }
    }
}
