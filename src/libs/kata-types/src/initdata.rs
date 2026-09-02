// Copyright (c) 2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::sl;
use anyhow::{anyhow, bail, Context, Result};
use base64::Engine as _;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha384, Sha512};
use std::{collections::BTreeMap, io::Read, path::Path};

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
    /// Se platform for IBM SEL
    Se,
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
    ///
    /// Ordered rather than hashed, because a document Kata serializes itself
    /// (see [`merge_initdata_documents`]) must come out byte-identical for
    /// identical inputs: the digest of that text is what a verifier checks
    /// against the TEE launch measurement.
    data: BTreeMap<String, String>,
}

impl InitData {
    /// new InitData
    pub fn new(algorithm: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            algorithm: algorithm.into(),
            data: BTreeMap::new(),
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
    pub fn data(&self) -> &BTreeMap<String, String> {
        &self.data
    }

    /// serialize it to Vec<u8>
    pub fn to_vec(&self) -> Result<Vec<u8>> {
        Ok(toml::to_string(&self)?.into_bytes())
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
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        "sha384" => {
            let mut hasher = Sha384::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        "sha512" => {
            let mut hasher = Sha512::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        _ => return Err(anyhow!("unsupported Hash algorithm: {algorithm}")),
    };

    Ok(digest)
}

/// Handle digest for different TEE platform
fn adjust_digest(digest: &[u8], platform: ProtectedPlatform) -> Vec<u8> {
    let required_len = match platform {
        ProtectedPlatform::Tdx => 48,
        ProtectedPlatform::Snp => 32,
        ProtectedPlatform::Cca => 64,
        ProtectedPlatform::Se => 256,
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
pub fn parse_initdata(initdata_str: &str) -> Result<InitData> {
    let initdata: InitData = toml::from_str(initdata_str)?;
    initdata.validate()?;

    Ok(initdata)
}

/// Magic number starting a packed initdata image.
///
/// The image layout is this 8-byte magic, an 8-byte little-endian length, the
/// gzipped initdata document, then zero padding up to a 512-byte sector
/// boundary. The guest identifies the initdata block device by this magic, so
/// the writers and readers of the format all share the constant from here.
pub const INITDATA_IMAGE_MAGIC: &[u8; 8] = b"initdata";

/// Length of the packed initdata image header: the magic plus the payload length.
const INITDATA_IMAGE_HEADER_LEN: usize = INITDATA_IMAGE_MAGIC.len() + 8;

/// An initdata document loaded from a file on the host.
#[derive(Debug)]
pub struct InitdataFile {
    /// The initdata TOML document.
    pub document: String,
    /// Whether the file was already a packed initdata image, and can therefore
    /// be attached to a guest as it sits on disk rather than being packed again.
    pub packed: bool,
}

/// Whether `data` starts with the packed initdata image magic.
pub fn is_packed_initdata_image(data: &[u8]) -> bool {
    data.starts_with(INITDATA_IMAGE_MAGIC)
}

/// Extract the initdata document from a packed initdata image.
pub fn unpack_initdata_image(image: &[u8]) -> Result<String> {
    if !is_packed_initdata_image(image) {
        bail!("not a packed initdata image: the initdata magic is missing");
    }
    if image.len() < INITDATA_IMAGE_HEADER_LEN {
        bail!(
            "truncated initdata image: {} bytes, shorter than the {}-byte header",
            image.len(),
            INITDATA_IMAGE_HEADER_LEN
        );
    }

    let mut length = [0u8; 8];
    length.copy_from_slice(&image[INITDATA_IMAGE_MAGIC.len()..INITDATA_IMAGE_HEADER_LEN]);
    let length = u64::from_le_bytes(length);

    let payload = &image[INITDATA_IMAGE_HEADER_LEN..];
    if length > payload.len() as u64 {
        bail!(
            "initdata image declares a {}-byte payload but only {} bytes follow the header",
            length,
            payload.len()
        );
    }
    let payload = &payload[..length as usize];

    let mut document = String::new();
    GzDecoder::new(payload)
        .read_to_string(&mut document)
        .context("decompressing the initdata image payload")?;

    Ok(document)
}

/// Load an initdata document from a file, which may be either a packed initdata
/// image or a bare initdata TOML document.
///
/// The two forms are told apart by the image magic, so an operator can install
/// whichever they have under a single configuration key: a document to hand-edit
/// on the node, or a prebuilt image to ship as a versioned artifact.
///
/// The document is parsed and validated before returning, so a malformed file is
/// reported against the file that holds it.
pub fn load_initdata_file(path: impl AsRef<Path>) -> Result<InitdataFile> {
    let path = path.as_ref();
    let raw = std::fs::read(path)
        .with_context(|| format!("reading the initdata file {}", path.display()))?;

    let (document, packed) = if is_packed_initdata_image(&raw) {
        let document = unpack_initdata_image(&raw)
            .with_context(|| format!("unpacking the initdata image {}", path.display()))?;
        (document, true)
    } else {
        let document = String::from_utf8(raw)
            .with_context(|| format!("the initdata file {} is not valid UTF-8", path.display()))?;
        (document, false)
    };

    parse_initdata(&document)
        .with_context(|| format!("parsing the initdata document in {}", path.display()))?;

    Ok(InitdataFile { document, packed })
}

/// Overlay one initdata document onto another, entry by entry.
///
/// `overlay` wins: its `[data]` entries replace same-named entries from `base`,
/// and the result carries its `algorithm` and `version`. This lets a node supply
/// defaults, such as an `aa.toml` naming the cluster's KBS, while a workload
/// adds or replaces individual entries.
///
/// The result is re-serialized, so it is this merged text -- not either input --
/// that gets digested, bound into the TEE launch measurement, and handed to the
/// guest.
pub fn merge_initdata_documents(base: &str, overlay: &str) -> Result<String> {
    let base = parse_initdata(base).context("parsing the base initdata document")?;
    let overlay = parse_initdata(overlay).context("parsing the overlay initdata document")?;

    let mut merged = InitData::new(overlay.algorithm(), overlay.version());
    for (key, value) in base.data().iter().chain(overlay.data()) {
        merged.insert_data(key.clone(), value.clone());
    }

    merged
        .to_string()
        .context("serializing the merged initdata document")
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
    let digest = calculate_digest(algorithm, initdata_toml).context("calculate digest")?;

    // 3. Adjust Digest with Platform
    let digest_platform = adjust_digest(&digest, platform);

    // 4. Encode digest with base64/Standard
    let b64encoded_digest = base64::engine::general_purpose::STANDARD.encode(digest_platform);

    Ok(b64encoded_digest)
}

/// Encodes initdata as an annotation
pub fn encode_initdata(init_data: &InitData) -> String {
    let toml_str = toml::to_string_pretty(&init_data).unwrap();
    create_encoded_input(&toml_str)
}

/// Decodes a base64-encoded gzipped initdata document to its raw TOML representation.
fn decode_raw_initdata(initdata_annotation: &str) -> Result<String> {
    // Base64 decode the annotation value
    let b64_decoded = base64::engine::general_purpose::STANDARD
        .decode(initdata_annotation)
        .context("base64 decode")?;

    // Gzip decompress the decoded data
    let mut gz_decoder = GzDecoder::new(&b64_decoded[..]);
    let mut initdata_str = String::new();
    gz_decoder
        .read_to_string(&mut initdata_str)
        .context("gz decoder failed")?;
    Ok(initdata_str)
}

/// Decodes initdata annotation
pub fn decode_initdata(initdata_annotation: &str) -> Result<InitData> {
    let initdata_str = decode_raw_initdata(initdata_annotation)?;
    // Return parsed initdata
    let initdata = parse_initdata(&initdata_str).context("parse initdata overrides")?;

    Ok(initdata)
}

/// The argument `initdata_annotation` is a Standard base64 encoded string containing a TOML formatted content.
/// This function decodes the base64 string, parses the TOML content into an InitData structure.
pub fn add_hypervisor_initdata_overrides(initdata_annotation: &str) -> Result<String> {
    // If the initdata is empty, return an empty string
    if initdata_annotation.is_empty() {
        info!(sl!(), "initdata_annotation is empty");
        return Ok("".to_string());
    }

    decode_raw_initdata(initdata_annotation).context("decoding initdata annotation failed")
}

use std::io::Write;

/// create gzipped and base64 encoded string
fn create_encoded_input(content: &str) -> String {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(content.as_bytes()).unwrap();
    let compressed = encoder.finish().unwrap();
    base64::engine::general_purpose::STANDARD.encode(&compressed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    #[test]
    fn test_empty_annotation() {
        // Test with empty string input
        let result = add_hypervisor_initdata_overrides("");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_empty_data_section() {
        // Test with empty data section
        let toml_content = r#"
algorithm = "sha384"
version = "0.1.0"

[data]
"#;
        let encoded = create_encoded_input(toml_content);

        let result = add_hypervisor_initdata_overrides(&encoded);
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_complete_initdata() {
        // Test with complete InitData structure
        let toml_content = r#"
algorithm = "sha384"
version = "0.1.0"

[data]
"aa.toml" = '''
[token_configs]
[token_configs.coco_as]
url = 'http://kbs-service.xxx.cluster.local:8080'

[token_configs.kbs]
url = 'http://kbs-service.xxx.cluster.local:8080'
'''

"cdh.toml" = '''
socket = 'unix:///run/guest-services/cdh.sock'
credentials = []

[kbc]
name = 'cc_kbc'
url = 'http://kbs-service.xxx.cluster.local:8080'
'''
"#;
        let encoded = create_encoded_input(toml_content);

        let result = add_hypervisor_initdata_overrides(&encoded);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(!output.is_empty());
        assert!(output.contains("algorithm"));
        assert!(output.contains("version"));
    }

    #[test]
    fn test_invalid_base64() {
        // Test with invalid base64 string
        let invalid_base64 = "This is not valid base64!";

        let result = add_hypervisor_initdata_overrides(invalid_base64);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error
            .chain()
            .any(|e| e.to_string().contains("base64 decode")));
    }

    #[test]
    fn test_valid_base64_invalid_gzip() {
        // Test with valid base64 but invalid gzip content
        let not_gzipped = "This is not gzipped content";
        let encoded = base64::engine::general_purpose::STANDARD.encode(not_gzipped.as_bytes());

        let result = add_hypervisor_initdata_overrides(&encoded);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error
            .chain()
            .any(|e| e.to_string().contains("gz decoder failed")));
    }

    #[test]
    fn test_missing_algorithm() {
        // Test with missing algorithm field
        let toml_content = r#"
version = "0.1.0"

[data]
"test.toml" = '''
key = "value"
'''
"#;
        let encoded = create_encoded_input(toml_content);

        let result = add_hypervisor_initdata_overrides(&encoded);
        // This might fail depending on whether algorithm is required
        if let Err(error) = result {
            assert!(error.to_string().contains("parse initdata"));
        }
    }

    #[test]
    fn test_missing_version() {
        // Test with missing version field
        let toml_content = r#"
algorithm = "sha384"

[data]
"test.toml" = '''
key = "value"
'''
"#;
        let encoded = create_encoded_input(toml_content);

        let result = add_hypervisor_initdata_overrides(&encoded);
        // This might fail depending on whether version is required
        if let Err(error) = result {
            assert!(error.to_string().contains("parse initdata"));
        }
    }

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
        assert!(toml_str.contains("initdata_key = \"initdata_value\"\n"));
        assert!(toml_str.starts_with("version = \"0.1.0\""));
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

        // Test SE platform (requires 256 bytes)
        let long_digest = vec![0xAA; 256];
        let se_result = adjust_digest(&long_digest, ProtectedPlatform::Se);
        assert_eq!(se_result.len(), 256);
        assert_eq!(&se_result[..256], &long_digest[..256]);
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
        let b64_annotation = base64::engine::general_purpose::STANDARD.encode(compressed);

        // Test processing
        let result = add_hypervisor_initdata_overrides(&b64_annotation).unwrap();
        assert!(result.contains("hypervisor_key = \"config_value\"\n"));
        assert!(result.contains("algorithm = \"sha512\"\n"));
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

    #[test]
    fn test_pretty_initdata() {
        let nested_toml = r#"
algorithm = "sha384"
version = "0.1.0"

[data]
"aa.toml" = '''
[token_configs]
[token_configs.coco_as]
url = 'http://kbs-service.xxx.cluster.local:8080'

[token_configs.kbs]
url = 'http://kbs-service.xxx.cluster.local:8080'
'''
        "#;
        let init_data = parse_initdata(nested_toml).expect("canned initdata document should parse");

        let doc = decode_raw_initdata(&encode_initdata(&init_data))
            .expect("encoding and decoding again should work");
        assert!(
            !doc.contains("\\n"),
            "the encoded initdata toml should not contain escaped newlines, but does:\n{}",
            doc
        )
    }

    const NODE_DOCUMENT: &str = r#"version = "0.1.0"
algorithm = "sha384"

[data]
"aa.toml" = "node aa"
"cdh.toml" = "node cdh"
"#;

    const POD_DOCUMENT: &str = r#"version = "0.1.0"
algorithm = "sha256"

[data]
"aa.toml" = "pod aa"
"policy.rego" = "pod policy"
"#;

    /// Pack a document the way the runtime does: magic, payload length, gzipped
    /// document, then zero padding up to a sector boundary.
    fn pack_initdata_image(document: &str) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(document.as_bytes()).unwrap();
        let payload = encoder.finish().unwrap();

        let mut image = Vec::new();
        image.extend_from_slice(INITDATA_IMAGE_MAGIC);
        image.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        image.extend_from_slice(&payload);

        let padding = (512 - image.len() % 512) % 512;
        image.resize(image.len() + padding, 0);
        image
    }

    #[test]
    fn unpacks_a_packed_image() {
        let image = pack_initdata_image(NODE_DOCUMENT);
        assert!(is_packed_initdata_image(&image));
        assert_eq!(unpack_initdata_image(&image).unwrap(), NODE_DOCUMENT);
    }

    #[test]
    fn a_bare_document_is_not_a_packed_image() {
        assert!(!is_packed_initdata_image(NODE_DOCUMENT.as_bytes()));
        // Shorter than the magic itself.
        assert!(!is_packed_initdata_image(b"init"));
        assert!(unpack_initdata_image(NODE_DOCUMENT.as_bytes()).is_err());
    }

    #[test]
    fn rejects_an_image_with_nothing_after_the_magic() {
        let err = unpack_initdata_image(INITDATA_IMAGE_MAGIC)
            .unwrap_err()
            .to_string();
        assert!(err.contains("truncated initdata image"), "{}", err);
    }

    #[test]
    fn rejects_an_image_whose_payload_is_truncated() {
        let mut image = pack_initdata_image(NODE_DOCUMENT);
        image.truncate(INITDATA_IMAGE_HEADER_LEN + 2);

        let err = unpack_initdata_image(&image).unwrap_err().to_string();
        assert!(err.contains("only 2 bytes follow the header"), "{}", err);
    }

    #[test]
    fn loads_either_form_from_a_file() {
        let dir = tempfile::tempdir().unwrap();

        let document_path = dir.path().join("initdata.toml");
        std::fs::write(&document_path, NODE_DOCUMENT).unwrap();
        let loaded = load_initdata_file(&document_path).unwrap();
        assert_eq!(loaded.document, NODE_DOCUMENT);
        assert!(!loaded.packed);

        let image_path = dir.path().join("initdata.img");
        std::fs::write(&image_path, pack_initdata_image(NODE_DOCUMENT)).unwrap();
        let loaded = load_initdata_file(&image_path).unwrap();
        assert_eq!(loaded.document, NODE_DOCUMENT);
        assert!(loaded.packed);
    }

    #[test]
    fn rejects_a_file_that_is_not_an_initdata_document() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("initdata.toml");
        // A valid TOML file, but missing the mandatory version field.
        std::fs::write(&path, "algorithm = \"sha384\"\n[data]\n").unwrap();

        let err = format!("{:#}", load_initdata_file(&path).unwrap_err());
        assert!(err.contains("parsing the initdata document"), "{}", err);
    }

    #[test]
    fn merge_lets_the_overlay_win_per_entry() {
        let merged = merge_initdata_documents(NODE_DOCUMENT, POD_DOCUMENT).unwrap();
        let merged = parse_initdata(&merged).unwrap();

        assert_eq!(merged.data()["aa.toml"], "pod aa");
        assert_eq!(merged.data()["cdh.toml"], "node cdh");
        assert_eq!(merged.data()["policy.rego"], "pod policy");
        // The overlay's algorithm comes along, since it selects the digest.
        assert_eq!(merged.algorithm(), "sha256");
    }

    #[test]
    fn merge_is_reproducible() {
        // The digest of the merged text is what the verifier checks against the
        // launch measurement, so the same inputs must give back the same bytes.
        let first = merge_initdata_documents(NODE_DOCUMENT, POD_DOCUMENT).unwrap();
        for _ in 0..16 {
            assert_eq!(
                merge_initdata_documents(NODE_DOCUMENT, POD_DOCUMENT).unwrap(),
                first
            );
        }
    }

    #[test]
    fn merge_rejects_a_malformed_side() {
        assert!(merge_initdata_documents("not initdata", POD_DOCUMENT).is_err());
        assert!(merge_initdata_documents(NODE_DOCUMENT, "not initdata").is_err());
    }

    #[test]
    fn the_packing_script_produces_an_image_this_module_can_read() {
        // gen-initdata-image.sh spells the image layout out a second time, in
        // shell. Pin it to the reader here: a change to one side that is not
        // mirrored in the other would otherwise surface only as a guest unable
        // to read its init data.
        let script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tools/packaging/scripts/gen-initdata-image.sh");
        assert!(
            script.exists(),
            "packing script not found at {}",
            script.display()
        );

        let dir = tempfile::tempdir().unwrap();
        let document = dir.path().join("initdata.toml");
        let image = dir.path().join("initdata.img");
        std::fs::write(&document, NODE_DOCUMENT).unwrap();

        let status = std::process::Command::new("bash")
            .arg(&script)
            .arg("-o")
            .arg(&image)
            .arg(&document)
            .stdout(std::process::Stdio::null())
            .status()
            .expect("running the packing script");
        assert!(status.success(), "the packing script failed: {}", status);

        let loaded = load_initdata_file(&image).unwrap();
        assert!(loaded.packed);
        assert_eq!(loaded.document, NODE_DOCUMENT);
    }
}
