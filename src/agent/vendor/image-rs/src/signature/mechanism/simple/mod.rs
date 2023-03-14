// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::*;
use async_trait::async_trait;
use oci_distribution::secrets::RegistryAuth;
use serde::*;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::Path;
use std::path::PathBuf;
use strum_macros::Display;
use strum_macros::EnumString;
use tokio::fs;

mod sigstore;
mod verify;

pub use self::sigstore::SigstoreConfig;
pub use self::sigstore::{format_sigstore_name, get_sigs_from_specific_sigstore};
pub use verify::verify_sig_and_extract_payload;

use crate::config::Paths;
use crate::signature::image::digest::Digest;
use crate::signature::image::Image;
use crate::signature::policy::ref_match::PolicyReqMatchType;

use super::SignScheme;

/// The name of resource to request sigstore config from kbs
pub const SIG_STORE_CONFIG_KBS: &str = "Sigstore Config";

/// The name of gpg key ring to request sigstore config from kbs
pub const GPG_KEY_RING_KBS: &str = "GPG Keyring";

#[derive(Deserialize, Debug, PartialEq, Eq, Serialize, Default)]
pub struct SimpleParameters {
    // KeyType specifies what kind of the public key to verify the signatures.
    #[serde(rename = "keyType")]
    pub key_type: String,

    // KeyPath is a pathname to a local file containing the trusted key(s).
    // Exactly one of KeyPath and KeyData can be specified.
    //
    // This field is optional.
    #[serde(rename = "keyPath")]
    pub key_path: Option<String>,
    // KeyData contains the trusted key(s), base64-encoded.
    // Exactly one of KeyPath and KeyData can be specified.
    //
    // This field is optional.
    #[serde(rename = "keyData")]
    pub key_data: Option<String>,

    // SignedIdentity specifies what image identity the signature must be claiming about the image.
    // Defaults to "match-exact" if not specified.
    //
    // This field is optional.
    #[serde(default, rename = "signedIdentity")]
    pub signed_identity: Option<PolicyReqMatchType>,

    /// Dir of `Sigstore Config file`
    #[serde(skip)]
    pub sig_store_config_dir: String,

    /// Default sigstore config file
    #[serde(skip)]
    pub default_sig_store_config_file: String,

    /// Path to the gpg pubkey ring of the signature
    #[serde(skip)]
    pub gpg_key_ring: String,
}

/// Prepare directories for configs and sigstore configs.
/// It will create (if not) the following dirs:
/// * [`SIG_STORE_CONFIG_DIR`]
async fn prepare_runtime_dirs(sig_store_config_dir: &str) -> Result<()> {
    if !Path::new(sig_store_config_dir).exists() {
        fs::create_dir_all(sig_store_config_dir)
            .await
            .map_err(|e| anyhow!("Create Simple Signing sigstore-config dir failed: {:?}", e))?;
    }
    Ok(())
}

#[async_trait]
impl SignScheme for SimpleParameters {
    /// Init simple scheme signing
    async fn init(&mut self, config: &Paths) -> Result<()> {
        self.sig_store_config_dir = config.sig_store_config_dir.clone();
        self.default_sig_store_config_file = config.default_sig_store_config_file.clone();
        self.gpg_key_ring = config.gpg_key_ring.clone();
        prepare_runtime_dirs(&self.sig_store_config_dir).await?;

        Ok(())
    }

    /// Check whether [`SIG_STORE_CONFIG_DIR`] and [`GPG_KEY_RING`] exist.
    fn resource_manifest(&self) -> HashMap<&str, &str> {
        let mut res = HashMap::<&str, &str>::new();

        // Sigstore Config
        if PathBuf::from(&self.sig_store_config_dir)
            .read_dir()
            .map(|mut i| i.next().is_none())
            .unwrap_or(false)
        {
            res.insert(SIG_STORE_CONFIG_KBS, &self.default_sig_store_config_file);
        }

        // gpg key ring
        if !Path::new(&self.gpg_key_ring).exists() {
            res.insert(GPG_KEY_RING_KBS, &self.gpg_key_ring);
        }

        res
    }

