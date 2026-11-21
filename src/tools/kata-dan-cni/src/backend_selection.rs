// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use backend_host_tap::HostTapBackend;
use dan_core::{Backend, BackendError};
use serde_json::Value;

type BackendConstructor = fn(&Value) -> Result<Box<dyn Backend>, BackendError>;
struct Registration {
    name: &'static str,
    create: BackendConstructor,
}

const BACKENDS: &[Registration] = &[Registration {
    name: "host-tap",
    create: |config| Ok(Box::new(HostTapBackend::from_config(config)?)),
}];

pub(crate) fn create_backend(config: &Value) -> Result<Box<dyn Backend>, BackendError> {
    let name = config["type"]
        .as_str()
        .ok_or_else(|| BackendError::InvalidInput("backend.type is required".into()))?;
    let registration = BACKENDS
        .iter()
        .find(|backend| backend.name == name)
        .ok_or_else(|| BackendError::InvalidInput(format!("unknown backend: {name}")))?;
    (registration.create)(config)
}
