// Copyright The ocicrypt Authors.
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

use anyhow::{anyhow, Result};
use serde::{de, Deserializer, Serialize, Serializer};

/// OCICRYPT_ENVVARNAME is the environment name for ocicrypt provider config file,
/// the key will be "OCICRYPT_KEYPROVIDER_CONFIG" and format is defined at:
/// <https://github.com/containers/ocicrypt/blob/main/docs/keyprovider.md>
pub const OCICRYPT_ENVVARNAME: &str = "OCICRYPT_KEYPROVIDER_CONFIG";

fn base64_enc(val: &[Vec<u8>]) -> Vec<String> {
    let mut res_vec = vec![];
    for x in val {
        res_vec.push(base64::encode_config(x, base64::STANDARD));
    }

    res_vec
}

fn base64_hashmap_s<S>(
    value: &HashMap<String, Vec<Vec<u8>>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut b64_encoded: HashMap<String, Vec<String>> = HashMap::default();
    for (key, value) in value {
        b64_encoded.insert(key.clone().to_string(), base64_enc(value));
    }
    b64_encoded.serialize(serializer)
}

fn base64_dec(val: &[String]) -> Result<Vec<Vec<u8>>, base64::DecodeError> {
    let mut res_vec = vec![];
    for x in val {
        res_vec.push(base64::decode_config(x, base64::STANDARD)?);
    }

    Ok(res_vec)
}

fn base64_hashmap_d<'de, D>(deserializer: D) -> Result<HashMap<String, Vec<Vec<u8>>>, D::Error>
where
    D: Deserializer<'de>,
{
    let b64_encoded: HashMap<String, Vec<String>> = serde::Deserialize::deserialize(deserializer)?;
    b64_encoded
        .iter()
        .map(|(k, v)| -> Result<(String, Vec<Vec<u8>>), D::Error> {
            Ok((k.clone(), base64_dec(v).map_err(de::Error::custom)?))
        })
        .collect()
}

/// Command describes the structure of command, it consist of path and args, where path defines
/// the location of binary executable and args are passed on to the binary executable
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Command {
    pub path: String,
    pub args: Option<Vec<String>>,
}

/// KeyProviderAttrs describes the structure of key provider, it defines the different ways of
/// invocation to key provider
#[derive(Deserialize, Debug, Clone)]
pub struct KeyProviderAttrs {
    pub cmd: Option<Command>,
    pub grpc: Option<String>,
    pub ttrpc: Option<String>,
    pub native: Option<String>,
}

/// DecryptConfig wraps the Parameters map that holds the decryption key
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DecryptConfig {
    /// map holding 'privkeys', 'x509s', 'gpg-privatekeys'
    #[serde(
        rename = "Parameters",
        serialize_with = "base64_hashmap_s",
        deserialize_with = "base64_hashmap_d"
    )]
    pub param: HashMap<String, Vec<Vec<u8>>>,
}

impl DecryptConfig {
    /// Update DecryptConfig param with key and value
    fn update_param(&mut self, key: &str, value: Vec<Vec<u8>>) -> Result<()> {
        if value.is_empty() {
            return Err(anyhow!("decrypt config: value of {} is None", key));
        }

        self.param
            .entry(key.to_string())
            .and_modify(|v| v.extend(value.iter().cloned()))
            .or_insert(value);

        Ok(())
    }

    /// Add DecryptConfig with configured private keys for decryption
    pub fn decrypt_with_priv_keys(
        &mut self,
        priv_keys: Vec<Vec<u8>>,
        priv_key_passwords: Vec<Vec<u8>>,
    ) -> Result<()> {
        if priv_keys.len() != priv_key_passwords.len() {
            return Err(anyhow!(
                "Length of privKeys should match with privKeysPasswords"
            ));
        }

        self.update_param("privkeys", priv_keys)?;
        self.update_param("privkeys-passwords", priv_key_passwords)?;

        Ok(())
    }

    /// Add DecryptConfig with configured x509 certs for decryption
    pub fn decrypt_with_x509s(&mut self, x509s: Vec<Vec<u8>>) -> Result<()> {
        self.update_param("x509s", x509s)?;

        Ok(())
    }

    /// Add DecryptConfig with configured gpg private keys for decryption
    pub fn decrypt_with_gpg(
        &mut self,
        gpg_priv_keys: Vec<Vec<u8>>,
        gpg_priv_pwds: Vec<Vec<u8>>,
    ) -> Result<()> {
        self.update_param("gpg-privatekeys", gpg_priv_keys)?;
        self.update_param("gpg-privatekeys-passwords", gpg_priv_pwds)?;

        Ok(())
    }

