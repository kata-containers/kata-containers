// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use crate::{CniError, CniErrorCode, SUPPORTED_CNI_VERSIONS};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VersionRequest {
    cni_version: String,
}

pub fn version_response(input: &[u8]) -> Result<Value, CniError> {
    let request: VersionRequest = serde_json::from_slice(input).map_err(|source| {
        CniError::new(
            CniErrorCode::DecodeFailure,
            "failed to decode VERSION request",
            source.to_string(),
        )
    })?;
    if request.cni_version.is_empty() {
        return Err(CniError::new(
            CniErrorCode::DecodeFailure,
            "failed to decode VERSION request",
            "cniVersion must not be empty",
        ));
    }
    Ok(serde_json::json!({
        "cniVersion": request.cni_version,
        "supportedVersions": SUPPORTED_CNI_VERSIONS,
    }))
}
