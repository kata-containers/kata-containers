// Copyright (c) 2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha384, Sha512};
use std::{collections::HashMap, io::Read};

/// Currently, initdata only supports version 0.1.0.
const INITDATA_VERSION: &str = "0.1.0";
/// supported algorithms list
const SUPPORTED_ALGORITHMS: [&str; 3] = ["sha256", "sha384", "sha512"];

/// TEE platform type
#[derive(Debug, Default, Clone, Copy)]
pub enum ProtectedPlatform {
    /// Tdx platform for Intel TDX
    Tdx,
    /// Snp platform for AMD SEV-SNP
    Snp,
    /// Cca platform for ARM CCA
    Cca,
    /// Default with no protection
    #[default]
    NoProtection,
}

#[allow(clippy::doc_lazy_continuation)]
/// <https://github.com/confidential-containers/trustee/blob/47d7a2338e0be76308ac19be5c0c172c592780aa/kbs/docs/initdata.md>
/// The Initdata specification defines the key data structures and algorithms for injecting any well-defined data
/// from an untrusted host into a TEE (Trusted Execution Environment). To guarantee the integrity of the data,
/// either the hostdata capability of TEE evidence or the (v)TPM dynamic measurement capability will be utilized.
/// And its format looks like as below:
/// ```toml
/// algorithm = "sha384"
/// version = "0.1.0"
///
/// [data]
/// key1 = "value1"
/// key2 = "value2"
///```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct InitData {
    /// version of InitData Spec
    version: String,
    /// algorithm: sha256, sha512, sha384
    algorithm: String,
    /// data for specific "key:value"
    data: HashMap<String, String>,
}

impl InitData {
    /// new InitData
    pub fn new(algorithm: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            algorithm: algorithm.into(),
            data: HashMap::new(),
        }
    }

    /// get coco data
    pub fn get_coco_data(&self, key: &str) -> Option<&String> {
        self.data.get(key)
    }

    /// insert data items
    pub fn insert_data(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.data.insert(key.into(), value.into());
    }

    /// get algorithm
    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }

    /// get version
    pub fn version(&self) -> &str {
        &self.version
    }

    /// get data
    pub fn data(&self) -> &HashMap<String, String> {
        &self.data
    }

    /// serialize it to Vec<u8>
    pub fn to_vec(&self) -> Result<Vec<u8>> {
        Ok(toml::to_vec(&self)?)
    }

    /// serialize config to TOML string
    pub fn to_string(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// Validate InitData
    pub fn validate(&self) -> Result<()> {
        // Currently, it only supports 0.1.0
        if self.version != INITDATA_VERSION {
            return Err(anyhow!(
                "unsupported version: {}, expected: {}",
                self.version,
                INITDATA_VERSION
            ));
        }

        if !SUPPORTED_ALGORITHMS
            .iter()
            .any(|&alg| alg == self.algorithm)
        {
            return Err(anyhow!(
                "unsupported algorithm: {}, supported algorithms: {}",
                self.algorithm,
                SUPPORTED_ALGORITHMS.join(", ")
            ));
        }

        Ok(())
    }
}

/// calculate initdata digest
fn calculate_digest(algorithm: &str, data: &str) -> Result<Vec<u8>> {
    let digest = match algorithm {
        "sha256" => {
            let mut hasher = Sha256::new();
            hasher.update(&data);
            hasher.finalize().to_vec()
        }
        "sha384" => {
            let mut hasher = Sha384::new();
            hasher.update(&data);
            hasher.finalize().to_vec()
        }
        "sha512" => {
            let mut hasher = Sha512::new();
            hasher.update(&data);
            hasher.finalize().to_vec()
        }
        _ => return Err(anyhow!("unsupported Hash algorithm: {}", algorithm).into()),
    };

    Ok(digest)
}

/// Handle digest for different TEE platform
fn adjust_digest(digest: &[u8], platform: ProtectedPlatform) -> Vec<u8> {
    let required_len = match platform {
        ProtectedPlatform::Tdx => 48,
        ProtectedPlatform::Snp => 32,
        ProtectedPlatform::Cca => 64,
        ProtectedPlatform::NoProtection => digest.len(),
    };

    let mut adjusted = Vec::with_capacity(required_len);

    if digest.len() >= required_len {
        adjusted.extend_from_slice(&digest[..required_len]);
    } else {
        adjusted.extend_from_slice(digest);
        adjusted.resize(required_len, 0u8); // padding with zero
    }

    // Vec<u8>
    adjusted
}

/// Parse initdata
fn parse_initdata(initdata_str: &str) -> Result<InitData> {
    let initdata: InitData = toml::from_str(&initdata_str)?;
    initdata.validate()?;

    Ok(initdata)
}

/// calculate initdata digest
/// 1. Parse InitData
/// 2. Calculate Digest
/// 3. Adjust Digest with Platform
/// 4. Encode digest with base64/Standard
pub fn calculate_initdata_digest(
    initdata_toml: &str,
    platform: ProtectedPlatform,
) -> Result<String> {
    // 1. Parse InitData
    let initdata: InitData = parse_initdata(initdata_toml).context("parse initdata")?;
    let algorithm: &str = &initdata.algorithm;

    // 2. Calculate Digest
    let digest = calculate_digest(algorithm, &initdata_toml).context("calculate digest")?;

    // 3. Adjust Digest with Platform
    let digest_platform = adjust_digest(&digest, platform);

    // 4. Encode digest with base64/Standard
    let b64encoded_digest = base64::encode_config(digest_platform, base64::STANDARD);

    Ok(b64encoded_digest)
}

