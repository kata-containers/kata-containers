// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use cni_protocol::{CniError, CniErrorCode};
use serde_json::{Map, Value};

pub(crate) const POD_ANNOTATIONS_CAPABILITY: &str = "io.kubernetes.cri.pod-annotations";
pub(crate) const DAN_REQUEST_ANNOTATION: &str = "networking.katacontainers.io/dan";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RequestPath {
    Delegate,
    Dan,
}

/// Select the attachment path from CRI-provided Pod annotations.
/// DAN requires an explicit `"true"`; malformed values are errors.
pub(crate) fn select_request_path(
    runtime_config: &Map<String, Value>,
) -> Result<RequestPath, CniError> {
    let Some(raw_annotations) = runtime_config.get(POD_ANNOTATIONS_CAPABILITY) else {
        return Ok(RequestPath::Delegate);
    };
    let annotations = raw_annotations.as_object().ok_or_else(|| {
        invalid_selection(format!(
            "runtimeConfig.{POD_ANNOTATIONS_CAPABILITY} must be an object of string annotations"
        ))
    })?;

    match annotations.get(DAN_REQUEST_ANNOTATION) {
        None => Ok(RequestPath::Delegate),
        Some(Value::String(value)) if value == "true" => Ok(RequestPath::Dan),
        Some(Value::String(value)) if value == "false" => Ok(RequestPath::Delegate),
        Some(Value::String(value)) => Err(invalid_selection(format!(
            "annotation {DAN_REQUEST_ANNOTATION} must be \"true\" or \"false\", got {value:?}"
        ))),
        Some(_) => Err(invalid_selection(format!(
            "annotation {DAN_REQUEST_ANNOTATION} must be a string"
        ))),
    }
}

fn invalid_selection(details: impl Into<String>) -> CniError {
    CniError::new(
        CniErrorCode::InvalidNetworkConfig,
        "invalid DAN request selection",
        details,
    )
}
