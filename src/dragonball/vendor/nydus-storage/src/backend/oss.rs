// Copyright 2020 Ant Group. All rights reserved.
// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! Storage backend driver to access blobs on Oss(Object Storage System).
use std::io::{Error, Result};
use std::sync::Arc;
use std::time::SystemTime;

use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, CONTENT_LENGTH};
use reqwest::Method;
use sha1::Sha1;

use nydus_api::http::OssConfig;
use nydus_utils::metrics::BackendMetrics;

use crate::backend::connection::{Connection, ConnectionConfig, ConnectionError};
use crate::backend::{BackendError, BackendResult, BlobBackend, BlobReader};

const HEADER_DATE: &str = "Date";
const HEADER_AUTHORIZATION: &str = "Authorization";

type HmacSha1 = Hmac<Sha1>;

/// Error codes related to OSS storage backend.
#[derive(Debug)]
pub enum OssError {
    Auth(Error),
    Url(String),
    Request(ConnectionError),
    ConstructHeader(String),
    Transport(reqwest::Error),
    Response(String),
}

impl From<OssError> for BackendError {
    fn from(error: OssError) -> Self {
        BackendError::Oss(error)
    }
}

// `OssState` is almost identical to `OssConfig`, but let's keep them separated.
#[derive(Debug)]
struct OssState {
    access_key_id: String,
    access_key_secret: String,
    scheme: String,
    object_prefix: String,
    endpoint: String,
    bucket_name: String,
    retry_limit: u8,
}

impl OssState {
    fn resource(&self, object_key: &str, query_str: &str) -> String {
        format!("/{}/{}{}", self.bucket_name, object_key, query_str)
    }

    fn url(&self, object_key: &str, query: &[&str]) -> (String, String) {
        let object_key = &format!("{}{}", self.object_prefix, object_key);
        let url = format!(
            "{}://{}.{}/{}",
            self.scheme, self.bucket_name, self.endpoint, object_key
        );

        if query.is_empty() {
            (self.resource(object_key, ""), url)
        } else {
            let query_str = format!("?{}", query.join("&"));
            let resource = self.resource(object_key, &query_str);
            let url = format!("{}{}", url.as_str(), &query_str);
            (resource, url)
        }
    }

    /// generate oss request signature
    fn sign(
        &self,
        verb: Method,
        headers: &mut HeaderMap,
        canonicalized_resource: &str,
    ) -> Result<()> {
        let content_md5 = "";
        let content_type = "";
        let mut canonicalized_oss_headers = vec![];
        let date = httpdate::fmt_http_date(SystemTime::now());
        let mut data = vec![
            verb.as_str(),
            content_md5,
            content_type,
            date.as_str(),
            // canonicalized_oss_headers,
            canonicalized_resource,
        ];

        for (name, value) in headers.iter() {
            let name = name.as_str();
            let value = value.to_str().map_err(|e| einval!(e))?;
            if name.starts_with("x-oss-") {
                let header = format!("{}:{}", name.to_lowercase(), value);
                canonicalized_oss_headers.push(header);
            }
        }
        let canonicalized_oss_headers = canonicalized_oss_headers.join("\n");
        if !canonicalized_oss_headers.is_empty() {
            data.insert(4, canonicalized_oss_headers.as_str());
        }
        let data = data.join("\n");
        let mut mac =
            HmacSha1::new_from_slice(self.access_key_secret.as_bytes()).map_err(|e| einval!(e))?;
        mac.update(data.as_bytes());
        let signature = base64::encode(&mac.finalize().into_bytes());

        let authorization = format!("OSS {}:{}", self.access_key_id, signature);

        headers.insert(HEADER_DATE, date.as_str().parse().map_err(|e| einval!(e))?);
        headers.insert(
            HEADER_AUTHORIZATION,
            authorization.as_str().parse().map_err(|e| einval!(e))?,
        );

        Ok(())
    }
}

struct OssReader {
    blob_id: String,
    connection: Arc<Connection>,
    state: Arc<OssState>,
    metrics: Arc<BackendMetrics>,
}

impl BlobReader for OssReader {
    fn blob_size(&self) -> BackendResult<u64> {
        let (resource, url) = self.state.url(&self.blob_id, &[]);
        let mut headers = HeaderMap::new();

        self.state
            .sign(Method::HEAD, &mut headers, resource.as_str())
            .map_err(OssError::Auth)?;

        let resp = self
            .connection
            .call::<&[u8]>(
                Method::HEAD,
                url.as_str(),
                None,
                None,
                &mut headers,
                true,
                false,
            )
            .map_err(OssError::Request)?;
        let content_length = resp
            .headers()
            .get(CONTENT_LENGTH)
            .ok_or_else(|| OssError::Response("invalid content length".to_string()))?;

        Ok(content_length
            .to_str()
            .map_err(|err| OssError::Response(format!("invalid content length: {:?}", err)))?
            .parse::<u64>()
            .map_err(|err| OssError::Response(format!("invalid content length: {:?}", err)))?)
    }

