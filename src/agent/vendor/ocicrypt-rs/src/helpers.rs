// Copyright The ocicrypt Authors.
// SPDX-License-Identifier: Apache-2.0

use std::fs;
use std::fs::File;
use std::io::Read;
use std::os::unix::io::FromRawFd;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config::{CryptoConfig, DecryptConfig, EncryptConfig};

// process_recipient_keys sorts the array of recipients by type.
// Recipients may be either: x509 certificates, public keys,
// or PGP public keys identified by email address or name
fn process_recipient_keys(recipients: Vec<String>) -> Result<[Vec<Vec<u8>>; 6]> {
    let mut gpg_recipients = vec![];
    let mut pubkeys = vec![];
    let mut x509s = vec![];
    let mut pkcs11_pubkeys = vec![];
    let mut pkcs11_yamls = vec![];
    let mut key_providers = vec![];

    for recipient in recipients {
        if let Some(index) = recipient.find(':') {
            let protocol: String = recipient.chars().take(index).collect();
            let value: String = recipient.chars().skip(index + 1).collect();

            match &protocol[..] {
                "pgp" => gpg_recipients.push(value.as_bytes().to_vec()),
                "jwe" => {
                    let contents = fs::read(&value)?;
                    // TODO: Check valid public key
                    pubkeys.push(contents);
                }
                "pkcs7" => {
                    let contents = fs::read(&value)?;
                    // TODO: Check valid certificate
                    x509s.push(contents);
                }
                "pkcs11" => {
                    let contents = fs::read(&value)?;
                    // TODO: Check valid pkcs11 public key or normal public key
                    pkcs11_yamls.push(contents.clone());
                    pkcs11_pubkeys.push(contents);
                    /*
                    if true {
                        pkcs11_yamls.push(contents);
                    } else if true {
                        pkcs11_pubkeys.push(contents);
                    } else {
                        return Err(anyhow!("Provided file is not a public key"));
                    }
                    */
                }
                "provider" => key_providers.push(value.as_bytes().to_vec()),
                _ => return Err(anyhow!("Provided protocol not recognized")),
            };
        } else {
            return Err(anyhow!("Invalid recipient format"));
        }
    }

    Ok([
        gpg_recipients,
        pubkeys,
        x509s,
        pkcs11_pubkeys,
        pkcs11_yamls,
        key_providers,
    ])
}

// process_x509_certs processes x509 certificate files
fn process_x509_certs(keys: &[String]) -> Result<Vec<Vec<u8>>> {
    let mut x509s = vec![];

    for key in keys {
        if key.contains(':') {
            continue;
        }
        if !Path::new(&key).exists() {
            continue;
        }
        let contents = fs::read(key)?;
        // TODO: Check valid certificate

        x509s.push(contents);
    }

    Ok(x509s)
}

// process_pwd_string process a password that may be in any of the following formats:
// - file=<passwordfile>
// - pass=<password>
// - fd=<filedescriptor>
// - <password>
fn process_pwd_string(pwd_string: &str) -> Result<Vec<u8>> {
    if let Some(pwd) = pwd_string.strip_prefix("file=") {
        let contents = fs::read(pwd)?;
        return Ok(contents);
    } else if let Some(pwd) = pwd_string.strip_prefix("pass=") {
        return Ok(pwd.as_bytes().to_vec());
    } else if let Some(pwd) = pwd_string.strip_prefix("fd=") {
        // TODO: it's a little dangerous
        let fd = pwd.parse::<i32>().unwrap();
        let mut fd_file = unsafe { File::from_raw_fd(fd) };
        let mut contents = String::new();
        fd_file.read_to_string(&mut contents)?;
        return Ok(contents.as_bytes().to_vec());
    }

    Ok(pwd_string.as_bytes().to_vec())
}