    /// Add DecryptConfig with configured pkcs11 config and YAML formatted keys for decryption
    pub fn decrypt_with_pkcs11(
        &mut self,
        pkcs11_config: Vec<Vec<u8>>,
        pkcs11_yaml: Vec<Vec<u8>>,
    ) -> Result<()> {
        self.update_param("pkcs11-config", pkcs11_config)?;
        self.update_param("pkcs11-yamls", pkcs11_yaml)?;

        Ok(())
    }

    /// Add DecryptConfig with configured key_providers for decryption
    pub fn decrypt_with_key_provider(&mut self, key_providers: Vec<Vec<u8>>) -> Result<()> {
        for val in key_providers.iter().map(|v| String::from_utf8_lossy(v)) {
            if let Some(index) = val.find(':') {
                let key: String = val.chars().take(index).collect();
                let value: String = val.chars().skip(index + 1).collect();

                self.update_param(&key, vec![value.as_bytes().to_vec()])?;
            } else {
                self.update_param(val.as_ref(), vec![b"Enabled".to_vec()])?;
            }
        }

        Ok(())
    }
}

/// EncryptConfig is the container image PGP encryption configuration holding
/// the identifiers of those that will be able to decrypt the container and
/// the PGP public keyring file data that contains their public keys.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EncryptConfig {
    /// map holding 'gpg-recipients', 'gpg-pubkeyringfile', 'pubkeys', 'x509s'
    #[serde(
        rename = "Parameters",
        serialize_with = "base64_hashmap_s",
        deserialize_with = "base64_hashmap_d"
    )]
    pub param: HashMap<String, Vec<Vec<u8>>>,

    /// Allow for adding wrapped keys to an encrypted layer
    #[serde(rename = "DecryptConfig")]
    pub decrypt_config: Option<DecryptConfig>,
}

impl EncryptConfig {
    /// Update EncryptConfig param with key and value
    fn update_param(&mut self, key: &str, value: Vec<Vec<u8>>) -> Result<()> {
        if value.is_empty() {
            return Err(anyhow!("encrypt config: value of {} is None", key));
        }

        self.param
            .entry(key.to_string())
            .and_modify(|v| v.extend(value.iter().cloned()))
            .or_insert(value);

        Ok(())
    }

    /// Add EncryptConfig with jwe public keys for encryption
    pub fn encrypt_with_jwe(&mut self, pubkeys: Vec<Vec<u8>>) -> Result<()> {
        self.update_param("pubkeys", pubkeys)
    }

    /// Add EncryptConfig with pkcs7 x509 certs for encryption
    pub fn encrypt_with_pkcs7(&mut self, x509s: Vec<Vec<u8>>) -> Result<()> {
        self.update_param("x509s", x509s)
    }

    /// Add EncryptConfig with configured gpg parameters for encryption
    pub fn encrypt_with_gpg(
        &mut self,
        gpg_recipients: Vec<Vec<u8>>,
        gpg_pub_ring_file: Vec<u8>,
    ) -> Result<()> {
        self.update_param("gpg-recipients", gpg_recipients)?;
        self.update_param("gpg-pubkeyringfile", vec![gpg_pub_ring_file])?;

        Ok(())
    }

    /// Add EncryptConfig with configured pkcs11 parameters for encryption
    pub fn encrypt_with_pkcs11(
        &mut self,
        pkcs11_config: Vec<Vec<u8>>,
        pkcs11_pubkeys: Vec<Vec<u8>>,
        pkcs11_yaml: Vec<Vec<u8>>,
    ) -> Result<()> {
        if !pkcs11_pubkeys.is_empty() {
            self.update_param("pkcs11-pubkeys", pkcs11_pubkeys)?;
        }

        if !pkcs11_yaml.is_empty() {
            self.update_param("pkcs11-config", pkcs11_config)?;
            self.update_param("pkcs11-yamls", pkcs11_yaml)?;
        }

        Ok(())
    }

    /// Add EncryptConfig with configured keyprovider parameters for encryption
    pub fn encrypt_with_key_provider(&mut self, key_providers: Vec<Vec<u8>>) -> Result<()> {
        for val in key_providers.iter().map(|v| String::from_utf8_lossy(v)) {
            if let Some(index) = val.find(':') {
                let key: String = val.chars().take(index).collect();
                let value: String = val.chars().skip(index + 1).collect();

                self.update_param(&key, vec![value.as_bytes().to_vec()])?;
            } else {
                self.update_param(val.as_ref(), vec![b"Enabled".to_vec()])?;
            }
        }

        Ok(())
    }
}

