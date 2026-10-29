// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use dan_core::{BackendError, BackendState};
use serde::Deserialize;
use serde_json::json;

use crate::BACKEND_NAME;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub(crate) struct HostTapState {
    pub tap_name: String,
    pub owner_id: String,
    pub created_ovs: bool,
}

impl From<HostTapState> for BackendState {
    fn from(state: HostTapState) -> Self {
        Self {
            backend: BACKEND_NAME.to_string(),
            data: json!({
                "tap_name": state.tap_name,
                "owner_id": state.owner_id,
                "created_ovs": state.created_ovs,
            }),
        }
    }
}

impl TryFrom<&BackendState> for HostTapState {
    type Error = BackendError;

    fn try_from(state: &BackendState) -> Result<Self, Self::Error> {
        if state.backend != BACKEND_NAME {
            return Err(BackendError::InvalidInput(format!(
                "backend state belongs to `{}`, expected `{BACKEND_NAME}`",
                state.backend
            )));
        }
        let decoded = serde_json::from_value::<Self>(state.data.clone()).map_err(|error| {
            BackendError::InvalidInput(format!("invalid host-tap backend state: {error}"))
        })?;
        Ok(decoded)
    }
}
