//! # Initdata Module
//!
//! This module will do the following things if [`INITDATA_DEV`] exists.
//! 1. Parse the initdata block device and extract the config files to [`INITDATA_PATH`].
//! 2. Store the initdata hash in [`INITDATA`].

// Copyright (c) 2024 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::Path;

use anyhow::{bail, Context, Result};
use base64::Engine;
use const_format::concatcp;
use nix::mount::MsFlags;
use serde::Deserialize;
use sha2::{Digest, Sha256, Sha384, Sha512};
use slog::Logger;
use tokio::sync::OnceCell;

/// This is the target directory to store the extracted initdata.
pub const INITDATA_PATH: &str = "/run/confidential-containers/initdata";

/// This is the mount point of the initdata block device.
const INITDATA_TEMPPATH: &str = "/tmp/initdata";

/// The path of AA's config file
pub const AA_CONFIG_PATH: &str = concatcp!(INITDATA_PATH, "/aa.toml");

/// The path of CDH's config file
pub const CDH_CONFIG_PATH: &str = concatcp!(INITDATA_PATH, "/cdh.toml");

/// The path of policy file
pub const POLICY_PATH: &str = concatcp!(INITDATA_PATH, "/policy.rego");

const INITDATA_DEV: &str = "/dev/initdata";

const FILE_SYSTEM: &str = "ext4";

/// Now only initdata `0.1.0` is defined.
const DEFAULT_INITDATA_VERSION: &str = "0.1.0";

/// If initdata is given and parsed successfully, this static
/// variable will be set with the base64 encoded digest of the
/// initdata.
pub static INITDATA: OnceCell<String> = OnceCell::const_new();

/// Initdata defined in
/// <https://github.com/confidential-containers/trustee/blob/47d7a2338e0be76308ac19be5c0c172c592780aa/kbs/docs/initdata.md>
#[derive(Deserialize)]
pub struct Initdata {
    version: String,
    algorithm: String,
    data: DefinedFields,
}

/// Well-defined keys for initdata of kata/CoCo
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DefinedFields {
    #[serde(rename = "aa.toml")]
    aa_config: Option<String>,
    #[serde(rename = "cdh.toml")]
    cdh_config: Option<String>,
    #[serde(rename = "policy.rego")]
    policy: Option<String>,
}

pub async fn initialize_initdata(logger: &Logger) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "initdata"));
    if !Path::new(INITDATA_DEV).exists() {
        info!(
            logger,
            "Initdata device not found, skip initdata initialization"
        );
        return Ok(());
    }

    tokio::fs::create_dir_all(INITDATA_TEMPPATH)
        .await
        .inspect_err(|e| error!(logger, "Failed to create tmp initdata dir: {e:?}"))?;

    tokio::fs::create_dir_all(INITDATA_PATH)
        .await
        .inspect_err(|e| error!(logger, "Failed to create initdata dir: {e:?}"))?;

    nix::mount::mount::<_, _, _, str>(
        Some(INITDATA_DEV),
        INITDATA_TEMPPATH,
        Some(FILE_SYSTEM),
        MsFlags::empty(),
        None,
    )
    .context("mount initdata device failed")?;

    let initdata_content = tokio::fs::read(format!("{INITDATA_TEMPPATH}/initdata.toml"))
        .await
        .context("read initdata file failed")?;

    let initdata: Initdata =
        toml::from_slice(&initdata_content).context("parse initdata failed")?;
    info!(logger, "Initdata version: {}", initdata.version);

    if initdata.version != DEFAULT_INITDATA_VERSION {
        bail!("Unsupported initdata version");
    }

    let digest = match &initdata.algorithm[..] {
        "sha256" => {
            let mut hasher = Sha256::new();
            hasher.update(&initdata_content);
            hasher.finalize().to_vec()
        }
        "sha384" => {
            let mut hasher = Sha384::new();
            hasher.update(&initdata_content);
            hasher.finalize().to_vec()
        }
        "sha512" => {
            let mut hasher = Sha512::new();
            hasher.update(&initdata_content);
            hasher.finalize().to_vec()
        }
        others => bail!("Unsupported hash algorithm {others}"),
    };

    let initdata_digest = base64::engine::general_purpose::STANDARD.encode(digest);
    INITDATA
        .set(initdata_digest)
        .context("Failed to set INITDATA")?;

    if let Some(config) = initdata.data.aa_config {
        tokio::fs::write(AA_CONFIG_PATH, config)
            .await
            .context("write aa config failed")?;
        info!(logger, "write AA config from initdata");
    }

    if let Some(config) = initdata.data.cdh_config {
        tokio::fs::write(CDH_CONFIG_PATH, config)
            .await
            .context("write cdh config failed")?;
        info!(logger, "write CDH config from initdata");
    }

    if let Some(policy) = initdata.data.policy {
        tokio::fs::write(POLICY_PATH, policy)
            .await
            .context("write policy failed")?;
        info!(logger, "write policy from initdata");
    }

    Ok(())
}
