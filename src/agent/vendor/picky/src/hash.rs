//! Hash algorithms supported by picky

use digest::Digest;
use picky_asn1_x509::ShaVariant;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

/// unsupported algorithm
#[derive(Debug)]
pub struct UnsupportedHashAlgorithmError {
    pub algorithm: String,
}

impl fmt::Display for UnsupportedHashAlgorithmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unsupported algorithm:  {}", self.algorithm)
    }
}

impl Error for UnsupportedHashAlgorithmError {}

/// Supported hash algorithms
#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum HashAlgorithm {
    MD5,
    SHA1,
    SHA2_224,
    SHA2_256,
    SHA2_384,
    SHA2_512,
    SHA3_384,
    SHA3_512,
}

impl From<HashAlgorithm> for rsa::Hash {
    fn from(v: HashAlgorithm) -> rsa::Hash {
        match v {
            HashAlgorithm::MD5 => rsa::Hash::MD5,
            HashAlgorithm::SHA1 => rsa::Hash::SHA1,
            HashAlgorithm::SHA2_224 => rsa::Hash::SHA2_224,
            HashAlgorithm::SHA2_256 => rsa::Hash::SHA2_256,
            HashAlgorithm::SHA2_384 => rsa::Hash::SHA2_384,
            HashAlgorithm::SHA2_512 => rsa::Hash::SHA2_512,
            HashAlgorithm::SHA3_384 => rsa::Hash::SHA3_384,
            HashAlgorithm::SHA3_512 => rsa::Hash::SHA3_512,
        }
    }
}

impl TryFrom<HashAlgorithm> for ShaVariant {
    type Error = UnsupportedHashAlgorithmError;

    fn try_from(v: HashAlgorithm) -> Result<ShaVariant, UnsupportedHashAlgorithmError> {
        match v {
            HashAlgorithm::MD5 => Ok(ShaVariant::MD5),
            HashAlgorithm::SHA1 => Ok(ShaVariant::SHA1),
            HashAlgorithm::SHA2_256 => Ok(ShaVariant::SHA2_256),
            HashAlgorithm::SHA2_384 => Ok(ShaVariant::SHA2_384),
            HashAlgorithm::SHA2_512 => Ok(ShaVariant::SHA2_512),
            HashAlgorithm::SHA3_384 => Ok(ShaVariant::SHA3_384),
            HashAlgorithm::SHA3_512 => Ok(ShaVariant::SHA3_512),
            _ => Err(UnsupportedHashAlgorithmError {
                algorithm: format!("{:?}", v),
            }),
        }
    }
}

impl TryFrom<ShaVariant> for HashAlgorithm {
    type Error = UnsupportedHashAlgorithmError;

    fn try_from(v: ShaVariant) -> Result<HashAlgorithm, UnsupportedHashAlgorithmError> {
        match v {
            ShaVariant::MD5 => Ok(HashAlgorithm::MD5),
            ShaVariant::SHA1 => Ok(HashAlgorithm::SHA1),
            ShaVariant::SHA2_256 => Ok(HashAlgorithm::SHA2_256),
            ShaVariant::SHA2_384 => Ok(HashAlgorithm::SHA2_384),
            ShaVariant::SHA2_512 => Ok(HashAlgorithm::SHA2_512),
            ShaVariant::SHA3_384 => Ok(HashAlgorithm::SHA3_384),
            ShaVariant::SHA3_512 => Ok(HashAlgorithm::SHA3_512),
            _ => Err(UnsupportedHashAlgorithmError {
                algorithm: format!("{:?}", v),
            }),
        }
    }
}

impl HashAlgorithm {
    pub fn digest(self, msg: &[u8]) -> Vec<u8> {
        match self {
            Self::MD5 => md5::Md5::digest(msg).as_slice().to_vec(),
            Self::SHA1 => sha1::Sha1::digest(msg).as_slice().to_vec(),
            Self::SHA2_224 => sha2::Sha224::digest(msg).as_slice().to_vec(),
            Self::SHA2_256 => sha2::Sha256::digest(msg).as_slice().to_vec(),
            Self::SHA2_384 => sha2::Sha384::digest(msg).as_slice().to_vec(),
            Self::SHA2_512 => sha2::Sha512::digest(msg).as_slice().to_vec(),
            Self::SHA3_384 => sha3::Sha3_384::digest(msg).as_slice().to_vec(),
            Self::SHA3_512 => sha3::Sha3_512::digest(msg).as_slice().to_vec(),
        }
    }

    pub fn output_size(self) -> usize {
        match self {
            Self::MD5 => md5::Md5::output_size(),
            Self::SHA1 => sha1::Sha1::output_size(),
            Self::SHA2_224 => sha2::Sha224::output_size(),
            Self::SHA2_256 => sha2::Sha256::output_size(),
            Self::SHA2_384 => sha2::Sha384::output_size(),
            Self::SHA2_512 => sha2::Sha512::output_size(),
            Self::SHA3_384 => sha3::Sha3_384::output_size(),
            Self::SHA3_512 => sha3::Sha3_512::output_size(),
        }
    }
}
