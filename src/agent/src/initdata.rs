//! # Initdata Module
//!
//! This module will do the following things if a proper initdata device with initdata exists.
//! 1. Parse the initdata block device and extract the config files to [`INITDATA_PATH`].
//! 2. Return the initdata and the policy (if any).

// Copyright (c) 2025 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{os::unix::fs::FileTypeExt, path::Path};

use anyhow::{bail, Context, Result};
use async_compression::tokio::bufread::GzipDecoder;
use base64::{engine::general_purpose::STANDARD, Engine};
use const_format::concatcp;
use serde::Deserialize;
use sha2::{Digest, Sha256, Sha384, Sha512};
use slog::Logger;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// This is the target directory to store the extracted initdata.
pub const INITDATA_PATH: &str = "/run/confidential-containers/initdata";

/// The path of AA's config file
pub const AA_CONFIG_PATH: &str = concatcp!(INITDATA_PATH, "/aa.toml");

/// The path of CDH's config file
pub const CDH_CONFIG_PATH: &str = concatcp!(INITDATA_PATH, "/cdh.toml");

/// Magic number of initdata device
pub const INITDATA_MAGIC_NUMBER: &[u8] = b"initdata";

/// Now only initdata `0.1.0` is defined.
const INITDATA_VERSION: &str = "0.1.0";

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

async fn detect_initdata_device(logger: &Logger) -> Result<Option<String>> {
    let dev_dir = Path::new("/dev");
    let mut read_dir = tokio::fs::read_dir(dev_dir).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        let filename = entry.file_name();
        let filename = filename.to_string_lossy();
        debug!(logger, "Initdata check device `{filename}`");
        if !filename.starts_with("vd") {
            continue;
        }
        let path = entry.path();

        debug!(logger, "Initdata find potential device: `{path:?}`");
        let metadata = std::fs::metadata(path.clone())?;
        if !metadata.file_type().is_block_device() {
            continue;
        }

        let mut file = tokio::fs::File::open(&path).await?;
        let mut magic = [0; 8];
        match file.read_exact(&mut magic).await {
            Ok(_) => {
                debug!(
                    logger,
                    "Initdata read device `{filename}` first 8 bytes: {magic:?}"
                );
                if magic == INITDATA_MAGIC_NUMBER {
                    let path = path.as_path().to_string_lossy().to_string();
                    debug!(logger, "Found initdata device {path}");
                    return Ok(Some(path));
                }
            }
            Err(e) => debug!(logger, "Initdata read device `{filename}` failed: {e:?}"),
        }
    }

    Ok(None)
}

pub async fn read_initdata(device_path: &str) -> Result<Vec<u8>> {
    let initdata_devfile = tokio::fs::File::open(device_path).await?;
    let mut buf_reader = tokio::io::BufReader::new(initdata_devfile);
    // skip the magic number "initdata"
    buf_reader.seek(std::io::SeekFrom::Start(8)).await?;

    let mut len_buf = [0u8; 8];
    buf_reader.read_exact(&mut len_buf).await?;
    let length = u64::from_le_bytes(len_buf) as usize;

    let mut buf = vec![0; length];
    buf_reader.read_exact(&mut buf).await?;
    let mut gzip_decoder = GzipDecoder::new(&buf[..]);

    let mut initdata = Vec::new();
    let _ = gzip_decoder.read_to_end(&mut initdata).await?;
    Ok(initdata)
}

pub struct InitdataReturnValue {
    pub digest: Vec<u8>,
    pub _policy: Option<String>,
}

pub async fn initialize_initdata(logger: &Logger) -> Result<Option<InitdataReturnValue>> {
    let logger = logger.new(o!("subsystem" => "initdata"));
    let Some(initdata_device) = detect_initdata_device(&logger).await? else {
        info!(
            logger,
            "Initdata device not found, skip initdata initialization"
        );
        return Ok(None);
    };

    tokio::fs::create_dir_all(INITDATA_PATH)
        .await
        .inspect_err(|e| error!(logger, "Failed to create initdata dir: {e:?}"))?;

    let initdata_content = read_initdata(&initdata_device)
        .await
        .inspect_err(|e| error!(logger, "Failed to read initdata: {e:?}"))?;

    let initdata: Initdata =
        toml::from_slice(&initdata_content).context("parse initdata failed")?;
    info!(logger, "Initdata version: {}", initdata.version);

    if initdata.version != INITDATA_VERSION {
        bail!("Unsupported initdata version");
    }

    let digest = match &initdata.algorithm[..] {
        "sha256" => Sha256::digest(&initdata_content).to_vec(),
        "sha384" => Sha384::digest(&initdata_content).to_vec(),
        "sha512" => Sha512::digest(&initdata_content).to_vec(),
        others => bail!("Unsupported hash algorithm {others}"),
    };

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

    debug!(logger, "Initdata digest: {}", STANDARD.encode(&digest));

    let res = InitdataReturnValue {
        digest,
        _policy: initdata.data.policy,
    };

    Ok(Some(res))
}

#[cfg(test)]
mod tests {
    use crate::initdata::read_initdata;

    const INITDATA_IMG_PATH: &str = "testdata/initdata.img";
    const INITDATA_PLAINTEXT: &[u8] = b"some content";

    #[tokio::test]
    async fn parse_initdata() {
        let initdata = read_initdata(INITDATA_IMG_PATH).await.unwrap();
        assert_eq!(initdata, INITDATA_PLAINTEXT);
    }
}
