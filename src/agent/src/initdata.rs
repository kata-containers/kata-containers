//! # Initdata Module
//!
//! This module will do the following things if a proper initdata device with initdata exists.
//! 1. Parse the initdata block device and extract the config files to [`INITDATA_PATH`].
//! 2. Return the initdata and the policy (if any).

// Copyright (c) 2025 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{collections::BTreeMap, sync::OnceLock};
#[cfg(feature = "init-data")]
use std::{os::unix::fs::FileTypeExt, path::Path};

use anyhow::{bail, Context, Result};
use async_compression::tokio::bufread::GzipDecoder;
use base64::{engine::general_purpose::STANDARD, Engine};
use const_format::concatcp;
use kata_types::initdata::InitData;
use serde::Deserialize;
use sha2::{Digest, Sha256, Sha384, Sha512};
use slog::Logger;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// This is the target directory to store the extracted initdata.
pub const INITDATA_PATH: &str = "/run/confidential-containers/initdata";

const AA_CONFIG_KEY: &str = "aa.toml";
const CDH_CONFIG_KEY: &str = "cdh.toml";
const POLICY_KEY: &str = "policy.rego";
pub(crate) const CONFIDENTIAL_STORAGE_CLAIM: &str = "confidential_storage";
pub(crate) const CONFIDENTIAL_STORAGE_PROFILE_LUKS2_INTEGRITY_EXT4: &str = "luks2-integrity-ext4";
const CONFIDENTIAL_STORAGE_REGISTRY_VERSION: u32 = 1;
const CONFIDENTIAL_STORAGE_MAX_VOLUMES: usize = 64;
const CONFIDENTIAL_STORAGE_VOLUME_ID_MAX_BYTES: usize = 256;
const CONFIDENTIAL_STORAGE_KEY_URI_MAX_BYTES: usize = 2048;

static CONFIDENTIAL_STORAGE_REGISTRY: OnceLock<BTreeMap<String, ConfidentialStorageClaim>> =
    OnceLock::new();

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ConfidentialStorageClaim {
    pub(crate) profile: String,
    pub(crate) volume_id: String,
    pub(crate) key_uri: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConfidentialStorageRegistryDocument {
    version: u32,
    volumes: Vec<ConfidentialStorageClaim>,
}

pub(crate) fn confidential_storage_claim(
    volume_id: &str,
) -> Option<&'static ConfidentialStorageClaim> {
    CONFIDENTIAL_STORAGE_REGISTRY
        .get()
        .and_then(|registry| registry.get(volume_id))
}

fn canonical_confidential_storage_volume_id(value: &str) -> bool {
    if value.is_empty() || value.len() > CONFIDENTIAL_STORAGE_VOLUME_ID_MAX_BYTES {
        return false;
    }
    if value
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return false;
    }
    value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/' | b'@')
    })
}

fn canonical_confidential_storage_key_uri(value: &str) -> bool {
    value.len() > "kbs:///".len()
        && value.len() <= CONFIDENTIAL_STORAGE_KEY_URI_MAX_BYTES
        && value.starts_with("kbs:///")
        && value.bytes().all(|byte| byte.is_ascii_graphic())
}

