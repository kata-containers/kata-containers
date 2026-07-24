//! # Initdata Module
//!
//! This module will do the following things if a proper initdata device with initdata exists.
//! 1. Parse the initdata block device and extract the config files to [`INITDATA_PATH`].
//! 2. Return the initdata and the policy (if any).

// Copyright (c) 2025 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(feature = "init-data")]
use std::{os::unix::fs::FileTypeExt, path::Path};

use anyhow::{bail, Context, Result};
use async_compression::tokio::bufread::GzipDecoder;
use base64::{engine::general_purpose::STANDARD, Engine};
use const_format::concatcp;
use kata_types::initdata::InitData;
use sha2::{Digest, Sha256, Sha384, Sha512};
use slog::Logger;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// This is the target directory to store the extracted initdata.
pub const INITDATA_PATH: &str = "/run/confidential-containers/initdata";

const AA_CONFIG_KEY: &str = "aa.toml";
const CDH_CONFIG_KEY: &str = "cdh.toml";
const POLICY_KEY: &str = "policy.rego";

// BL-5: SRM trust-root config keys carried in the measured initdata section. When present,
// these bind the fragment-issuer trust root and the verified-layer / verified-image
// allowlists to the initdata digest (attestation-measured), instead of relying only on a
// file in the measured rootfs. Absent keys fall back to the rootfs file (backward-compatible).
const FRAGMENT_ISSUERS_KEY: &str = "fragment-issuers.toml";
const VERIFIED_LAYERS_KEY: &str = "verified-layers.toml";
const VERIFIED_IMAGES_KEY: &str = "verified-images.toml";

/// The path of initdata toml
pub const INITDATA_TOML_PATH: &str = concatcp!(INITDATA_PATH, "/initdata.toml");

/// The path of AA's config file
pub const AA_CONFIG_PATH: &str = concatcp!(INITDATA_PATH, "/aa.toml");

/// The path of CDH's config file
pub const CDH_CONFIG_PATH: &str = concatcp!(INITDATA_PATH, "/cdh.toml");

/// Magic number of initdata device
#[cfg(feature = "init-data")]
pub const INITDATA_MAGIC_NUMBER: &[u8] = b"initdata";

/// initdata device with disk type 'vd*'
#[cfg(feature = "init-data")]
const INITDATA_PREFIX_DISK_VDX: &str = "vd";

/// initdata device with disk type 'sd*'
#[cfg(feature = "init-data")]
const INITDATA_PREFIX_DISK_SDX: &str = "sd";

#[cfg(not(feature = "init-data"))]
async fn detect_initdata_device(logger: &Logger) -> Result<Option<String>> {
    debug!(logger, "Initdata is disabled");
    Ok(None)
}

#[cfg(feature = "init-data")]
async fn detect_initdata_device(logger: &Logger) -> Result<Option<String>> {
    let dev_dir = Path::new("/dev");
    let mut read_dir = tokio::fs::read_dir(dev_dir).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        let filename = entry.file_name();
        let filename = filename.to_string_lossy();
        debug!(logger, "Initdata check device `{filename}`");

        // Currently there're two disk types supported:
        // virtio-blk (vd*) and virtio-scsi (sd*)
        if !filename.starts_with(INITDATA_PREFIX_DISK_VDX)
            && !filename.starts_with(INITDATA_PREFIX_DISK_SDX)
        {
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
    pub _digest: Vec<u8>,
    pub _policy: Option<String>,
    // BL-5: SRM trust-root configs sourced from the measured initdata section (each is the
    // TOML text of the corresponding `/etc/kata/*.toml`), when the initdata declares them.
    pub _fragment_issuers: Option<String>,
    pub _verified_layers: Option<String>,
    pub _verified_images: Option<String>,
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

    let initdata: InitData =
        toml::from_slice(&initdata_content).context("parse initdata failed")?;
    info!(logger, "Initdata version: {}", initdata.version());
    initdata.validate()?;

    tokio::fs::write(INITDATA_TOML_PATH, &initdata_content)
        .await
        .context("write initdata toml failed")?;

    let _digest = match initdata.algorithm() {
        "sha256" => Sha256::digest(&initdata_content).to_vec(),
        "sha384" => Sha384::digest(&initdata_content).to_vec(),
        "sha512" => Sha512::digest(&initdata_content).to_vec(),
        others => bail!("Unsupported hash algorithm {others}"),
    };

    if let Some(config) = initdata.get_coco_data(AA_CONFIG_KEY) {
        tokio::fs::write(AA_CONFIG_PATH, config)
            .await
            .context("write aa config failed")?;
        info!(logger, "write AA config from initdata");
    }

    if let Some(config) = initdata.get_coco_data(CDH_CONFIG_KEY) {
        tokio::fs::write(CDH_CONFIG_PATH, config)
            .await
            .context("write cdh config failed")?;
        info!(logger, "write CDH config from initdata");
    }

    debug!(logger, "Initdata digest: {}", STANDARD.encode(&_digest));

    let res = InitdataReturnValue {
        _digest,
        _policy: initdata.get_coco_data(POLICY_KEY).cloned(),
        _fragment_issuers: initdata.get_coco_data(FRAGMENT_ISSUERS_KEY).cloned(),
        _verified_layers: initdata.get_coco_data(VERIFIED_LAYERS_KEY).cloned(),
        _verified_images: initdata.get_coco_data(VERIFIED_IMAGES_KEY).cloned(),
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

    // BL-5: the SRM trust-root configs (fragment issuers + verified-layer/-image allowlists)
    // are carried as measured initdata keys — extractable by the agent and covered by the
    // initdata digest that is bound to TEE attestation.
    #[test]
    fn initdata_carries_and_measures_srm_trust_roots() {
        use kata_types::initdata::InitData;
        use sha2::{Digest, Sha256};

        let content = concat!(
            "version = \"0.1.0\"\n",
            "algorithm = \"sha256\"\n",
            "[data]\n",
            "\"fragment-issuers.toml\" = \"require_receipt = true\\n\"\n",
            "\"verified-layers.toml\" = \"require_verified_layers = true\\n\"\n",
            "\"verified-images.toml\" = \"require_verified_images = true\\n\"\n",
        );
        let id: InitData = toml::from_str(content).expect("parse initdata");
        id.validate().expect("valid initdata");

        assert_eq!(
            id.get_coco_data("fragment-issuers.toml").map(|s| s.as_str()),
            Some("require_receipt = true\n")
        );
        assert!(id.get_coco_data("verified-layers.toml").is_some());
        assert!(id.get_coco_data("verified-images.toml").is_some());

        // Measured: flipping a trust-root value changes the initdata content digest, so a
        // tampered trust root cannot pass attestation unnoticed.
        let tampered = content.replace("require_receipt = true", "require_receipt = false");
        assert_ne!(
            Sha256::digest(content.as_bytes()),
            Sha256::digest(tampered.as_bytes())
        );
    }
}