// process_private_keyfiles sorts the different types of private key files;
// private key files may either be private keys or GPG private key ring files.
// The private key files may include the password for the private key and
// take any of the following forms:
// - <filename>
// - <filename>:file=<passwordfile>
// - <filename>:pass=<password>
// - <filename>:fd=<filedescriptor>
// - <filename>:<password>
// - provider:<...>
fn process_private_keyfiles(keyfiles_and_pwds: &[String]) -> Result<[Vec<Vec<u8>>; 6]> {
    let mut gpg_secret_key_ring_files = vec![];
    let mut gpg_secret_key_passwords = vec![];
    let mut priv_keys = vec![];
    let mut priv_keys_passwords = vec![];
    let mut pkcs11_yamls = vec![];
    let mut key_providers = vec![];

    for keyfile_and_pwd in keyfiles_and_pwds {
        // treat "provider" protocol separately
        if let Some(provider) = keyfile_and_pwd.strip_prefix("provider:") {
            key_providers.push(provider.as_bytes().to_vec());
            continue;
        }

        let (content, password) = match keyfile_and_pwd.split_once(':') {
            Some((file, pass)) => {
                let contents = fs::read(file)?;
                let password = process_pwd_string(pass)?;
                (contents, password)
            }
            None => {
                let contents = fs::read(keyfile_and_pwd)?;
                (contents, Vec::new())
            }
        };

        // TODO: Check valid pkcs11 public key or normal public key
        pkcs11_yamls.push(content.clone());
        priv_keys.push(content.clone());
        priv_keys_passwords.push(password.clone());
        gpg_secret_key_ring_files.push(content);
        gpg_secret_key_passwords.push(password);
        /*
        if true {
            pkcs11_yamls.push(contents);
        } else if false {
            priv_keys.push(contents);
            priv_keys_passwords.push(password);
        } else if false {
            gpg_secret_key_ring_files.push(contents);
            gpg_secret_key_passwords.push(password);
        } else {
            // ignore if file is not recognized, so as not to error if additional
            // metadata/cert files exists
            continue;
        }
        */
    }

    Ok([
        gpg_secret_key_ring_files,
        gpg_secret_key_passwords,
        priv_keys,
        priv_keys_passwords,
        pkcs11_yamls,
        key_providers,
    ])
}

/// Create the CryptoConfig object that contains the necessary information to perform decryption.
///
/// # Arguments
/// * `keys` - decryption key info in following format:\
///           - \<filename> \
///           - \<filename>:file=\<passwordfile> \
///           - \<filename>:pass=\<password> \
///           - \<filename>:fd=\<filedescriptor> \
///           - \<filename>:\<password> \
///           - provider:<cmd/gprc/native>
/// * `dec_recipients` - contains x509 cert for PKCS7 decryption.
pub fn create_decrypt_config(
    keys: Vec<String>,
    dec_recipients: Vec<String>,
) -> Result<CryptoConfig> {
    let mut dc = DecryptConfig::default();
    let mut cc = CryptoConfig::default();

    let [_, _, mut x509s, _, _, _] = process_recipient_keys(dec_recipients)?;

    // x509 certs can also be passed in via keys
    let x509_from_keys = process_x509_certs(&keys)?;
    x509s.extend(x509_from_keys);

    let [gpg_secret_key_ring_files, gpg_secret_key_passwords, priv_keys, priv_keys_passwords, pkcs11_yamls, key_providers] =
        process_private_keyfiles(&keys)?;

    if !gpg_secret_key_ring_files.is_empty() {
        dc.decrypt_with_gpg(gpg_secret_key_ring_files, gpg_secret_key_passwords)?;
    }

    if !x509s.is_empty() {
        dc.decrypt_with_x509s(x509s)?;
    }

    if !priv_keys.is_empty() {
        dc.decrypt_with_priv_keys(priv_keys, priv_keys_passwords)?;
    }

    if !pkcs11_yamls.is_empty() {
        // TODO: Get pkcs11_config from the config file
        let pkcs11_config: Vec<Vec<u8>> = vec![vec![]];
        dc.decrypt_with_pkcs11(pkcs11_config, pkcs11_yamls)?;
    }

    if !key_providers.is_empty() {
        dc.decrypt_with_key_provider(key_providers)?;
    }

    cc.decrypt_config = Some(dc);
    Ok(cc)
}