fn confidential_storage_registry_from_initdata(
    initdata: &InitData,
) -> Result<Option<BTreeMap<String, ConfidentialStorageClaim>>> {
    let Some(document) = initdata.get_coco_data(CONFIDENTIAL_STORAGE_CLAIM) else {
        return Ok(None);
    };

    let document: ConfidentialStorageRegistryDocument = serde_json::from_str(document)
        .context("parse confidential storage registry from measured init-data")?;
    if document.version != CONFIDENTIAL_STORAGE_REGISTRY_VERSION {
        bail!("unsupported confidential storage registry version");
    }
    if document.volumes.len() > CONFIDENTIAL_STORAGE_MAX_VOLUMES {
        bail!("confidential storage registry contains too many volumes");
    }

    let mut registry = BTreeMap::new();
    for claim in document.volumes {
        if claim.profile != CONFIDENTIAL_STORAGE_PROFILE_LUKS2_INTEGRITY_EXT4 {
            bail!("unsupported confidential storage profile in measured init-data");
        }
        if !canonical_confidential_storage_volume_id(&claim.volume_id) {
            bail!("invalid confidential storage volume ID in measured init-data");
        }
        if !canonical_confidential_storage_key_uri(&claim.key_uri) {
            bail!("invalid confidential storage key URI in measured init-data");
        }
        if registry.insert(claim.volume_id.clone(), claim).is_some() {
            bail!("duplicate confidential storage volume ID in measured init-data");
        }
    }

    Ok(Some(registry))
}

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

    if let Some(registry) = confidential_storage_registry_from_initdata(&initdata)? {
        CONFIDENTIAL_STORAGE_REGISTRY.set(registry).map_err(|_| {
            anyhow::anyhow!("confidential storage registry initialized more than once")
        })?;
    }

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
    };

    Ok(Some(res))
}

#[cfg(test)]
mod tests {
    use super::*;

    const INITDATA_IMG_PATH: &str = "testdata/initdata.img";
    const INITDATA_PLAINTEXT: &[u8] = b"some content";

    #[tokio::test]
    async fn parse_initdata() {
        let initdata = read_initdata(INITDATA_IMG_PATH).await.unwrap();
        assert_eq!(initdata, INITDATA_PLAINTEXT);
    }

    #[test]
    fn extracts_versioned_confidential_storage_registry() {
        let mut initdata = InitData::new("sha384", "0.1.0");
        initdata.insert_data(
            CONFIDENTIAL_STORAGE_CLAIM,
            r#"{"version":1,"volumes":[{"profile":"luks2-integrity-ext4","volumeId":"tenant/workload/volume","keyUri":"kbs:///tenant/storage/key"}]}"#,
        );

        let registry = confidential_storage_registry_from_initdata(&initdata)
            .unwrap()
            .unwrap();
        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry.get("tenant/workload/volume"),
            Some(&ConfidentialStorageClaim {
                profile: CONFIDENTIAL_STORAGE_PROFILE_LUKS2_INTEGRITY_EXT4.to_string(),
                volume_id: "tenant/workload/volume".to_string(),
                key_uri: "kbs:///tenant/storage/key".to_string(),
            })
        );
    }

    #[test]
    fn rejects_invalid_confidential_storage_registries() {
        for document in [
            r#"{"version":2,"volumes":[]}"#,
            r#"{"version":1,"unexpected":true,"volumes":[]}"#,
            r#"{"version":1,"volumes":[{"profile":"unknown","volumeId":"tenant/volume","keyUri":"kbs:///tenant/key"}]}"#,
            r#"{"version":1,"volumes":[{"profile":"luks2-integrity-ext4","volumeId":"tenant//volume","keyUri":"kbs:///tenant/key"}]}"#,
            r#"{"version":1,"volumes":[{"profile":"luks2-integrity-ext4","volumeId":"tenant/volume","keyUri":"https://example.invalid/key"}]}"#,
            r#"{"version":1,"volumes":[{"profile":"luks2-integrity-ext4","volumeId":"tenant/volume","keyUri":"kbs:///tenant/key"},{"profile":"luks2-integrity-ext4","volumeId":"tenant/volume","keyUri":"kbs:///tenant/other"}]}"#,
        ] {
            let mut initdata = InitData::new("sha384", "0.1.0");
            initdata.insert_data(CONFIDENTIAL_STORAGE_CLAIM, document);

            assert!(confidential_storage_registry_from_initdata(&initdata).is_err());
        }
    }

    #[test]
    fn absent_confidential_storage_registry_authorizes_nothing() {
        let initdata = InitData::new("sha384", "0.1.0");

        assert!(confidential_storage_registry_from_initdata(&initdata)
            .unwrap()
            .is_none());
    }
}