    async fn allows_image(&self, image: &mut Image, _auth: &RegistryAuth) -> Result<()> {
        // FIXME: only support "GPGKeys" type now.
        //
        // refer to https://github.com/confidential-containers/image-rs/issues/14
        if self.key_type != KeyType::Gpg.to_string() {
            bail!(
                "Unknown key type in policy config: only support {} now.",
                KeyType::Gpg.to_string()
            );
        }

        let pubkey_ring = match (&self.key_path, &self.key_data) {
            (None, None) => bail!("Neither keyPath or keyData specified."),
            (Some(_), Some(_)) => bail!("Both keyPath and keyData specified."),
            (None, Some(key_data)) => base64::decode(key_data)?,
            (Some(key_path), None) => fs::read(key_path).await.map_err(|e| {
                anyhow!("Read SignedBy keyPath failed: {:?}, path: {}", e, key_path)
            })?,
        };

        let sigs = get_signatures(image).await?;
        let mut reject_reasons: Vec<anyhow::Error> = Vec::new();

        for sig in sigs.iter() {
            match judge_single_signature(
                image,
                self.signed_identity.as_ref(),
                pubkey_ring.clone(),
                sig.to_vec(),
            ) {
                // One accepted signature is enough.
                Result::Ok(()) => {
                    return Ok(());
                }
                Result::Err(e) => {
                    reject_reasons.push(e);
                }
            }
        }

        if reject_reasons.is_empty() {
            reject_reasons.push(anyhow!("Can not find any signatures."));
        }

        Err(anyhow!(
            "The signatures do not satisfied! Reject reason: {:?}",
            reject_reasons
        ))
    }
}

#[derive(Deserialize, EnumString, Display, Debug, PartialEq, Eq, Clone)]
pub enum KeyType {
    #[strum(to_string = "GPGKeys")]
    Gpg,
}

pub fn judge_single_signature(
    image: &Image,
    signed_identity: Option<&PolicyReqMatchType>,
    pubkey_ring: Vec<u8>,
    sig: Vec<u8>,
) -> Result<()> {
    // Verify the signature with the pubkey ring.
    let sig_payload = verify::verify_sig_and_extract_payload(pubkey_ring, sig)?;

    // Verify whether the information recorded in signature payload
    // is consistent with the real information of the image.
    //
    // The match policy of image-reference is the "signedIdentity" field.
    // If the signedIdentity field is not set, by default will be set
    // `matchRepoDigestOrExact`
    let signed_identity = match signed_identity {
        Some(rule) => rule,
        None => &PolicyReqMatchType::MatchRepoDigestOrExact,
    };

    sig_payload.validate_signed_docker_reference(&image.reference, signed_identity)?;
    sig_payload.validate_signed_docker_manifest_digest(&image.manifest_digest.to_string())?;

    Ok(())
}

pub async fn get_signatures(image: &mut Image) -> Result<Vec<Vec<u8>>> {
    // Get image digest (manifest digest)
    let image_digest = if !image.manifest_digest.is_empty() {
        image.manifest_digest.clone()
    } else if let Some(d) = image.reference.digest() {
        Digest::try_from(d)?
    } else {
        bail!("Missing image digest");
    };

    // Format the sigstore name: `image-repository@digest-algorithm=digest-value`.
    let sigstore_name = sigstore::format_sigstore_name(&image.reference, image_digest);

    // If the registry support `X-Registry-Supports-Signatures` API extension,
    // try to get signatures from the registry first.
    // Else, get signatures from "sigstore" according to the sigstore config file.
    // (https://github.com/containers/image/issues/384)
    //
    // TODO: Add get signatures from registry X-R-S-S API extension.
    //
    // issue: https://github.com/confidential-containers/image-rs/issues/12
    let sigstore_config =
        sigstore::SigstoreConfig::new_from_configs(crate::config::SIG_STORE_CONFIG_DIR).await?;

    let sigstore_base_url = sigstore_config
        .base_url(&image.reference)?
        .ok_or_else(|| anyhow!("The sigstore base url is none"))?;

    let sigstore = format!("{}/{}", &sigstore_base_url, &sigstore_name);
    let sigstore_uri =
        url::Url::parse(&sigstore).map_err(|e| anyhow!("Failed to parse sigstore_uri: {:?}", e))?;

    let sigs = sigstore::get_sigs_from_specific_sigstore(sigstore_uri).await?;

    Ok(sigs)
}
