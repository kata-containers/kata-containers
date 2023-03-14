// Copyright The ocicrypt Authors.
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use josekit::jwe::{
    deserialize_json, serialize_general_json, JweDecrypter, JweEncrypter, JweHeader, JweHeaderSet,
    ECDH_ES_A256KW, RSA_OAEP,
};
use josekit::jwk::{Jwk, KeyAlg, KeyFormat, KeyInfo};

use crate::config::{DecryptConfig, EncryptConfig};
use crate::keywrap::KeyWrapper;

/// A Jwe keywrapper
#[derive(Debug)]
pub struct JweKeyWrapper {}

// Get the encrypter from public key
fn encrypter(pubkey: &[u8]) -> Result<Box<dyn JweEncrypter>> {
    let key_info =
        KeyInfo::detect(&pubkey).ok_or_else(|| anyhow!("jwe: failed to detect public key info"))?;
    if !key_info.is_public_key() {
        return Err(anyhow!("jwe: expect public key, found private key"));
    }

    match key_info.alg() {
        Some(KeyAlg::Ec { curve: Some(_) }) => {
            let ecdh_encrypter = match key_info.format() {
                KeyFormat::Der { raw: _ } => ECDH_ES_A256KW.encrypter_from_der(pubkey)?,
                KeyFormat::Pem { traditional: _ } => ECDH_ES_A256KW.encrypter_from_pem(pubkey)?,
                KeyFormat::Jwk => ECDH_ES_A256KW.encrypter_from_jwk(&Jwk::from_bytes(pubkey)?)?,
            };

            Ok(ecdh_encrypter.box_clone())
        }
        _ => {
            let rsa_encrypter = match key_info.format() {
                KeyFormat::Der { raw: _ } => RSA_OAEP.encrypter_from_der(pubkey)?,
                KeyFormat::Pem { traditional: _ } => RSA_OAEP.encrypter_from_pem(pubkey)?,
                KeyFormat::Jwk => RSA_OAEP.encrypter_from_jwk(&Jwk::from_bytes(pubkey)?)?,
            };

            Ok(rsa_encrypter.box_clone())
        }
    }
}

// Get the decrypter from private key
fn decrypter(priv_key: &[u8]) -> Result<Box<dyn JweDecrypter>> {
    let key_info = KeyInfo::detect(&priv_key)
        .ok_or_else(|| anyhow!("jwe: failed to detect private key info"))?;
    if key_info.is_public_key() {
        return Err(anyhow!("jwe: expect private key, found public key"));
    }

    match key_info.alg() {
        Some(KeyAlg::Ec { curve: Some(_) }) => {
            let ecdh_decrypter = match key_info.format() {
                KeyFormat::Der { raw: _ } => ECDH_ES_A256KW.decrypter_from_der(priv_key)?,
                KeyFormat::Pem { traditional: _ } => ECDH_ES_A256KW.decrypter_from_pem(priv_key)?,
                KeyFormat::Jwk => ECDH_ES_A256KW.decrypter_from_jwk(&Jwk::from_bytes(priv_key)?)?,
            };

            Ok(ecdh_decrypter.box_clone())
        }
        _ => {
            let rsa_decrypter = match key_info.format() {
                KeyFormat::Der { raw: _ } => RSA_OAEP.decrypter_from_der(priv_key)?,
                KeyFormat::Pem { traditional: _ } => RSA_OAEP.decrypter_from_pem(priv_key)?,
                KeyFormat::Jwk => RSA_OAEP.decrypter_from_jwk(&Jwk::from_bytes(priv_key)?)?,
            };

            Ok(rsa_decrypter.box_clone())
        }
    }
}

impl KeyWrapper for JweKeyWrapper {
    fn wrap_keys(&self, ec: &EncryptConfig, opts_data: &[u8]) -> Result<Vec<u8>> {
        let pubkeys = ec
            .param
            .get("pubkeys")
            .ok_or_else(|| anyhow!("jwe: invalid configuration for keywrap"))?;
        let mut encrypters: Vec<Box<dyn JweEncrypter>> = Vec::with_capacity(pubkeys.len());
        for pubkey in pubkeys {
            let encrypter = encrypter(pubkey)?;
            encrypters.push(encrypter);
        }

        let src_rheader = JweHeader::new();
        let mut src_header = JweHeaderSet::new();
        src_header.set_content_encryption("A256GCM", true);

        let recipients: Vec<(Option<&JweHeader>, &dyn JweEncrypter)> = encrypters
            .iter()
            .map(|x| (Some(&src_rheader), &**x))
            .collect();
        let json = serialize_general_json(opts_data, Some(&src_header), &recipients, None)?;

        Ok(json.as_bytes().to_vec())
    }