/// The argument `initda_annotation` is a Standard base64 encoded string containing a TOML formatted content.
/// This function decodes the base64 string, parses the TOML content into an InitData structure.
pub fn add_hypervisor_initdata_overrides(initda_annotation: &str) -> Result<String> {
    // Base64 decode the annotation value
    let b64_decoded =
        base64::decode_config(initda_annotation, base64::STANDARD).context("base64 decode")?;

    // Gzip decompress the decoded data
    let mut gz_decoder = GzDecoder::new(&b64_decoded[..]);
    let mut initdata_str = String::new();
    gz_decoder
        .read_to_string(&mut initdata_str)
        .context("gz decoder failed")?;

    // Parse the initdata
    let initdata: InitData = parse_initdata(&initdata_str).context("parse initdata overrides")?;

    // initdata within a TOML string
    initdata.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    /// Test InitData creation and serialization
    #[test]
    fn test_init_data() {
        let mut init_data = InitData::new("sha384", "0.1.0");
        init_data.insert_data("initdata_key", "initdata_value");

        // Verify data insertion
        assert_eq!(
            init_data.data().get("initdata_key").unwrap(),
            "initdata_value"
        );
        assert_eq!(init_data.version(), "0.1.0");
        assert_eq!(init_data.algorithm(), "sha384");

        // Test TOML serialization
        let toml_str = init_data.to_string().unwrap();
        assert!(toml_str.contains("initdata_key = 'initdata_value'\n"));
        assert!(toml_str.starts_with("version = '0.1.0'"));
    }

    /// Test calculate_digest with different algorithms
    #[test]
    fn test_calculate_digest() {
        let data = "test_data";

        // Test SHA256
        let sha256 = calculate_digest("sha256", data).unwrap();
        assert_eq!(sha256.len(), 32);

        // Test SHA384
        let sha384 = calculate_digest("sha384", data).unwrap();
        assert_eq!(sha384.len(), 48);

        // Test SHA512
        let sha512 = calculate_digest("sha512", data).unwrap();
        assert_eq!(sha512.len(), 64);

        // Test invalid algorithm
        assert!(calculate_digest("md5", data).is_err());
    }

    /// Test digest adjustment for different platforms
    #[test]
    fn test_adjust_digest() {
        let sample_digest = vec![0xAA; 64]; // 64-byte digest

        // Test TDX platform (requires 48 bytes)
        let tdx_result = adjust_digest(&sample_digest, ProtectedPlatform::Tdx);
        assert_eq!(tdx_result.len(), 48);
        assert_eq!(&tdx_result[..48], &sample_digest[..48]);

        // Test SNP platform (requires 32 bytes)
        let snp_result = adjust_digest(&sample_digest, ProtectedPlatform::Snp);
        assert_eq!(snp_result.len(), 32);

        // Test short digest with CCA platform (requires 64 bytes)
        let short_digest = vec![0xBB; 32];
        let cca_result = adjust_digest(&short_digest, ProtectedPlatform::Cca);
        assert_eq!(cca_result.len(), 64);
        assert_eq!(&cca_result[..32], &short_digest[..]);
        assert_eq!(&cca_result[32..], vec![0u8; 32]);
    }

    /// Test hypervisor initdata processing with compression
    #[test]
    fn test_hypervisor_initdata_processing() {
        // Create test initdata
        let mut init_data = InitData::new("sha512", "0.1.0");
        init_data.insert_data("hypervisor_key", "config_value");

        // Create compressed annotation
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(init_data.to_string().unwrap().as_bytes())
            .unwrap();
        let compressed = encoder.finish().unwrap();
        let b64_annotation = base64::encode(compressed);

        // Test processing
        let result = add_hypervisor_initdata_overrides(&b64_annotation).unwrap();
        assert!(result.contains("hypervisor_key = 'config_value'\n"));
        assert!(result.contains("algorithm = 'sha512'\n"));
    }

    /// Test input validation
    #[test]
    fn test_initdata_validation() {
        // Valid TOML
        let valid_toml = r#"
            version = "0.1.0"
            algorithm = "sha384"
            
            [data]
            valid_key = "valid_value"
        "#;
        assert!(parse_initdata(valid_toml).is_ok());

        // Invalid TOML (missing version)
        let invalid_toml = r#"
            algorithm = "sha256"
            
            [data]
            key = "value"
        "#;
        assert!(parse_initdata(invalid_toml).is_err());
    }

    /// Test error handling for malformed inputs
    #[test]
    fn test_error_handling() {
        // Invalid base64
        assert!(add_hypervisor_initdata_overrides("invalid_base64!!").is_err());

        // Invalid compression format
        let invalid_data = base64::encode("raw uncompressed data");
        assert!(add_hypervisor_initdata_overrides(&invalid_data).is_err());
    }
}
