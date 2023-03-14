use crate::hash::HashAlgorithm;
use crate::key::KeyError;
use crate::signature::{SignatureAlgorithm, SignatureError};
use crate::ssh::decode::SshComplexTypeDecode;
use crate::ssh::encode::{SshComplexTypeEncode, SshWriteExt};
use crate::ssh::private_key::{SshBasePrivateKey, SshPrivateKey, SshPrivateKeyError};
use crate::ssh::public_key::{SshBasePublicKey, SshPublicKey, SshPublicKeyError};
use byteorder::{BigEndian, WriteBytesExt};
use rand::Rng;
use rsa::{PublicKeyParts, RsaPublicKey};
use serde::Deserialize;
use std::cell::RefCell;
use std::convert::TryFrom;
use std::ops::DerefMut;
use std::str::FromStr;
use std::{io, string};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SshCertificateError {
    #[error("Can not process the certificate: {0:?}")]
    CertificateProcessingError(#[from] std::io::Error),
    #[error("Unsupported certificate type: {0}")]
    UnsupportedCertificateType(String),
    #[error(transparent)]
    SshCriticalOptionError(#[from] SshCriticalOptionError),
    #[error(transparent)]
    SshExtensionError(#[from] SshExtensionError),
    #[error("Can not parse. Expected UTF-8 valid text: {0:?}")]
    FromUtf8Error(#[from] string::FromUtf8Error),
    #[error("Invalid base64 string: {0:?}")]
    Base64DecodeError(#[from] base64::DecodeError),
    #[error(transparent)]
    InvalidCertificateType(#[from] SshCertTypeError),
    #[error("Invalid certificate key type: {0}")]
    InvalidCertificateKeyType(String),
    #[error("Certificate had invalid public key: {0:?}")]
    InvalidPublicKey(#[from] SshPublicKeyError),
    #[error(transparent)]
    RsaError(#[from] rsa::errors::Error),
    #[error(transparent)]
    KeyError(#[from] KeyError),
    #[error(transparent)]
    SshSignatureError(#[from] SshSignatureError),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize)]
pub enum SshCertType {
    Client,
    Host,
}

#[derive(Error, Debug)]
pub enum SshCertTypeError {
    #[error("Invalid certificate type. Expected 1(Client) or 2(Host) but got: {0}")]
    InvalidCertificateType(u32),
    #[error(transparent)]
    IoError(#[from] io::Error),
}

impl TryFrom<u32> for SshCertType {
    type Error = SshCertTypeError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(SshCertType::Client),
            2 => Ok(SshCertType::Host),
            x => Err(SshCertTypeError::InvalidCertificateType(x)),
        }
    }
}

impl From<SshCertType> for u32 {
    fn from(val: SshCertType) -> u32 {
        match val {
            SshCertType::Client => 1,
            SshCertType::Host => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SshCertKeyType {
    SshRsaV01,
    SshDssV01,
    RsaSha2_256V01,
    RsaSha2_512v01,
    EcdsaSha2Nistp256V01,
    EcdsaSha2Nistp384V01,
    EcdsaSha2Nistp521V01,
    SshEd25519V01,
}

impl SshCertKeyType {
    pub fn as_str(&self) -> &str {
        match self {
            SshCertKeyType::SshRsaV01 => "ssh-rsa-cert-v01@openssh.com",
            SshCertKeyType::SshDssV01 => "ssh-dss-cert-v01@openssh.com",
            SshCertKeyType::RsaSha2_256V01 => "rsa-sha2-256-cert-v01@openssh.com",
            SshCertKeyType::RsaSha2_512v01 => "rsa-sha2-512-cert-v01@openssh.com",
            SshCertKeyType::EcdsaSha2Nistp256V01 => "ecdsa-sha2-nistp256-cert-v01@openssh.com",
            SshCertKeyType::EcdsaSha2Nistp384V01 => "ecdsa-sha2-nistp384-cert-v01@openssh.com",
            SshCertKeyType::EcdsaSha2Nistp521V01 => "ecdsa-sha2-nistp521-cert-v01@openssh.com",
            SshCertKeyType::SshEd25519V01 => "ssh-ed25519-cert-v01@openssh.com",
        }
    }
}

impl TryFrom<String> for SshCertKeyType {
    type Error = SshCertificateError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "ssh-rsa-cert-v01@openssh.com" => Ok(SshCertKeyType::SshRsaV01),
            "ssh-dss-cert-v01@openssh.com" => Ok(SshCertKeyType::SshDssV01),
            "rsa-sha2-256-cert-v01@openssh.com" => Ok(SshCertKeyType::RsaSha2_256V01),
            "rsa-sha2-512-cert-v01@openssh.com" => Ok(SshCertKeyType::RsaSha2_512v01),
            "ecdsa-sha2-nistp256-cert-v01@openssh.com" => Ok(SshCertKeyType::EcdsaSha2Nistp256V01),
            "ecdsa-sha2-nistp384-cert-v01@openssh.com" => Ok(SshCertKeyType::EcdsaSha2Nistp384V01),
            "ecdsa-sha2-nistp521-cert-v01@openssh.com" => Ok(SshCertKeyType::EcdsaSha2Nistp521V01),
            "ssh-ed25519-cert-v01@openssh.com" => Ok(SshCertKeyType::SshEd25519V01),
            _ => Err(SshCertificateError::InvalidCertificateKeyType(value)),
        }
    }
}

#[derive(Error, Debug)]
pub enum SshCriticalOptionError {
    #[error("Unsupported critical option type: {0}")]
    UnsupportedCriticalOptionType(String),
    #[error(transparent)]
    IoError(#[from] io::Error),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum SshCriticalOptionType {
    ForceCommand,
    SourceAddress,
    VerifyRequired,
}

impl SshCriticalOptionType {
    pub fn as_str(&self) -> &str {
        match self {
            SshCriticalOptionType::ForceCommand => "force-command",
            SshCriticalOptionType::SourceAddress => "source-address",
            SshCriticalOptionType::VerifyRequired => "verify-required",
        }
    }
}

impl TryFrom<String> for SshCriticalOptionType {
    type Error = SshCriticalOptionError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "force-command" => Ok(SshCriticalOptionType::ForceCommand),
            "source-address" => Ok(SshCriticalOptionType::SourceAddress),
            "verify-required" => Ok(SshCriticalOptionType::VerifyRequired),
            _ => Err(SshCriticalOptionError::UnsupportedCriticalOptionType(value)),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SshCriticalOption {
    pub option_type: SshCriticalOptionType,
    pub data: String,
}

#[derive(Error, Debug)]
pub enum SshExtensionError {
    #[error("Unsupported extension type: {0}")]
    UnsupportedExtensionType(String),
    #[error(transparent)]
    IoError(#[from] io::Error),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SshExtensionType {
    NoTouchRequired,
    PermitX11Forwarding,
    PermitAgentForwarding,
    PermitPortForwarding,
    PermitPty,
    PermitUserPc,
}

impl SshExtensionType {
    pub fn as_str(&self) -> &str {
        match self {
            SshExtensionType::NoTouchRequired => "no-touch-required",
            SshExtensionType::PermitUserPc => "permit-user-rc",
            SshExtensionType::PermitPty => "permit-pty",
            SshExtensionType::PermitAgentForwarding => "permit-agent-forwarding",
            SshExtensionType::PermitPortForwarding => "permit-port-forwarding",
            SshExtensionType::PermitX11Forwarding => "permit-X11-forwarding",
        }
    }
}

impl TryFrom<String> for SshExtensionType {
    type Error = SshExtensionError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "no-touch-required" => Ok(SshExtensionType::NoTouchRequired),
            "permit-X11-forwarding" => Ok(SshExtensionType::PermitX11Forwarding),
            "permit-agent-forwarding" => Ok(SshExtensionType::PermitAgentForwarding),
            "permit-port-forwarding" => Ok(SshExtensionType::PermitPortForwarding),
            "permit-pty" => Ok(SshExtensionType::PermitPty),
            "permit-user-rc" => Ok(SshExtensionType::PermitUserPc),
            _ => Err(SshExtensionError::UnsupportedExtensionType(value)),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SshExtension {
    pub extension_type: SshExtensionType,
    pub data: String,
}

impl SshExtension {
    pub fn new(extension_type: SshExtensionType, data: String) -> Self {
        Self { extension_type, data }
    }
}

#[derive(Error, Debug)]
pub enum SshSignatureError {
    #[error("unsupported signature format {0}")]
    UnsupportedSignatureFormat(String),
    #[error(transparent)]
    IoError(#[from] io::Error),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SshSignatureFormat {
    SshRsa,
    RsaSha256,
    RsaSha512,
}

impl SshSignatureFormat {
    pub fn new<T: AsRef<str>>(format: T) -> Result<SshSignatureFormat, SshSignatureError> {
        match format.as_ref() {
            "ssh-rsa" => Ok(SshSignatureFormat::SshRsa),
            "rsa-sha2-256" => Ok(SshSignatureFormat::RsaSha256),
            "rsa-sha2-512" => Ok(SshSignatureFormat::RsaSha512),
            _ => Err(SshSignatureError::UnsupportedSignatureFormat(
                format.as_ref().to_owned(),
            )),
        }
    }

    pub fn as_str(&self) -> &str {
        match &self {
            SshSignatureFormat::SshRsa => "ssh-rsa",
            SshSignatureFormat::RsaSha256 => "rsa-sha2-256",
            SshSignatureFormat::RsaSha512 => "rsa-sha2-512",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SshSignature {
    pub format: SshSignatureFormat,
    pub blob: Vec<u8>,
}

/// Elapsed seconds since UNIX epoch
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub struct Timestamp(pub u64);

impl Timestamp {
    pub fn secs(self) -> u64 {
        self.0
    }
}

impl From<u64> for Timestamp {
    fn from(v: u64) -> Self {
        Self(v)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SshCertificate {
    pub cert_key_type: SshCertKeyType,
    pub public_key: SshPublicKey,
    pub nonce: Vec<u8>,
    pub serial: u64,
    pub cert_type: SshCertType,
    pub key_id: String,
    pub valid_principals: Vec<String>,
    pub valid_after: Timestamp,
    pub valid_before: Timestamp,
    pub critical_options: Vec<SshCriticalOption>,
    pub extensions: Vec<SshExtension>,
    pub signature_key: SshPublicKey,
    pub signature: SshSignature,
    pub comment: String,
}

impl SshCertificate {
    pub fn to_string(&self) -> Result<String, SshCertificateError> {
        let mut buffer = Vec::with_capacity(2048);
        self.encode(&mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }

    pub fn builder(&self) -> SshCertificateBuilder {
        SshCertificateBuilder::init()
    }
}

impl FromStr for SshCertificate {
    type Err = SshCertificateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        SshComplexTypeDecode::decode(s.as_bytes())
    }
}

#[derive(Debug, Error)]
pub enum SshCertificateGenerationError {
    #[error("Unsupported certificate key type: {0}")]
    UnsupportedCertificateKeyType(String),
    #[error("{0}")]
    IncorrectSignatureAlgorithm(String),
    #[error("Missing Public key")]
    MissingPublicKey,
    #[error("Missing certificate type")]
    MissingCertificateType,
    #[error("Invalid time")]
    InvalidTime,
    #[error("Missing signature key")]
    MissingSignatureKey,
    #[error("No extensions are defined for host certificates at present")]
    HostCertificateExtensions,
    #[error("No critical options are defined for host certificates at present")]
    HostCertificateCriticalOptions,
    #[error("Key type is required, but it's missing")]
    NoKeyType,
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error(transparent)]
    SshPublicKeyError(#[from] SshPublicKeyError),
    #[error(transparent)]
    SshPrivateKeyError(#[from] SshPrivateKeyError),
    #[error(transparent)]
    InvalidCertificateKeyType(#[from] SshCertTypeError),
    #[error(transparent)]
    SshCriticalOptionError(#[from] SshCriticalOptionError),
    #[error(transparent)]
    SshExtensionError(#[from] SshExtensionError),
    #[error(transparent)]
    SignatureError(#[from] SignatureError),
}

#[derive(Debug, Clone, PartialEq, Default)]
struct SshCertificateBuilderInner {
    cert_key_type: Option<SshCertKeyType>,
    public_key: Option<SshPublicKey>,
    serial: Option<u64>,
    cert_type: Option<SshCertType>,
    key_id: Option<String>,
    valid_principals: Option<Vec<String>>,
    valid_after: Option<Timestamp>,
    valid_before: Option<Timestamp>,
    critical_options: Option<Vec<SshCriticalOption>>,
    extensions: Option<Vec<SshExtension>>,
    signature_algo: Option<SignatureAlgorithm>,
    signature_key: Option<SshPrivateKey>,
    comment: Option<String>,
}

pub struct SshCertificateBuilder {
    inner: RefCell<SshCertificateBuilderInner>,
}

impl SshCertificateBuilder {
    pub fn init() -> Self {
        Self {
            inner: RefCell::new(SshCertificateBuilderInner::default()),
        }
    }

    /// Required
    pub fn cert_key_type(&self, key_type: SshCertKeyType) -> &Self {
        self.inner.borrow_mut().cert_key_type = Some(key_type);
        self
    }

    /// Required
    pub fn key(&self, key: SshPublicKey) -> &Self {
        self.inner.borrow_mut().public_key = Some(key);
        self
    }

    /// Optional (set to 0 by default)
    pub fn serial(&self, serial: u64) -> &Self {
        self.inner.borrow_mut().serial = Some(serial);
        self
    }

    /// Required
    pub fn cert_type(&self, cert_type: SshCertType) -> &Self {
        self.inner.borrow_mut().cert_type = Some(cert_type);
        self
    }

    /// Optional
    pub fn key_id(&self, key_id: String) -> &Self {
        self.inner.borrow_mut().key_id = Some(key_id);
        self
    }

    /// Optional. Zero by default means the certificate is valid for any principal of the specified type.
    pub fn principals(&self, principals: Vec<String>) -> &Self {
        self.inner.borrow_mut().valid_principals = Some(principals);
        self
    }

    /// Required
    pub fn valid_before(&self, valid_before: impl Into<Timestamp>) -> &Self {
        self.inner.borrow_mut().valid_before = Some(valid_before.into());
        self
    }

    /// Required
    pub fn valid_after(&self, valid_after: impl Into<Timestamp>) -> &Self {
        self.inner.borrow_mut().valid_after = Some(valid_after.into());
        self
    }

    /// Optional
    pub fn critical_options(&self, critical_options: Vec<SshCriticalOption>) -> &Self {
        self.inner.borrow_mut().critical_options = Some(critical_options);
        self
    }

    /// Optional
    pub fn extensions(&self, extensions: Vec<SshExtension>) -> &Self {
        self.inner.borrow_mut().extensions = Some(extensions);
        self
    }

    /// Required
    pub fn signature_key(&self, signature_key: SshPrivateKey) -> &Self {
        self.inner.borrow_mut().signature_key = Some(signature_key);
        self
    }

    /// Optional. RsaPkcs1v15 with SHA256 is used by default.
    pub fn signature_algo(&self, signature_algo: SignatureAlgorithm) -> &Self {
        self.inner.borrow_mut().signature_algo = Some(signature_algo);
        self
    }

    /// Optional
    pub fn comment(&self, comment: String) -> &Self {
        self.inner.borrow_mut().comment = Some(comment);
        self
    }

    pub fn build(&self) -> Result<SshCertificate, SshCertificateGenerationError> {
        let mut inner = self.inner.borrow_mut();

        let SshCertificateBuilderInner {
            cert_key_type,
            public_key,
            serial,
            cert_type,
            key_id,
            valid_principals,
            valid_after,
            valid_before,
            critical_options,
            extensions,
            signature_algo,
            signature_key,
            comment,
        } = inner.deref_mut();

        let cert_key_type = cert_key_type.ok_or(SshCertificateGenerationError::NoKeyType)?;
        match cert_key_type {
            SshCertKeyType::SshRsaV01 | SshCertKeyType::RsaSha2_256V01 | SshCertKeyType::RsaSha2_512v01 => {}
            SshCertKeyType::EcdsaSha2Nistp256V01
            | SshCertKeyType::SshDssV01
            | SshCertKeyType::EcdsaSha2Nistp384V01
            | SshCertKeyType::EcdsaSha2Nistp521V01
            | SshCertKeyType::SshEd25519V01 => {
                return Err(SshCertificateGenerationError::UnsupportedCertificateKeyType(
                    cert_key_type.as_str().to_owned(),
                ))
            }
        }

        let public_key = public_key
            .take()
            .ok_or(SshCertificateGenerationError::MissingPublicKey)?;
        let serial = serial.take().unwrap_or(0);
        let cert_type = cert_type
            .take()
            .ok_or(SshCertificateGenerationError::MissingCertificateType)?;
        let key_id = key_id.take().unwrap_or_default();

        let mut nonce = Vec::new();
        let mut rnd = rand::thread_rng();
        for _ in 0..32 {
            nonce.push(rnd.gen::<u8>());
        }

        let valid_after = valid_after.take().ok_or(SshCertificateGenerationError::InvalidTime)?;
        let valid_before = valid_before.take().ok_or(SshCertificateGenerationError::InvalidTime)?;

        if valid_after.secs() > valid_before.secs() {
            return Err(SshCertificateGenerationError::InvalidTime);
        }

        let valid_principals = valid_principals.take().unwrap_or_default();

        let mut critical_options = critical_options.take().unwrap_or_default();
        let mut extensions = extensions.take().unwrap_or_default();

        if cert_type == SshCertType::Host {
            if !extensions.is_empty() {
                return Err(SshCertificateGenerationError::HostCertificateExtensions);
            }
            if !critical_options.is_empty() {
                return Err(SshCertificateGenerationError::HostCertificateCriticalOptions);
            }
        }

        if cert_type == SshCertType::Client && extensions.is_empty() {
            // set default extensions for user certificate as ssh-keygen does
            extensions.extend_from_slice(&[
                SshExtension {
                    extension_type: SshExtensionType::PermitX11Forwarding,
                    data: String::new(),
                },
                SshExtension {
                    extension_type: SshExtensionType::PermitAgentForwarding,
                    data: String::new(),
                },
                SshExtension {
                    extension_type: SshExtensionType::PermitPortForwarding,
                    data: String::new(),
                },
                SshExtension {
                    extension_type: SshExtensionType::PermitPty,
                    data: String::new(),
                },
                SshExtension {
                    extension_type: SshExtensionType::PermitUserPc,
                    data: String::new(),
                },
            ])
        }

        // Options and extensions must be lexically ordered by "name" if they appear in the sequence
        critical_options
            .sort_by(|lhs, rhs| lexical_sort::lexical_cmp(lhs.option_type.as_str(), rhs.option_type.as_str()));
        extensions
            .sort_by(|lhs, rhs| lexical_sort::lexical_cmp(lhs.extension_type.as_str(), rhs.extension_type.as_str()));

        let signature_algo = signature_algo
            .take()
            .unwrap_or(SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_256));

        let signature_key = signature_key
            .take()
            .ok_or(SshCertificateGenerationError::MissingSignatureKey)?;
        let comment = comment.take().unwrap_or_default();

        let raw_signature = {
            let mut buff = Vec::with_capacity(1024);

            buff.write_ssh_string(cert_key_type.as_str())
                .map_err(SshCertificateGenerationError::IoError)?;

            buff.write_ssh_bytes(&nonce)
                .map_err(SshCertificateGenerationError::IoError)?;

            match &public_key.inner_key {
                SshBasePublicKey::Rsa(rsa) => {
                    let rsa = RsaPublicKey::try_from(rsa).unwrap();
                    buff.write_ssh_mpint(rsa.e())?;
                    buff.write_ssh_mpint(rsa.n())?;
                }
            };

            buff.write_u64::<BigEndian>(serial)
                .map_err(SshCertificateGenerationError::IoError)?;

            cert_type.encode(&mut buff)?;

            buff.write_ssh_string(&key_id)?;
            valid_principals.encode(&mut buff)?;

            valid_after.encode(&mut buff)?;
            valid_before.encode(&mut buff)?;

            critical_options.encode(&mut buff)?;

            extensions.encode(&mut buff)?;

            buff.write_ssh_bytes(&[])?; // reserved

            let mut buff2 = Vec::new();
            signature_key.public_key().inner_key.encode(&mut buff2)?;
            buff.write_ssh_bytes(&buff2)?;

            buff
        };

        let (signature_blob, signature_format) = match signature_key.base_key() {
            SshBasePrivateKey::Rsa(rsa) => {
                let signature_format = match signature_algo {
                    SignatureAlgorithm::RsaPkcs1v15(hash_algo) => match hash_algo {
                        HashAlgorithm::SHA1 => SshSignatureFormat::SshRsa,
                        HashAlgorithm::SHA2_256 => SshSignatureFormat::RsaSha256,
                        HashAlgorithm::SHA2_512 => SshSignatureFormat::RsaSha512,
                        _ => {
                            return Err(SshCertificateGenerationError::IncorrectSignatureAlgorithm(format!(
                                "Invalid signature format hash algorithm. Only sha1, sha2-256 and ssh2-521 are in use in OpenSSH, but got {:?} hash",
                                hash_algo
                            )))
                        }
                    },
                    SignatureAlgorithm::Ecdsa(_) => {
                        return Err(SshCertificateGenerationError::IncorrectSignatureAlgorithm(
                            "Ecdsa signatures for SSH certificates are not yet supported".to_owned(),
                        ))
                    }
                };

                let signature = signature_algo.sign(&raw_signature, rsa)?;
                (signature, signature_format)
            }
        };

        let signature = SshSignature {
            format: signature_format,
            blob: signature_blob,
        };

        Ok(SshCertificate {
            cert_key_type,
            public_key,
            nonce,
            serial,
            cert_type,
            key_id,
            valid_principals,
            valid_after,
            valid_before,
            critical_options,
            extensions,
            signature_key: signature_key.public_key,
            signature,
            comment,
        })
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::ssh::private_key::SshPrivateKey;
    use std::time::{SystemTime, UNIX_EPOCH};

    const PRIVATE_KEY_PEM: &str = "-----BEGIN OPENSSH PRIVATE KEY-----\n\
                                   b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAACFwAAAAdz\n\
                                   c2gtcnNhAAAAAwEAAQAAAgEA21AiuHR9Z+HThQb/7I3zJmuKuanu0mePY9hjgxiq\n\
                                   /A7nmTFmC03JOtblDDJVQU918l+pnul+FrAaIo80Fr4MKSwhk6pYUE57ZuRaYVxx\n\
                                   5CsRb4zIT8wpxzUvi9Hm83sHHnLGOa7YMPugYRcHWRoRQX4n9f+rPau8u/vBnt4V\n\
                                   CBKi3YjAw88XOusyGltuo2cTuATB7iqe15Z9iXg47ER789LwTQHXTn5L7afoDO9j\n\
                                   h+LZvcEv1fG1TmevKFNKLPA7ohBp8AOUZ4zo2hXR1rdZg/Afp0SDcSPM2MkHKqd7\n\
                                   eKeedj9Ba4b44IsYuu0cmsdA1DbszdjKUNDkVIEZH8v8VryJlLHj/wX6rzYlpBQF\n\
                                   hzQw0rHOdFpq/oNCYnBtoKMBy2D8SkYyyGzqviYMR6xOE3WgNjSaHlKaSYFlOMrh\n\
                                   peX8dRvgXHa9AvpbDI9eB6fmhmoxDi0OzKtx81hKMfRtSoDeK9uujKH3fE+L64xe\n\
                                   iWvRPqadKV4BL9nL7WCSz9Knax1mn295VrD+ISVp7/zWlz+mQMYhHh7IoK2PfJJo\n\
                                   GWx5v+gJogSe2ykP0vz3pWI95ky9GmJBhe/albQM0pe8iPclch7Je3beY3ZqeviK\n\
                                   H7hLTX5wHH6Gki7tDo6LafVQTL4peqI0nGyTSwS/LRjePrqyHLDVL1YwDp8HN56L\n\
                                   YSsAAAdIA4ihRQOIoUUAAAAHc3NoLXJzYQAAAgEA21AiuHR9Z+HThQb/7I3zJmuK\n\
                                   uanu0mePY9hjgxiq/A7nmTFmC03JOtblDDJVQU918l+pnul+FrAaIo80Fr4MKSwh\n\
                                   k6pYUE57ZuRaYVxx5CsRb4zIT8wpxzUvi9Hm83sHHnLGOa7YMPugYRcHWRoRQX4n\n\
                                   9f+rPau8u/vBnt4VCBKi3YjAw88XOusyGltuo2cTuATB7iqe15Z9iXg47ER789Lw\n\
                                   TQHXTn5L7afoDO9jh+LZvcEv1fG1TmevKFNKLPA7ohBp8AOUZ4zo2hXR1rdZg/Af\n\
                                   p0SDcSPM2MkHKqd7eKeedj9Ba4b44IsYuu0cmsdA1DbszdjKUNDkVIEZH8v8VryJ\n\
                                   lLHj/wX6rzYlpBQFhzQw0rHOdFpq/oNCYnBtoKMBy2D8SkYyyGzqviYMR6xOE3Wg\n\
                                   NjSaHlKaSYFlOMrhpeX8dRvgXHa9AvpbDI9eB6fmhmoxDi0OzKtx81hKMfRtSoDe\n\
                                   K9uujKH3fE+L64xeiWvRPqadKV4BL9nL7WCSz9Knax1mn295VrD+ISVp7/zWlz+m\n\
                                   QMYhHh7IoK2PfJJoGWx5v+gJogSe2ykP0vz3pWI95ky9GmJBhe/albQM0pe8iPcl\n\
                                   ch7Je3beY3ZqeviKH7hLTX5wHH6Gki7tDo6LafVQTL4peqI0nGyTSwS/LRjePrqy\n\
                                   HLDVL1YwDp8HN56LYSsAAAADAQABAAACAC7OXIqnefhIzx7uDoLLDODfRN05Mlo/\n\
                                   de/mR967zgo7mBwu2cuBz3e6U2oV9/IXZmHTHt1mkd1/uiQ0Efbkmq3S2FuumGiT\n\
                                   R2z/QXbUBw6eTntTPZEiTqxQYpRhuPuv/yX1cu7urP9PRLxT8OKIWLR0m0y6Qy7H\n\
                                   T2GDaqBgX3a4m3/SZumjch7GAYx0hRlkr2Wvxj/xYrM6UBKd0PBD8XxpQZX91ZjQ\n\
                                   BZ50HmdcVA61UKlZ6L6tdneEU3K0y/jpUKDXBfUOnoa3IR8iVwWPXhB1mBvX2IG2\n\
                                   FUsTJG9rDUQD6iLsfybWyJkLtrx2TIuQCPsBuep44Tz8SC7s2pLZs0HeihnrM5Ym\n\
                                   qprMggvZ1TkVFoR3bq/42XO6ULy5k8QPuP6t91UN5iVljgr8H/6Jo9MuCeRA45ZP\n\
                                   ZN94Cn1mKJWYamrqRuCqDR5za3A0oHPKYUAfzzD90BLL6Yaib75VpiEDTkOiBuW3\n\
                                   MJUcJsqZipDDl/6eas2Qyloplw60dx42FzcRIDXkXzRNn8hBSy7xmQ5MOKGBszCe\n\
                                   V/eTBtRITQN38yDVMerb8xDlwOsTtjo3PHCg4HEqqSzjv/B0op9aP7RJ8zp9xLOG\n\
                                   lxRZ9YhAlHctUOO6ATsv4uCFwCniZbVOdcUEYwNebYQ0x3IRGUF6RpqjOudUwgLl\n\
                                   o0Lq1KV05fM5AAABAC7fkAB4l5YMAseu+lcj+CwHySzcI+baRFCrMIKldNjEPvvZ\n\
                                   cCSOU/n5pgp2bw0ulw8c4mFQv0GsG//qQCBX1IrIWO0/nRBjEUTPIe2BUswoxm3+\n\
                                   F7pirphdIpABKMzV7ZvENn53p2ByrW9+uiwwXLo/z4tH18JW41Jyp5mXH2+1iWIY\n\
                                   zq5d4gVgMKLGnqWG3DisViHBGg/ExxQCayeXAhlcXVaWZiaVYsgyreaQg58S2RRU\n\
                                   IveWP+ZAeb8+ZJ72ZjIYLc0GIbP673GpcNWkRlCykTJXF9x+Ts0trffqvSxF+2YJ\n\
                                   naacLSWJmWFU1BsxUO2pIM4SI8VeHYBdEoAVqcQAAAEBAPUodhyNIr8dtcJona8L\n\
                                   kn+3BxLdvYAV1bnlWnUcG9m0RQ2L95kH6folOG00aWhRgJHFDoXcCaHND8Mg3PkA\n\
                                   XYUKCucipiIITyd8YeYnF0ckau5GmUEzwc6s4HcGyFilX1yBoyLE7hFMzOJ4+Rcq\n\
                                   +zpD2TfaWcuoo+njDWEHeTbzvGIDQoBYsPnGOtw57q9IA5oWYAG3LtwygazmNF2x\n\
                                   eEnMEtYPyPu7+W0teO0QIJiHWEuK/yLPOb+RHBfA6YJ1f9Jcgc614DxyW6qnB5Yu\n\
                                   zQBovLzgp/7j9J4Z9F8n8f9PAwYScf7IG8icVVhl5NwNgfNOpcjdg6+YB8Z0AXa4\n\
                                   dYcAAAEBAOUDEl6yS1nwZ0QsJwfHE232dpsOqxxqfV4ei4R8/obq+b5YPHiUgbt2\n\
                                   PlHyHtgfQr639BwMmIaAMSR9CLti44Mw6Z3k2DEz3Ef4+XilPeScNiZmWfYanWmV\n\
                                   wFEtb2c+YT3QweUH3DUAViHL+UdU7xp+zhkrd04daVPpYc9NNN9b9Gwmj6Pm0RP0\n\
                                   5UJxsG1ipvN1rGpaCsJiLfS9IoSsKh0Vzdzdty1YvFhEErTl0WBVGGK6xaA5lfMt\n\
                                   aclWi2mGGNXfWflyQzkz87eYlPe2RhM7jW1Lo9h1BBYE6R+jKt3q0mHwRehj+upd\n\
                                   AAXJx0RWF7EDQVJtlTfSrUCm+SSFoD0AAAAOdGVzdEBwaWNreS5jb20BAgMEBQ==\n\
                                   -----END OPENSSH PRIVATE KEY-----";

    #[test]
    fn decode_host_cert() {
        let cert = "ssh-rsa-cert-v01@openssh.com AAAAHHNzaC1yc2EtY2VydC12MDFAb3BlbnNzaC5jb20AAAAgxrum49LfnPQE9T+xcClCKuEzSrwNh3M5P6f4uwda6CsAAAADAQABAAACAQCxxwZypEyoP3lq2HfeGiyO7fenoj1txaF4UodcPMMRAyatme6BRy3gobY59IStkhN/oA1QZPVb+uOBpgepZgNPDOMrsODgU0ZxbbYwH/cdGWRoXMYlRZhw1y4KJB5ZVg+pRwrkeNpgP5yrAYuAzjg3GGovEHRDhNGuvANgje/Mr+Ye/YGASUaUaXouPMn4BxoVHM5h7SpWQSXWvy7pszsYAMadGmSnik9Xilrio3I0Z4I51vyxkePwZhKrLUW7tlJES/r3Ezurjz1FW2CniivWtTHDsuM6hLeFPdLZ/Y7yeRpUwmS+21SH/abaxqKvU5dQr1rFs2anXBnPgH2RGXS7a3TznZe0BBccy2uRrvta4eN1pjIL7Olxe8yuea1rygjAn+wb6BFLekYu/GvIPzpf+bw9yVtE51eIkQy5QyqBNJTdRXdKSU5bm8Z4XZcgX5osDG+dpL2SewgLlrxXrAsrSjAeycLKwO+VOUFLMmFO040ZjuAs4Sbw8ptkePdCveU1BFHpWyvf/WG/BmdUzrSwjjVOJT2kguBLiOiH8YAOncCFMLDcHBfd5hFU6jQ5U7CU8HM2wYV8uq1kXtXqmfJ4QJV1D9he8MOJ+u3G4KZR0uNREe5gX7WjvQGT3kql5c8LanDb3rY0Auj9pJd639f7XGN+UYGROuycqvB7BvgQ1wAAAAAAAAAAAAAAAgAAAAVwaWNreQAAACsAAAARZmlyc3QuZXhhbXBsZS5jb20AAAASc2Vjb25kLmV4YW1wbGUuY29tAAAAAGFlVGwAAAAAY0U22QAAAAAAAAAAAAAAAAAAAhcAAAAHc3NoLXJzYQAAAAMBAAEAAAIBAMwDtw6lA1R20MaWSHCB/23LYMQvKjiXv2mh3YjsHZZYj9mzoeWmhOF4jjDTB2r6//BuwPIyq+We4AQqbZladmXo1CVPZqtgCa2zCMRfWukj+OvluglSFqgc4fpFyEvbC1o7HA+OGzCcWS7fg2VKNyWnXuVxvPNJhgCo+fzXf3CQyWJ9rO5H6QGKaTtczW7IlZ7WfA1KP/NtCg57QWQzghH2hxTHK+DQN6uGzdIMmddJBklJXkialS+FhSJuWNKAkeN/gwfQ7qgItDUG9hRYvOO7aQbf1u/UQpXtV9jH+KAZrDlRS4/DdSta6G9bHjPfX/sqJYchIdbjLwPvu07Q2Gu6BRVj5qiKxH5VJ1eoHuw6PyV/EJP0nseUK8bspcxZ2ooIxmXbetpBdv5r4Piztw4CPZAap1ZXUhivc8hR/1Q5DhXAHKjtZVQ6nUTqALB27b6lkCUoaOgN/BW//O9Yh/g1uW8le8pzO7y8KsQL1pO9DkutJYQh9dEhVJvYkAHeQVWLTKOIUgGCzaVwh6i9VgwdVgibgqrJPxqJPhA1AEk2Wl+390cU/BfqyDM7/S0ezNoBKSY9dtAOBFE5uBd8PwwdhhnQKbHl+FVyco2A5ncN9bkpQgPlF1Cp+Pi/xQUyrJ3oOxuIszmN7Mhg+b2DiDygqbQ0U/IPpa3AY8QlMnL3AAACFAAAAAxyc2Etc2hhMi01MTIAAAIAaUKPXTKkIouWmHjfhSqV97D3Sh/airfktqVeZTAwjvVkwDcNSswJROfNr8r1Y3RlcFzGI/iFFBjfdoq4kdhMyh+wQs12lkqywj+S96Um9ox846OZwVa43eGuI+aH8D1jUiaFiLJG6+NK0yj4y/i+fHQpS9xveF1T+MsxCnhZ8AMLp0dkokfM1QowXpHHoTJeyg5g2GngxWYZcKogLYo/bVNcL5OoWQwrPDLQeJ+Oumv6HxNb1EOR6QpdQBvrw4mnpfyR1Z8pMNCACFHPCKimvEhfV5xlTtp6N1GH2rDyT8L1iuluMBMBVYmS9MLt2xbY4MJSf2wpvjgyQhhlOlMWjC1/dmaIri+V2qozG5S8Z/Yc0hgigJ8YQl747j7KDA6fSSYzSNogt7x1DLE8Vg6eSHEw05QDPZwBDh7sV+9MKgsZZX0Yb/dXGMEAttDs63YmLL2IqIRFgcJLlsD3fkNxnZvgkppKSw2KVic5PpONwD3DgvRyneVKLUICbh/WhOev90J+UKU/vyHEjrNX4XcJ9uhTc14sWxS5JyRRU48MjrLLQYK1ods6aAIqmOGc6YW3Q4pZFDuwO0dFpNnJPlzeytOObVSk+9ybFF45tJdViU1H7i832o4ifVFVV+jicLB8uy4ov6XG1h4kCeaUzIil90yosg9+qmBzDktkqbocPKc= sasha@kubuntu \n";
        let cert = SshCertificate::from_str(cert).unwrap();

        assert_eq!(SshCertType::Host, cert.cert_type);
        assert_eq!("picky".to_owned(), cert.key_id);
        assert_eq!(
            vec!["first.example.com".to_owned(), "second.example.com".to_owned()],
            cert.valid_principals
        );
        assert_eq!("sasha@kubuntu", cert.comment);
        assert!(cert.critical_options.is_empty());
        assert!(cert.extensions.is_empty());
    }

    #[test]
    fn decode_client_cert() {
        let cert = "ssh-rsa-cert-v01@openssh.com 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 sasha@kubuntu \n";
        let cert = SshCertificate::from_str(cert).unwrap();

        assert_eq!(SshCertType::Client, cert.cert_type);
        assert_eq!("picky@example.com".to_owned(), cert.key_id);
        assert_eq!(vec!["test-user".to_owned(), "guest".to_owned()], cert.valid_principals);
        assert_eq!("sasha@kubuntu", cert.comment);
        assert!(cert.critical_options.is_empty());
        assert_eq!(
            vec![
                SshExtension::new(SshExtensionType::PermitX11Forwarding, "".to_owned()),
                SshExtension::new(SshExtensionType::PermitAgentForwarding, "".to_owned()),
                SshExtension::new(SshExtensionType::PermitPortForwarding, "".to_owned()),
                SshExtension::new(SshExtensionType::PermitPty, "".to_owned()),
                SshExtension::new(SshExtensionType::PermitUserPc, "".to_owned()),
            ],
            cert.extensions
        );
    }

    #[test]
    fn encode_host_cert() {
        let cert_before = "ssh-rsa-cert-v01@openssh.com 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 sasha@kubuntu\r\n";
        let cert: SshCertificate = SshCertificate::from_str(cert_before).unwrap();

        let cert_after = cert.to_string().unwrap();

        pretty_assertions::assert_eq!(cert_after, cert_before);
    }

    #[test]
    fn encode_client_cert() {
        let cert_before = "ssh-rsa-cert-v01@openssh.com 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 sasha@kubuntu\r\n";
        let cert: SshCertificate = SshCertificate::from_str(cert_before).unwrap();

        let cert_after = cert.to_string().unwrap();

        pretty_assertions::assert_eq!(cert_before, cert_after);
    }

    #[test]
    fn test_required_fields_in_certificate_builder() {
        let certificate_builder = SshCertificateBuilder::init();

        certificate_builder.cert_key_type(SshCertKeyType::RsaSha2_256V01);

        let private_key: SshPrivateKey = SshPrivateKey::from_pem_str(PRIVATE_KEY_PEM, None).unwrap();
        certificate_builder.key(private_key.public_key().clone());

        certificate_builder.cert_type(SshCertType::Host);

        let now_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        certificate_builder.valid_after(now_timestamp);

        // 10 minutes = 600 seconds
        let valid_before = now_timestamp + 600;
        certificate_builder.valid_before(valid_before);

        certificate_builder.signature_key(private_key);
        certificate_builder.build().unwrap();
    }

    #[test]
    fn test_time_validation_in_certificate_builder() {
        let certificate_builder = SshCertificateBuilder::init();

        certificate_builder.cert_key_type(SshCertKeyType::RsaSha2_256V01);

        let private_key: SshPrivateKey = SshPrivateKey::from_pem_str(PRIVATE_KEY_PEM, None).unwrap();
        certificate_builder.key(private_key.public_key().clone());

        certificate_builder.cert_type(SshCertType::Host);

        let now_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        // 10 minutes = 600 seconds
        let after = now_timestamp + 600;
        let before = now_timestamp - 600;

        certificate_builder.valid_after(after);

        certificate_builder.valid_before(before);

        certificate_builder.signature_key(private_key);

        let cert = certificate_builder.build();
        assert!(matches!(cert.unwrap_err(), SshCertificateGenerationError::InvalidTime));
    }

    #[test]
    fn test_host_certificate_generation() {
        let certificate_builder = SshCertificateBuilder::init();

        certificate_builder.cert_key_type(SshCertKeyType::RsaSha2_256V01);

        let private_key: SshPrivateKey = SshPrivateKey::from_pem_str(PRIVATE_KEY_PEM, None).unwrap();
        certificate_builder.key(private_key.public_key().clone());

        certificate_builder.cert_type(SshCertType::Host);

        let now_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        certificate_builder.valid_after(now_timestamp);

        // 10 minutes = 600 seconds
        let valid_before = now_timestamp + 600;
        certificate_builder.valid_before(valid_before);

        certificate_builder.signature_key(private_key);

        certificate_builder.principals(vec!["example".to_owned()]);

        certificate_builder.extensions(vec![SshExtension::new(
            SshExtensionType::NoTouchRequired,
            "".to_owned(),
        )]);

        let cert = certificate_builder.build();
        assert!(matches!(
            cert.unwrap_err(),
            SshCertificateGenerationError::HostCertificateExtensions
        ));
    }
}
