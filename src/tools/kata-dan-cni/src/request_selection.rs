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

/// Select the attachment path from runtime-provided Pod annotations.
/// A missing capability requires a Pod lookup; an empty annotation map does not.
pub(crate) fn select_request_path(
    runtime_config: &Map<String, Value>,
) -> Result<Option<RequestPath>, CniError> {
    let Some(raw_annotations) = runtime_config.get(POD_ANNOTATIONS_CAPABILITY) else {
        return Ok(None);
    };
    let annotations = raw_annotations.as_object().ok_or_else(|| {
        invalid_selection(format!(
            "runtimeConfig.{POD_ANNOTATIONS_CAPABILITY} must be an object of string annotations"
        ))
    })?;

    let annotation = annotations
        .get(DAN_REQUEST_ANNOTATION)
        .map(|value| {
            value.as_str().ok_or_else(|| {
                invalid_selection(format!(
                    "annotation {DAN_REQUEST_ANNOTATION} must be a string"
                ))
            })
        })
        .transpose()?;
    select_dan_annotation(annotation).map(Some)
}

/// Apply the same annotation rule to runtime input and API-fetched Pods.
pub(crate) fn select_dan_annotation(annotation: Option<&str>) -> Result<RequestPath, CniError> {
    match annotation {
        None | Some("false") => Ok(RequestPath::Delegate),
        Some("true") => Ok(RequestPath::Dan),
        Some(value) => Err(invalid_selection(format!(
            "annotation {DAN_REQUEST_ANNOTATION} must be \"true\" or \"false\", got {value:?}"
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