    fn unwrap_keys(&self, dc: &DecryptConfig, jwe_string: &[u8]) -> Result<Vec<u8>> {
        let data = std::str::from_utf8(jwe_string)
            .map_err(|_e| anyhow!("jwe: invalid data to unwrap_keys()"))?;
        let privkeys = self
            .private_keys(&dc.param)
            .ok_or_else(|| anyhow!("jwe: invalid configuration for keyunwrap"))?;
        for privkey in privkeys {
            let decrypter = decrypter(&privkey)?;
            if let Ok((keys, _)) = deserialize_json(data, &*decrypter) {
                return Ok(keys);
            }
        }

        Err(anyhow!("jwe: No suitable private key found for decryption"))
    }

    fn annotation_id(&self) -> String {
        "org.opencontainers.image.enc.keys.jwe".to_string()
    }

    fn probe(&self, dc_param: &HashMap<String, Vec<Vec<u8>>>) -> bool {
        dc_param.get("privkeys").is_some()
    }

    fn private_keys(&self, dc_param: &HashMap<String, Vec<Vec<u8>>>) -> Option<Vec<Vec<u8>>> {
        dc_param.get("privkeys").cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn test_keywrap_jwe() {
        let path = load_data_path();
        let path = path.display();
        let pub_key_files = vec![
            format!("{}/{}", path, "public_key.pem"),
            format!("{}/{}", path, "public_key_ec.der"),
            format!("{}/{}", path, "RSA_public.jwk"),
        ];

        let mut ec = EncryptConfig::default();
        let mut dc = DecryptConfig::default();
        let payload = b"test".to_vec();

        let jwe_key_wrapper = JweKeyWrapper {};

        assert!(!jwe_key_wrapper.probe(&dc.param));
        assert!(jwe_key_wrapper.private_keys(&dc.param).is_none());

        let mut pubkeys = vec![];
        for key_file in pub_key_files.iter() {
            let contents = fs::read(key_file).unwrap();
            pubkeys.push(contents);
        }

        assert!(ec.encrypt_with_jwe(pubkeys).is_ok());
        assert!(jwe_key_wrapper.wrap_keys(&ec, &payload).is_ok());

        let json = jwe_key_wrapper.wrap_keys(&ec, &payload).unwrap();

        let priv_key_files = vec![
            format!("{}/{}", path, "private_key.pem"),
            format!("{}/{}", path, "private_key.der"),
            format!("{}/{}", path, "private_key8.pem"),
            format!("{}/{}", path, "private_key8.der"),
            format!("{}/{}", path, "private_key_ec.der"),
            format!("{}/{}", path, "RSA_private.jwk"),
        ];

        let mut privkeys = vec![];
        let mut privkey_passwords: Vec<Vec<u8>> = vec![];
        for key_file in priv_key_files.iter() {
            let contents = fs::read(key_file).unwrap();
            privkeys.push(contents);
            privkey_passwords.push(vec![]);
        }

        assert!(dc
            .decrypt_with_priv_keys(privkeys, privkey_passwords)
            .is_ok());

        assert!(jwe_key_wrapper.probe(&dc.param));
        assert!(jwe_key_wrapper.private_keys(&dc.param).is_some());
        assert_eq!(jwe_key_wrapper.unwrap_keys(&dc, &json).unwrap(), payload);

        assert_eq!(
            jwe_key_wrapper.annotation_id(),
            "org.opencontainers.image.enc.keys.jwe".to_string()
        );

        assert!(jwe_key_wrapper
            .keyids_from_packet("packet".to_string())
            .is_none());
        assert!(jwe_key_wrapper
            .recipients("recipients".to_string())
            .is_none());
    }

    fn load_data_path() -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("data");
        path
    }
}