/// CryptoConfig is a common wrapper for EncryptConfig and DecrypConfig that can
/// be passed through functions that share much code for encryption and decryption
#[derive(Debug, Default, Clone)]
pub struct CryptoConfig {
    pub encrypt_config: Option<EncryptConfig>,
    pub decrypt_config: Option<DecryptConfig>,
}

/// OcicryptConfig represents the format of an ocicrypt_provider.conf config file.
/// Detail ocicrypt keyprovider protocol and config file format is defined at:
/// <https://github.com/containers/ocicrypt/blob/main/docs/keyprovider.md>
#[derive(Deserialize)]
pub struct OcicryptConfig {
    #[serde(rename = "key-providers")]
    pub key_providers: HashMap<String, KeyProviderAttrs>,
}

impl OcicryptConfig {
    fn from_file(filename: &str) -> Result<OcicryptConfig> {
        let file = File::open(filename)?;
        let reader = BufReader::new(file);

        serde_json::from_reader(reader)
            .map_err(|e| anyhow!("Error reading file {:?}", e.to_string()))
    }

    /// from_env tries to read the configuration file at the following locations
    /// ${OCICRYPT_KEYPROVIDER_CONFIG} == "/etc/ocicrypt_keyprovider.json"
    /// If no configuration file could be found or read a null pointer is returned
    pub fn from_env(env: &str) -> Result<Option<OcicryptConfig>> {
        // find file name from environment variable, ignore error if environment variable is not set.
        match std::env::var(env) {
            Err(_e) => Ok(None),
            Ok(filename) => OcicryptConfig::from_file(filename.as_str()).map(Some),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::path::PathBuf;

    #[test]
    fn test_decrypt_config() {
        let priv_keys1 = vec![b"priv_key1".to_vec()];
        let priv_keys2 = vec![b"priv_key2".to_vec()];
        let priv_keys3 = vec![b"priv_key3".to_vec(), b"priv_key4".to_vec()];
        let pkcs11_config = vec![b"pkcs11_config".to_vec()];
        let pkcs11_yaml = vec![b"pkcs11_yaml".to_vec()];

        let key_providers = vec![
            b"key_p1".to_vec(),
            b"key_p2:abc".to_vec(),
            b"key_p3:abc:abc".to_vec(),
            b"key_p3:def".to_vec(),
            b":abc".to_vec(),
            b":def".to_vec(),
        ];

        let mut dc = DecryptConfig::default();

        assert!(dc
            .decrypt_with_priv_keys(priv_keys1.clone(), priv_keys2.clone())
            .is_ok());
        assert!(dc
            .decrypt_with_priv_keys(priv_keys1.clone(), priv_keys3)
            .is_err());
        assert!(dc.decrypt_with_x509s(priv_keys1.clone()).is_ok());
        assert!(dc.decrypt_with_gpg(priv_keys1, priv_keys2).is_ok());
        assert!(dc.decrypt_with_pkcs11(pkcs11_config, pkcs11_yaml).is_ok());
        assert!(dc.decrypt_with_key_provider(key_providers).is_ok());

        assert_eq!(dc.param.get("privkeys-passwords").unwrap().len(), 1);
        assert_eq!(dc.param.get("privkeys").unwrap().len(), 1);
        assert_eq!(dc.param.get("pkcs11-config").unwrap().len(), 1);
        assert_eq!(dc.param.get("key_p1").unwrap().len(), 1);
        assert_eq!(dc.param.get("key_p2").unwrap().len(), 1);
        assert_eq!(dc.param.get("key_p3").unwrap().len(), 2);
        assert_eq!(dc.param.get("").unwrap().len(), 2);
        assert_eq!(dc.param.get("gpg-privatekeys").unwrap().len(), 1);
        assert_eq!(dc.param.get("gpg-privatekeys-passwords").unwrap().len(), 1);
        assert_eq!(dc.param.get("pkcs11-yamls").unwrap().len(), 1);

        println!("final decrypt config is: {dc:?}");
    }

    #[test]
    fn test_encrypt_config() {
        let pubkeys1 = vec![b"pubkey1".to_vec()];
        let pubkeys2 = vec![b"pubkey2".to_vec()];
        let gpg_recipients = vec![b"recip1".to_vec(), b"recip2".to_vec()];
        let gpg_pub_ring_file = b"gpg_pub_ring_file".to_vec();
        let pkcs11_config = vec![b"pkcs11_config".to_vec()];
        let pkcs11_pubkeys = vec![b"pkcs11_pubkeys".to_vec()];
        let pkcs11_yaml = vec![b"pkcs11_yaml".to_vec()];
        let key_providers = vec![
            b"key_p1".to_vec(),
            b"key_p2:abc".to_vec(),
            b"key_p3:abc:abc".to_vec(),
        ];

        let mut ec = EncryptConfig::default();

        assert!(ec.encrypt_with_jwe(vec![]).is_err());
        assert!(ec.encrypt_with_jwe(pubkeys1.clone()).is_ok());
        assert_eq!(pubkeys1, ec.param["pubkeys"]);

        assert!(ec.encrypt_with_jwe(pubkeys2.clone()).is_ok());
        assert_eq!(2, ec.param["pubkeys"].len());

        assert!(ec.encrypt_with_pkcs7(pubkeys2).is_ok());
        assert!(ec
            .encrypt_with_gpg(gpg_recipients.clone(), gpg_pub_ring_file.clone())
            .is_ok());
        assert_eq!(gpg_recipients, ec.param["gpg-recipients"]);
        assert_eq!(vec![gpg_pub_ring_file], ec.param["gpg-pubkeyringfile"]);

        assert!(ec
            .encrypt_with_pkcs11(pkcs11_config, pkcs11_pubkeys, pkcs11_yaml)
            .is_ok());
        assert!(ec.encrypt_with_key_provider(key_providers).is_ok());
        assert_eq!(vec![b"Enabled".to_vec()], ec.param["key_p1"]);
        assert_eq!(vec![b"abc".to_vec()], ec.param["key_p2"]);
        assert_eq!(vec![b"abc:abc".to_vec()], ec.param["key_p3"]);

        println!("final encrypt config is: {ec:?}");
    }

    #[test]
    fn test_crypto_config() {
        let dc = DecryptConfig::default();
        let ec = EncryptConfig::default();
        let mut cc = CryptoConfig::default();

        assert!(dc.param.is_empty());
        assert!(ec.param.is_empty());
        assert!(ec.decrypt_config.is_none());
        assert!(cc.encrypt_config.is_none());
        assert!(cc.decrypt_config.is_none());
        cc.encrypt_config = Some(ec);
        cc.decrypt_config = Some(dc);
        assert!(cc.encrypt_config.is_some());
        assert!(cc.decrypt_config.is_some());
    }

    #[test]
    fn test_ocicrypt_config() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("data");
        let test_conf_path = format!("{}/{}", path.to_str().unwrap(), "ocicrypt_config.json");
        env::set_var("OCICRYPT_KEYPROVIDER_CONFIG", test_conf_path);

        let mut provider = HashMap::new();
        let args: Vec<String> = Vec::default();
        let attrs = KeyProviderAttrs {
            cmd: Some(Command {
                path: "/usr/lib/keyprovider-wrapkey".to_string(),
                args: Some(args),
            }),
            grpc: None,
            ttrpc: None,
            native: None,
        };
        provider.insert(String::from("keyprovider1"), attrs);

        let provider_unmarshalled = OcicryptConfig::from_env(OCICRYPT_ENVVARNAME)
            .expect("Unable to read ocicrypt config file")
            .unwrap();
        let p1 = provider_unmarshalled
            .key_providers
            .get("keyprovider1")
            .unwrap();
        let cmd = p1.cmd.as_ref().unwrap();
        assert_eq!(
            cmd.path,
            provider
                .get("keyprovider1")
                .unwrap()
                .cmd
                .as_ref()
                .unwrap()
                .path
        );
        assert_eq!(cmd.args.as_ref().unwrap().len(), 0);
        assert!(p1.grpc.is_none());
        assert!(p1.ttrpc.is_none());
        assert!(p1.native.is_none());

        let p2 = provider_unmarshalled
            .key_providers
            .get("keyprovider2")
            .unwrap();
        assert!(p2.cmd.is_none());
        assert!(p2.grpc.is_some());
        assert!(p2.ttrpc.is_none());
        assert!(p2.native.is_none());

        let p3 = provider_unmarshalled
            .key_providers
            .get("keyprovider3")
            .unwrap();
        assert!(p3.cmd.is_none());
        assert!(p3.grpc.is_none());
        assert!(p3.ttrpc.is_none());
        assert!(p3.native.is_some());

        let p4 = provider_unmarshalled
            .key_providers
            .get("keyprovider4")
            .unwrap();
        assert!(p4.cmd.is_none());
        assert!(p4.grpc.is_none());
        assert!(p4.ttrpc.is_some());
        assert!(p4.native.is_none());
    }
}
