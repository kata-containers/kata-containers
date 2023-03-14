// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod auth_config;

use std::{collections::HashMap, fs::File, io::BufReader, path::Path};

use anyhow::*;
use oci_distribution::{secrets::RegistryAuth, Reference};
use serde::{Deserialize, Serialize};

/// Hard-coded ResourceDescription of `auth.json`.
pub const RESOURCE_DESCRIPTION: &str = "Credential";

#[derive(Deserialize, Serialize)]
pub struct DockerConfigFile {
    auths: HashMap<String, DockerAuthConfig>,
    // TODO: support credential helpers
}

#[derive(Deserialize, Serialize)]
pub struct DockerAuthConfig {
    auth: String,
}

/// Get a credential (RegistryAuth) for the given Reference.
/// First, it will try to find auth info in the local
/// `auth.json`. If there is not one, it will
/// ask one from the [`crate::secure_channel::SecureChannel`], which connects
/// to the GetResource API of Attestation Agent.
/// Then, it will use the `auth.json` to find
/// a credential of the given image reference.
#[cfg(feature = "getresource")]
pub async fn credential_for_reference(
    reference: &Reference,
    secure_channel: std::sync::Arc<tokio::sync::Mutex<crate::secure_channel::SecureChannel>>,
    auth_file_path: &str,
) -> Result<RegistryAuth> {
    // if Policy config file does not exist, get if from KBS.
    if !Path::new(auth_file_path).exists() {
        secure_channel
            .lock()
            .await
            .get_resource(RESOURCE_DESCRIPTION, HashMap::new(), auth_file_path)
            .await?;
    }

    let reader = File::open(auth_file_path)?;
    let buf_reader = BufReader::new(reader);
    let config: DockerConfigFile = serde_json::from_reader(buf_reader)?;

    // TODO: support credential helpers
    auth_config::credential_from_auth_config(reference, &config.auths)
}

/// Get a credential (RegistryAuth) for the given Reference.
/// First, it will try to find auth info in the local
/// `auth.json`. If there is not one, it will
/// directly return [`RegistryAuth::Anonymous`].
/// Or, it will use the `auth.json` to find
/// a credential of the given image reference.
pub async fn credential_for_reference_local(
    reference: &Reference,
    auth_file_path: &str,
) -> Result<RegistryAuth> {
    // if Policy config file does not exist, get if from KBS.
    if !Path::new(auth_file_path).exists() {
        return Ok(RegistryAuth::Anonymous);
    }

    let reader = File::open(auth_file_path)?;
    let buf_reader = BufReader::new(reader);
    let config: DockerConfigFile = serde_json::from_reader(buf_reader)?;

    // TODO: support credential helpers
    auth_config::credential_from_auth_config(reference, &config.auths)
}