/// Create the CryptoConfig object from the list of recipient strings and
/// list of key paths of private keys to perform encryption.
///
/// # Arguments
/// * `recipients` - encryption key info in format protocol:value.\
///           - jwe:\<keyfile> \
///           - pkcs7:\<keyfile> \
///           - pkcs11:\<keyfile> \
///           - pgp: \<address> \
///           - provider:<cmd/grpc/native>
/// * `keys` - private keys potential needs for encryption.
pub fn create_encrypt_config(recipients: Vec<String>, keys: Vec<String>) -> Result<CryptoConfig> {
    let mut ec = EncryptConfig::default();
    let mut cc = CryptoConfig::default();

    if !keys.is_empty() {
        let dc = create_decrypt_config(keys, vec![])?;
        ec.decrypt_config = dc.decrypt_config;
    }

    if !recipients.is_empty() {
        let [gpg_recipients, pubkeys, x509s, pkcs11_pubkeys, pkcs11_yamls, key_providers] =
            process_recipient_keys(recipients)?;

        // Create GPG client with guessed GPG version and default homedir
        if !gpg_recipients.is_empty() {
            // TODO: Check GPG installed and read GPG pub ring file
            ec.encrypt_with_gpg(gpg_recipients, vec![])?;
        }

        // Create Encryption Crypto Config
        if !x509s.is_empty() {
            ec.encrypt_with_pkcs7(x509s)?;
        }

        if !pubkeys.is_empty() {
            ec.encrypt_with_jwe(pubkeys)?;
        }

        if !pkcs11_pubkeys.is_empty() || !pkcs11_yamls.is_empty() {
            // TODO: Get pkcs11_config from the config file
            let pkcs11_config: Vec<Vec<u8>> = vec![vec![]];

            ec.encrypt_with_pkcs11(pkcs11_config, pkcs11_pubkeys, pkcs11_yamls)?;
        }

        if !key_providers.is_empty() {
            ec.encrypt_with_key_provider(key_providers)?;
        }
    }

    cc.encrypt_config = Some(ec);
    Ok(cc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_process_recipient_keys() {
        let path = load_data_path();
        let invalid_recipients1 = vec!["xxx:filename".to_string()];
        let invalid_recipients2 = vec!["jwe".to_string()];

        let jwe_recipient = format!("jwe:{}/{}", path, "public_key.pem");
        let pkcs7_recipient = format!("pkcs7:{}/{}", path, "public_certificate.pem");
        let pkcs11_recipient = format!("pkcs11:{}/{}", path, "public_key.pem");

        let valid_recipients = vec![
            "pgp:testkey@key.org".to_string(),
            jwe_recipient,
            pkcs7_recipient,
            pkcs11_recipient,
            "provider:cmd/grpc".to_string(),
        ];

        assert!(process_recipient_keys(invalid_recipients1).is_err());
        assert!(process_recipient_keys(invalid_recipients2).is_err());

        let [gpg_recipients, pubkeys, x509s, pkcs11_pubkeys, pkcs11_yamls, key_providers] =
            process_recipient_keys(valid_recipients).unwrap();
        assert_eq!(gpg_recipients.len(), 1);
        assert_eq!(pubkeys.len(), 1);
        assert_eq!(x509s.len(), 1);
        assert_eq!(pkcs11_pubkeys.len(), 1);
        assert_eq!(pkcs11_yamls.len(), 1);
        assert_eq!(key_providers.len(), 1);
    }

    #[test]
    fn test_process_x509_certs() {
        let path = load_data_path();
        let cert_keys = format!("{}/{}", path, "private_key.pem");

        assert!(process_x509_certs(&[cert_keys]).is_ok());
    }

    #[test]
    fn test_process_pwd_string() {
        let password: Vec<u8> = b"123456".to_vec();
        let path = load_data_path();

        let pwd_file = format!("file={}/{}", path, "passwordfile");
        assert_eq!(process_pwd_string(&pwd_file).unwrap(), password);

        let mut pwd_string = "pass=123456".to_string();
        assert_eq!(process_pwd_string(&pwd_string).unwrap(), password);

        pwd_string = "123456".to_string();
        assert_eq!(process_pwd_string(&pwd_string).unwrap(), password);
    }

    #[test]
    fn test_process_private_keyfiles() {
        let path = load_data_path();

        let private_keys = format!("{}/{}", path, "private_key.pem");
        let pwd_file = format!("file={}/{}", path, "passwordfile");
        let keyfiles_and_pwds = format!("{private_keys}:{pwd_file}");

        let [_, _, priv_keys, priv_keys_passwords, _, key_providers] =
            process_private_keyfiles(&[private_keys]).unwrap();
        assert_eq!(priv_keys.len(), 1);
        assert_eq!(priv_keys_passwords.len(), 1);
        assert!(priv_keys_passwords[0].is_empty());
        assert_eq!(key_providers.len(), 0);

        let [_, _, priv_keys, priv_keys_passwords, _, key_providers] =
            process_private_keyfiles(&[keyfiles_and_pwds]).unwrap();
        assert_eq!(priv_keys.len(), 1);
        assert_eq!(priv_keys_passwords.len(), 1);
        assert!(!priv_keys_passwords[0].is_empty());
        assert_eq!(key_providers.len(), 0);

        let [_, _, _, _, _, key_providers] =
            process_private_keyfiles(&["provider:cmd/grpc".to_string()]).unwrap();
        assert_eq!(key_providers.len(), 1);
    }

    #[test]
    fn test_create_decrypt_config() {
        let path = load_data_path();
        let private_keys = format!("{}/{}", path, "private_key.pem");
        let jwe_dec_recipient = format!("jwe:{}/{}", path, "private_key.pem");
        let pkcs7_dec_recipient = format!("pkcs7:{}/{}", path, "public_certificate.pem");

        let cc = create_decrypt_config(vec![private_keys.clone()], vec![]).unwrap();
        assert!(cc.encrypt_config.is_none());
        let dc = cc.decrypt_config.as_ref().unwrap();
        assert_eq!(dc.param.get("privkeys").unwrap().len(), 1);

        let cc = create_decrypt_config(vec![], vec![jwe_dec_recipient]).unwrap();
        assert!(cc.encrypt_config.is_none());
        let dc = cc.decrypt_config.as_ref().unwrap();
        assert!(dc.param.is_empty());

        let cc = create_decrypt_config(vec![], vec![pkcs7_dec_recipient.clone()]).unwrap();
        assert!(cc.encrypt_config.is_none());
        let dc = cc.decrypt_config.as_ref().unwrap();
        assert_eq!(dc.param.get("x509s").unwrap().len(), 1);

        let cc = create_decrypt_config(vec![private_keys], vec![pkcs7_dec_recipient]).unwrap();
        assert!(cc.encrypt_config.is_none());
        let dc = cc.decrypt_config.as_ref().unwrap();
        assert_eq!(dc.param.get("privkeys").unwrap().len(), 1);
        assert_eq!(dc.param.get("x509s").unwrap().len(), 2);
    }

    #[test]
    fn test_create_encrypt_config() {
        let path = load_data_path();

        let private_keys = format!("{}/{}", path, "private_key.pem");
        let jwe_recipient = format!("jwe:{}/{}", path, "public_key.pem");
        let pgp_recipient = "pgp:testkey@key.org".to_string();
        let pkcs7_recipient = format!("pkcs7:{}/{}", path, "public_certificate.pem");

        let cc = create_encrypt_config(vec![jwe_recipient.clone()], vec![]).unwrap();
        assert!(cc.decrypt_config.is_none());
        let ec = cc.encrypt_config.as_ref().unwrap();
        assert_eq!(ec.param.get("pubkeys").unwrap().len(), 1);

        let cc = create_encrypt_config(vec![jwe_recipient], vec![private_keys]).unwrap();
        assert!(cc.decrypt_config.is_none());
        let ec = cc.encrypt_config.as_ref().unwrap();
        assert_eq!(ec.param.get("pubkeys").unwrap().len(), 1);
        let dc = ec.decrypt_config.as_ref().unwrap();
        assert_eq!(dc.param.get("privkeys").unwrap().len(), 1);

        assert!(create_encrypt_config(vec![pgp_recipient], vec![]).is_ok());
        assert!(create_encrypt_config(vec![pkcs7_recipient], vec![]).is_ok());
    }

    fn load_data_path() -> String {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("data");

        path.to_str().unwrap().to_string()
    }
}
