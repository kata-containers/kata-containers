// Copyright The ocicrypt Authors.
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::io::Read;

use anyhow::{anyhow, Result};

use crate::blockcipher::{
    EncryptionFinalizer, LayerBlockCipherHandler, LayerBlockCipherOptions,
    PrivateLayerBlockCipherOptions, PublicLayerBlockCipherOptions, AES256CTR,
};
use crate::config::{DecryptConfig, EncryptConfig};
use crate::keywrap::KeyWrapper;
use crate::{get_key_wrapper, KEY_WRAPPERS_ANNOTATIONS};

lazy_static! {
    static ref DEFAULT_ANNOTATION_MAP: HashMap<String, String> = HashMap::new();
}

// EncryptLayerFinalizer can get the annotations to set for the encrypted layer
#[derive(Debug, Default, Clone)]
pub struct EncLayerFinalizer {
    lbco: LayerBlockCipherOptions,
}

impl EncLayerFinalizer {
    /// Generate annotations for image decryption.
    pub fn finalize_annotations(
        &mut self,
        ec: &EncryptConfig,
        annotations: Option<&HashMap<String, String>>,
        finalizer: Option<&mut impl EncryptionFinalizer>,
    ) -> Result<HashMap<String, String>> {
        let mut priv_opts = vec![];
        let mut pub_opts = vec![];
        if let Some(finalizer) = finalizer {
            finalizer.finalized_lbco(&mut self.lbco)?;
            priv_opts = serde_json::to_vec(&self.lbco.private)?;
            pub_opts = serde_json::to_vec(&self.lbco.public)?;
        }

        let mut new_annotations = HashMap::new();
        let mut keys_wrapped = false;
        for (annotations_id, scheme) in KEY_WRAPPERS_ANNOTATIONS.iter() {
            let mut b64_annotations = String::new();
            let anno = annotations.unwrap_or(&DEFAULT_ANNOTATION_MAP);
            if let Some(key_annotations) = anno.get(annotations_id) {
                b64_annotations = key_annotations.clone();
            }

            let key_wrapper = get_key_wrapper(scheme)?;
            b64_annotations = pre_wrap_key(key_wrapper, ec, b64_annotations, &priv_opts)?;
            if !b64_annotations.is_empty() {
                keys_wrapped = true;
                new_annotations.insert(annotations_id.to_string(), b64_annotations);
            }
        }

        if !keys_wrapped {
            return Err(anyhow!("no wrapped keys produced by encryption"));
        }

        if new_annotations.is_empty() {
            return Err(anyhow!("no encryptor found to handle encryption"));
        }

        new_annotations.insert(
            "org.opencontainers.image.enc.pubopts".to_string(),
            base64::encode(pub_opts),
        );

        Ok(new_annotations)
    }
}

// pre_wrap_keys calls wrap_keys and handles the base64 encoding and
// concatenation of the annotation data.
fn pre_wrap_key(
    keywrapper: &dyn KeyWrapper,
    ec: &EncryptConfig,
    mut b64_annotations: String,
    opts_data: &[u8],
) -> Result<String> {
    let new_annotation = keywrapper.wrap_keys(ec, opts_data)?;
    if new_annotation.is_empty() {
        return Err(anyhow!("new annotations is empty!"));
    }

    let b64_new_annotation = base64::encode(new_annotation);
    if b64_annotations.is_empty() {
        return Ok(b64_new_annotation);
    }

    b64_annotations.push(',');
    b64_annotations.push_str(&b64_new_annotation);
    Ok(b64_annotations)
}

// pre_unwrap_key decodes the comma separated base64 strings and calls the unwrap_key function
// of the given keywrapper with it and returns the result in case the unwrap_key functions
// does not return an error. If all attempts fail, an error is returned.
fn pre_unwrap_key(
    keywrapper: &dyn KeyWrapper,
    dc: &DecryptConfig,
    b64_annotations: &str,
) -> Result<Vec<u8>> {
    if b64_annotations.is_empty() {
        return Err(anyhow!("annotations is empty!"));
    }

    let mut errs = String::new();
    for b64_annotation in b64_annotations.split(',') {
        let annotation = base64::decode(b64_annotation)?;

        match keywrapper.unwrap_keys(dc, &annotation) {
            Err(e) => {
                errs.push_str(&e.to_string());
                continue;
            }
            Ok(opts_data) => {
                return Ok(opts_data);
            }
        };
    }

    Err(anyhow!(
        "no suitable key found for decrypting layer key:\n {}",
        errs
    ))
}

fn get_layer_pub_opts(annotations: &HashMap<String, String>) -> Result<Vec<u8>> {
    if let Some(pub_opts) = annotations.get("org.opencontainers.image.enc.pubopts") {
        return Ok(base64::decode(pub_opts)?);
    }

    Ok(
        serde_json::to_string(&PublicLayerBlockCipherOptions::default())?
            .as_bytes()
            .to_vec(),
    )
}

