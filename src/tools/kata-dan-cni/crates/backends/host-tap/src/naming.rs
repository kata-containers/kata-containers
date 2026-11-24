// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use dan_core::{AttachmentIdentity, BackendError, NetworkContext};
use sha2::{Digest, Sha256};

pub(crate) const TAP_NAME_PREFIX: &str = "kdt";
pub(crate) const TAP_NAME_HASH_LENGTH: usize = 12;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResourceIdentity {
    pub tap_name: String,
    pub owner_id: String,
}

impl TryFrom<&NetworkContext> for ResourceIdentity {
    type Error = BackendError;

    fn try_from(context: &NetworkContext) -> Result<Self, Self::Error> {
        Self::try_from(&AttachmentIdentity {
            container_id: context.container_id.clone(),
            if_name: context.selection.if_name.clone(),
        })
    }
}

impl TryFrom<&AttachmentIdentity> for ResourceIdentity {
    type Error = BackendError;

    fn try_from(identity: &AttachmentIdentity) -> Result<Self, Self::Error> {
        let fields = [
            ("container ID", identity.container_id.as_str()),
            ("interface name", identity.if_name.as_str()),
        ];
        let mut hasher = Sha256::new();

        for (field_name, field_value) in fields {
            if field_value.is_empty() {
                return Err(BackendError::InvalidInput(format!("{field_name} is empty")));
            }
            let field_length = field_value.len() as u64;
            hasher.update(field_length.to_be_bytes());
            hasher.update(field_value.as_bytes());
        }

        let owner_id = format!("{:x}", hasher.finalize());
        let tap_name = format!("{TAP_NAME_PREFIX}{}", &owner_id[..TAP_NAME_HASH_LENGTH]);

        Ok(Self { tap_name, owner_id })
    }
}