    fn try_read(&self, mut buf: &mut [u8], offset: u64) -> BackendResult<usize> {
        let query = &[];
        let (resource, url) = self.state.url(&self.blob_id, query);
        let mut headers = HeaderMap::new();
        let end_at = offset + buf.len() as u64 - 1;
        let range = format!("bytes={}-{}", offset, end_at);

        headers.insert(
            "Range",
            range
                .as_str()
                .parse()
                .map_err(|e| OssError::ConstructHeader(format!("{}", e)))?,
        );
        self.state
            .sign(Method::GET, &mut headers, resource.as_str())
            .map_err(OssError::Auth)?;

        // Safe because the the call() is a synchronous operation.
        let mut resp = self
            .connection
            .call::<&[u8]>(
                Method::GET,
                url.as_str(),
                None,
                None,
                &mut headers,
                true,
                false,
            )
            .map_err(OssError::Request)?;
        Ok(resp
            .copy_to(&mut buf)
            .map_err(OssError::Transport)
            .map(|size| size as usize)?)
    }

    fn metrics(&self) -> &BackendMetrics {
        &self.metrics
    }

    fn retry_limit(&self) -> u8 {
        self.state.retry_limit
    }
}

/// Storage backend to access data stored in OSS.
#[derive(Debug)]
pub struct Oss {
    connection: Arc<Connection>,
    state: Arc<OssState>,
    metrics: Option<Arc<BackendMetrics>>,
    #[allow(unused)]
    id: Option<String>,
}

impl Oss {
    /// Create a new OSS storage backend.
    pub fn new(config: serde_json::value::Value, id: Option<&str>) -> Result<Oss> {
        let oss_config: OssConfig = serde_json::from_value(config).map_err(|e| einval!(e))?;
        let con_config: ConnectionConfig = oss_config.clone().into();
        let retry_limit = con_config.retry_limit;
        let connection = Connection::new(&con_config)?;
        let state = Arc::new(OssState {
            scheme: oss_config.scheme,
            object_prefix: oss_config.object_prefix,
            endpoint: oss_config.endpoint,
            access_key_id: oss_config.access_key_id,
            access_key_secret: oss_config.access_key_secret,
            bucket_name: oss_config.bucket_name,
            retry_limit,
        });
        let metrics = id.map(|i| BackendMetrics::new(i, "oss"));

        Ok(Oss {
            state,
            connection,
            metrics,
            id: id.map(|i| i.to_string()),
        })
    }
}

impl BlobBackend for Oss {
    fn shutdown(&self) {
        self.connection.shutdown();
    }

    fn metrics(&self) -> &BackendMetrics {
        // `metrics()` is only used for nydusd, which will always provide valid `blob_id`, thus
        // `self.metrics` has valid value.
        self.metrics.as_ref().unwrap()
    }

    fn get_reader(&self, blob_id: &str) -> BackendResult<Arc<dyn BlobReader>> {
        if let Some(metrics) = self.metrics.as_ref() {
            Ok(Arc::new(OssReader {
                blob_id: blob_id.to_string(),
                state: self.state.clone(),
                connection: self.connection.clone(),
                metrics: metrics.clone(),
            }))
        } else {
            Err(BackendError::Unsupported(
                "no metrics object available for OssReader".to_string(),
            ))
        }
    }
}

impl Drop for Oss {
    fn drop(&mut self) {
        if let Some(metrics) = self.metrics.as_ref() {
            metrics.release().unwrap_or_else(|e| error!("{:?}", e));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn test_oss_state() {
        let state = OssState {
            access_key_id: "key".to_string(),
            access_key_secret: "secret".to_string(),
            scheme: "https".to_string(),
            object_prefix: "nydus".to_string(),
            endpoint: "oss".to_string(),
            bucket_name: "images".to_string(),
            retry_limit: 5,
        };

        assert_eq!(
            state.resource("obj_key", "?idontcare"),
            "/images/obj_key?idontcare"
        );

        let (resource, url) = state.url("obj_key", &["idontcare", "second"]);
        assert_eq!(resource, "/images/nydusobj_key?idontcare&second");
        assert_eq!(url, "https://images.oss/nydusobj_key?idontcare&second");

        let mut headers = HeaderMap::new();
        state
            .sign(Method::HEAD, &mut headers, resource.as_str())
            .unwrap();
        let signature = headers.get(HEADER_AUTHORIZATION).unwrap();
        assert!(signature.to_str().unwrap().contains("OSS key:"));
    }

    #[test]
    fn test_oss_new() {
        let json_str = "{\"access_key_id\":\"key\",\"access_key_secret\":\"secret\",\"bucket_name\":\"images\",\"endpoint\":\"/oss\",\"object_prefix\":\"nydus\",\"scheme\":\"\",\"proxy\":{\"url\":\"\",\"ping_url\":\"\",\"fallback\":true,\"check_interval\":5},\"timeout\":5,\"connect_timeout\":5,\"retry_limit\":5}";
        let json: Value = serde_json::from_str(json_str).unwrap();
        let oss = Oss::new(json, Some("test-image")).unwrap();

        oss.metrics();

        let reader = oss.get_reader("test").unwrap();
        assert_eq!(reader.retry_limit(), 5);

        oss.shutdown();
    }
}