fn get_layer_key_opts(
    annotations_id: &str,
    annotations: &HashMap<String, String>,
) -> Option<String> {
    // TODO: what happens if there are multiple key-providers?
    let value = if annotations_id
        .strip_prefix("org.opencontainers.image.enc.keys.provider.")
        .is_some()
    {
        annotations.iter().find_map(|(k, v)| {
            // During decryption, ignore keyprovider name in annotations and use the
            // keyprovider defined in OCICRYPT_KEYPROVIDER_CONFIG.
            if k.strip_prefix("org.opencontainers.image.enc.keys.provider.")
                .is_some()
            {
                Some(v)
            } else {
                None
            }
        })
    } else {
        annotations.get(annotations_id)
    };
    value.cloned()
}

/// Unwrap layer decryption key from OCI descriptor annotations.
pub fn decrypt_layer_key_opts_data(
    dc: &DecryptConfig,
    annotations: Option<&HashMap<String, String>>,
) -> Result<Vec<u8>> {
    let mut priv_key_given = false;
    let annotations = annotations.unwrap_or(&DEFAULT_ANNOTATION_MAP);

    for (annotations_id, scheme) in KEY_WRAPPERS_ANNOTATIONS.iter() {
        if let Some(b64_annotation) = get_layer_key_opts(annotations_id, annotations) {
            let keywrapper = get_key_wrapper(scheme)?;
            if !keywrapper.probe(&dc.param) {
                continue;
            }

            if keywrapper.private_keys(&dc.param).is_some() {
                priv_key_given = true;
            }

            if let Ok(opts_data) = pre_unwrap_key(keywrapper, dc, &b64_annotation) {
                if !opts_data.is_empty() {
                    return Ok(opts_data);
                }
            }
            // try next keywrapper
        }
    }

    if !priv_key_given {
        return Err(anyhow!("missing private key needed for decryption"));
    }

    Err(anyhow!(
        "no suitable key unwrapper found or none of the private keys could be used for decryption"
    ))
}

/// encrypt_layer encrypts the layer by running one encryptor after the other
pub fn encrypt_layer<'a, R: 'a + Read>(
    ec: &EncryptConfig,
    layer_reader: R,
    annotations: Option<&HashMap<String, String>>,
    digest: &str,
) -> Result<(
    Option<impl Read + EncryptionFinalizer + 'a>,
    EncLayerFinalizer,
)> {
    let mut encrypted = false;
    for (annotations_id, _scheme) in KEY_WRAPPERS_ANNOTATIONS.iter() {
        let anno = annotations.unwrap_or(&DEFAULT_ANNOTATION_MAP);
        if anno.contains_key(annotations_id) {
            if let Some(decrypt_config) = ec.decrypt_config.as_ref() {
                decrypt_layer_key_opts_data(decrypt_config, annotations)?;
                get_layer_pub_opts(anno)?;

                // already encrypted!
                encrypted = true;
            } else {
                return Err(anyhow!(
                    "EncryptConfig::decrypt_config must not be None for encrypted layers"
                ));
            }
        }
    }

    if !encrypted {
        let mut lbch = LayerBlockCipherHandler::new()?;
        let mut lbco = LayerBlockCipherOptions::default();

        lbch.encrypt(layer_reader, AES256CTR, &mut lbco)?;
        lbco.private.digest = digest.to_string();
        let enc_layer_finalizer = EncLayerFinalizer { lbco };

        Ok((Some(lbch), enc_layer_finalizer))
    } else {
        Ok((None, EncLayerFinalizer::default()))
    }
}

// decrypt_layer decrypts a layer trying one keywrapper after the other to see whether it
// can apply the provided private key
// If unwrap_only is set we will only try to decrypt the layer encryption key and return
pub fn decrypt_layer<R: Read>(
    dc: &DecryptConfig,
    layer_reader: R,
    annotations: Option<&HashMap<String, String>>,
    unwrap_only: bool,
) -> Result<(Option<impl Read>, String)> {
    let priv_opts_data = decrypt_layer_key_opts_data(dc, annotations)?;
    let annotations = annotations.unwrap_or(&DEFAULT_ANNOTATION_MAP);
    let pub_opts_data = get_layer_pub_opts(annotations)?;

    if unwrap_only {
        return Ok((None, "".to_string()));
    }

    let priv_opts: PrivateLayerBlockCipherOptions = serde_json::from_slice(&priv_opts_data)?;
    let pub_opts: PublicLayerBlockCipherOptions = serde_json::from_slice(&pub_opts_data)?;
    let mut opts = LayerBlockCipherOptions {
        public: pub_opts,
        private: priv_opts,
    };
    let mut lbch = LayerBlockCipherHandler::new()?;

    lbch.decrypt(layer_reader, &mut opts)?;

    Ok((Some(lbch), opts.private.digest))
}

