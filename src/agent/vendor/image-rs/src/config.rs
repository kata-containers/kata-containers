// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use serde::{Deserialize, Deserializer};
use std::convert::TryFrom;
use std::fs::File;
use std::path::{Path, PathBuf};

use crate::snapshots::SnapshotType;
use crate::CC_IMAGE_WORK_DIR;

const DEFAULT_WORK_DIR: &str = "/var/lib/image-rs/";

/// Default policy file path.
pub const POLICY_FILE_PATH: &str = "/run/image-security/security_policy.json";

/// Dir of Sigstore Config file.
/// The reason for using the `/run` directory here is that in general HW-TEE,
/// the `/run` directory is mounted in `tmpfs`, which is located in the encrypted memory protected by HW-TEE.
pub const SIG_STORE_CONFIG_DIR: &str = "/run/image-security/simple_signing/sigstore_config";

pub const SIG_STORE_CONFIG_DEFAULT_FILE: &str =
    "/run/image-security/simple_signing/sigstore_config/default.yaml";

/// Path to the gpg pubkey ring of the signature
pub const GPG_KEY_RING: &str = "/run/image-security/simple_signing/pubkey.gpg";

/// Dir for storage of cosign verification keys.
pub const COSIGN_KEY_DIR: &str = "/run/image-security/cosign";

/// The reason for using the `/run` directory here is that in general HW-TEE,
/// the `/run` directory is mounted in `tmpfs`, which is located in the encrypted memory protected by HW-TEE.
/// [`AUTH_FILE_PATH`] shows the path to the `auth.json` file.
pub const AUTH_FILE_PATH: &str = "/run/image-security/auth.json";

/// `image-rs` configuration information.
#[derive(Clone, Debug, Deserialize)]
pub struct ImageConfig {
    /// The location for `image-rs` to store data.
    pub work_dir: PathBuf,

    /// The default snapshot for `image-rs` to use.
    pub default_snapshot: SnapshotType,

    /// Security validation control
    pub security_validate: bool,

    /// Use `auth.json` control
    pub auth: bool,

    /// Records different configurable paths
    #[serde(
        default = "Paths::default",
        deserialize_with = "deserialize_null_default"
    )]
    pub file_paths: Paths,
}

/// This function used to parse from string. When it is an
/// empty string, return the default value of the parsed
/// struct.
fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    T: Default + Deserialize<'de>,
    D: Deserializer<'de>,
{
    let opt = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

impl Default for ImageConfig {
    // Construct a default instance of `ImageConfig`
    fn default() -> ImageConfig {
        let work_dir = PathBuf::from(
            std::env::var(CC_IMAGE_WORK_DIR).unwrap_or_else(|_| DEFAULT_WORK_DIR.to_string()),
        );

        ImageConfig {
            work_dir,
            #[cfg(feature = "snapshot-overlayfs")]
            default_snapshot: SnapshotType::Overlay,
            #[cfg(not(feature = "snapshot-overlayfs"))]
            default_snapshot: SnapshotType::Unknown,
            security_validate: false,
            auth: false,
            file_paths: Paths::default(),
        }
    }
}

impl TryFrom<&Path> for ImageConfig {
    /// Load `ImageConfig` from a configuration file like:
    ///    {
    ///        "work_dir": "/var/lib/image-rs/",
    ///        "default_snapshot": "overlay"
    ///    }
    type Error = anyhow::Error;
    fn try_from(config_path: &Path) -> Result<Self, Self::Error> {
        let file = File::open(config_path)
            .map_err(|e| anyhow!("failed to open config file {}", e.to_string()))?;

        serde_json::from_reader::<File, ImageConfig>(file)
            .map_err(|e| anyhow!("failed to parse config file {}", e.to_string()))
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Paths {
    /// Path to `Policy.json`
    pub policy_path: String,

    /// Dir of `Sigstore Config file`, used by simple signing
    pub sig_store_config_dir: String,

    /// Default sigstore config file, used by simple signing
    pub default_sig_store_config_file: String,

    /// Path to the gpg pubkey ring of the signature
    pub gpg_key_ring: String,

    /// Dir for storage of cosign verification keys
    pub cosign_key_dir: String,

    /// Path to the auth file
    pub auth_file: String,
}

impl Default for Paths {
    fn default() -> Self {
        Self {
            policy_path: POLICY_FILE_PATH.into(),
            sig_store_config_dir: SIG_STORE_CONFIG_DIR.into(),
            default_sig_store_config_file: SIG_STORE_CONFIG_DEFAULT_FILE.into(),
            gpg_key_ring: GPG_KEY_RING.into(),
            cosign_key_dir: COSIGN_KEY_DIR.into(),
            auth_file: AUTH_FILE_PATH.into(),
        }
    }
}

#[cfg(feature = "snapshot-overlayfs")]
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::prelude::*;
    use tempfile;

    #[test]
    fn test_image_config() {
        let config = ImageConfig::default();
        let work_dir = PathBuf::from(DEFAULT_WORK_DIR);

        std::env::remove_var(CC_IMAGE_WORK_DIR);
        assert_eq!(config.work_dir, work_dir);
        assert_eq!(config.default_snapshot, SnapshotType::Overlay);

        let env_work_dir = "/tmp";
        std::env::set_var(CC_IMAGE_WORK_DIR, env_work_dir);
        let config = ImageConfig::default();
        let work_dir = PathBuf::from(env_work_dir);
        assert_eq!(config.work_dir, work_dir);
    }

    #[test]
    fn test_image_config_from_file() {
        let data = r#"{
            "work_dir": "/var/lib/image-rs/",
            "default_snapshot": "overlay",
            "security_validate": false,
            "auth": false
        }"#;

        let tempdir = tempfile::tempdir().unwrap();
        let config_file = tempdir.path().join("config.json");

        File::create(&config_file)
            .unwrap()
            .write_all(data.as_bytes())
            .unwrap();

        let config = ImageConfig::try_from(config_file.as_path()).unwrap();
        let work_dir = PathBuf::from(DEFAULT_WORK_DIR);

        assert_eq!(config.work_dir, work_dir);
        assert_eq!(config.default_snapshot, SnapshotType::Overlay);

        let invalid_config_file = tempdir.path().join("does-not-exist");
        assert!(!invalid_config_file.exists());

        let _ = ImageConfig::try_from(invalid_config_file.as_path()).is_err();
        assert!(!invalid_config_file.exists());
    }
}