/// This is a streaming version of [`decrypt_layer`].
///
/// priv_opts_data can get from [`decrypt_layer_key_opts_data`]
#[cfg(feature = "async-io")]
pub fn async_decrypt_layer<R: tokio::io::AsyncRead + Send>(
    layer_reader: R,
    annotations: Option<&HashMap<String, String>>,
    priv_opts_data: &[u8],
) -> Result<(impl tokio::io::AsyncRead + Send, String)> {
    let annotations = annotations.unwrap_or(&DEFAULT_ANNOTATION_MAP);
    let pub_opts_data = get_layer_pub_opts(annotations)?;
    let pub_opts: PublicLayerBlockCipherOptions = serde_json::from_slice(&pub_opts_data)?;
    let priv_opts: PrivateLayerBlockCipherOptions = serde_json::from_slice(priv_opts_data)?;
    let mut opts = LayerBlockCipherOptions {
        public: pub_opts,
        private: priv_opts,
    };
    let mut lbch = LayerBlockCipherHandler::new()?;

    lbch.decrypt(layer_reader, &mut opts)?;

    Ok((lbch, opts.private.digest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn test_encrypt_decrypt_layer() {
        let path = load_data_path();
        let test_conf_path = format!("{}/{}", path, "ocicrypt_config.json");
        env::set_var("OCICRYPT_KEYPROVIDER_CONFIG", test_conf_path);

        let pub_key_file = format!("{}/{}", path, "public_key.pem");
        let pub_key = fs::read(pub_key_file).unwrap();

        let priv_key_file = format!("{}/{}", path, "private_key.pem");
        let priv_key = fs::read(priv_key_file).unwrap();

        let mut ec = EncryptConfig::default();
        assert!(ec.encrypt_with_jwe(vec![pub_key.clone()]).is_ok());
        assert!(ec.encrypt_with_jwe(vec![pub_key]).is_ok());

        let mut dc = DecryptConfig::default();
        assert!(dc
            .decrypt_with_priv_keys(vec![priv_key.to_vec()], vec![vec![]])
            .is_ok());

        let layer_data: Vec<u8> = b"This is some text!".to_vec();
        let digest = format!("sha256:{:x}", Sha256::digest(&layer_data));

        let (layer_encryptor, mut elf) =
            encrypt_layer(&ec, layer_data.as_slice(), None, &digest).unwrap();

        let mut encrypted_data: Vec<u8> = Vec::new();
        let mut encryptor = layer_encryptor.unwrap();
        assert!(encryptor.read_to_end(&mut encrypted_data).is_ok());
        assert!(encryptor.finalized_lbco(&mut elf.lbco).is_ok());

        if let Ok(new_annotations) = elf.finalize_annotations(&ec, None, Some(&mut encryptor)) {
            let (layer_decryptor, dec_digest) = decrypt_layer(
                &dc,
                encrypted_data.as_slice(),
                Some(&new_annotations),
                false,
            )
            .unwrap();
            let mut plaintxt_data: Vec<u8> = Vec::new();
            let mut decryptor = layer_decryptor.unwrap();

            assert!(decryptor.read_to_end(&mut plaintxt_data).is_ok());
            assert_eq!(layer_data, plaintxt_data);
            assert_eq!(digest, dec_digest);
        }
    }

    #[cfg(feature = "async-io")]
    #[tokio::test]
    async fn test_async_decrypt_layer() {
        let path = load_data_path();
        let test_conf_path = format!("{}/{}", path, "ocicrypt_config.json");
        env::set_var("OCICRYPT_KEYPROVIDER_CONFIG", &test_conf_path);

        let pub_key_file = format!("{}/{}", path, "public_key.pem");
        let pub_key = fs::read(&pub_key_file).unwrap();

        let priv_key_file = format!("{}/{}", path, "private_key.pem");
        let priv_key = fs::read(&priv_key_file).unwrap();

        let mut ec = EncryptConfig::default();
        assert!(ec.encrypt_with_jwe(vec![pub_key]).is_ok());

        let mut dc = DecryptConfig::default();
        assert!(dc
            .decrypt_with_priv_keys(vec![priv_key.to_vec()], vec![vec![]])
            .is_ok());

        let layer_data: Vec<u8> = b"This is some text!".to_vec();
        let digest = format!("sha256:{:x}", Sha256::digest(&layer_data));

        let (layer_encryptor, mut elf) =
            encrypt_layer(&ec, layer_data.as_slice(), None, &digest).unwrap();

        let mut encrypted_data: Vec<u8> = Vec::new();
        let mut encryptor = layer_encryptor.unwrap();
        assert!(encryptor.read_to_end(&mut encrypted_data).is_ok());
        assert!(encryptor.finalized_lbco(&mut elf.lbco).is_ok());

        if let Ok(new_annotations) = elf.finalize_annotations(&ec, None, Some(&mut encryptor)) {
            let key_opts = decrypt_layer_key_opts_data(&dc, Some(&new_annotations)).unwrap();

            let (mut async_reader, dec_digest) =
                async_decrypt_layer(encrypted_data.as_slice(), Some(&new_annotations), &key_opts)
                    .unwrap();

            let mut plaintxt_data: Vec<u8> = Vec::new();
            tokio::io::AsyncReadExt::read_to_end(&mut async_reader, &mut plaintxt_data)
                .await
                .unwrap();

            assert_eq!(layer_data, plaintxt_data);
            assert_eq!(digest, dec_digest);
        }
    }

    fn load_data_path() -> String {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("data");

        path.to_str().unwrap().to_string()
    }
}
