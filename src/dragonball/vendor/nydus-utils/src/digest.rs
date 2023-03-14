// Copyright 2020 Ant Group. All rights reserved.
// Copyright (C) 2020-2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! Fast message digest algorithms for Rafs and Nydus, including Blake3 and SHA256.

use std::convert::TryFrom;
use std::fmt;
use std::io::Error;
use std::str::FromStr;

use sha2::digest::Digest;
use sha2::Sha256;

/// Size in bytes of chunk digest value.
pub const RAFS_DIGEST_LENGTH: usize = 32;

type DigestData = [u8; RAFS_DIGEST_LENGTH];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Algorithm {
    Blake3,
    Sha256,
}

impl Default for Algorithm {
    fn default() -> Self {
        Self::Blake3
    }
}

impl fmt::Display for Algorithm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl FromStr for Algorithm {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "blake3" => Ok(Self::Blake3),
            "sha256" => Ok(Self::Sha256),
            _ => Err(einval!("digest algorithm should be blake3 or sha256")),
        }
    }
}

impl TryFrom<u32> for Algorithm {
    type Error = ();

    fn try_from(value: u32) -> std::result::Result<Self, Self::Error> {
        if value == Algorithm::Sha256 as u32 {
            Ok(Algorithm::Sha256)
        } else if value == Algorithm::Blake3 as u32 {
            Ok(Algorithm::Blake3)
        } else {
            Err(())
        }
    }
}

pub trait DigestHasher {
    fn digest_update(&mut self, buf: &[u8]);
    fn digest_finalize(self) -> RafsDigest;
}

/// Fast message digest algorithm.
///
/// The size of Hasher struct is a little big, say
/// blake3::Hasher: 1912 bytes
/// Sha256: 112 bytes
/// RafsDigestHasher: 1920
///
/// So we should avoid any unnecessary clone() operation. Add we prefer allocation on stack
/// instead of allocation on heap.
///
/// If allocating memory for blake3::Hahser is preferred over using the stack, please try:
/// Blake3(Box<blake3::Hasher>). But be careful, this will cause one extra memory allocation/free
/// for each digest.
#[derive(Clone, Debug)]
pub enum RafsDigestHasher {
    Blake3(Box<blake3::Hasher>),
    Sha256(Sha256),
}

impl DigestHasher for RafsDigestHasher {
    fn digest_update(&mut self, buf: &[u8]) {
        match self {
            RafsDigestHasher::Blake3(hasher) => {
                hasher.update(buf);
            }
            RafsDigestHasher::Sha256(hasher) => {
                hasher.update(buf);
            }
        }
    }

    fn digest_finalize(self) -> RafsDigest {
        let data = match self {
            RafsDigestHasher::Blake3(hasher) => hasher.finalize().into(),
            RafsDigestHasher::Sha256(hasher) => hasher.finalize().into(),
        };

        RafsDigest { data }
    }
}

impl DigestHasher for blake3::Hasher {
    fn digest_update(&mut self, buf: &[u8]) {
        self.update(buf);
    }

    fn digest_finalize(self) -> RafsDigest {
        RafsDigest {
            data: self.finalize().into(),
        }
    }
}

impl DigestHasher for Sha256 {
    fn digest_update(&mut self, buf: &[u8]) {
        self.update(buf);
    }

    fn digest_finalize(self) -> RafsDigest {
        RafsDigest {
            data: self.finalize().into(),
        }
    }
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Default, Ord, PartialOrd)]
pub struct RafsDigest {
    pub data: DigestData,
}

impl RafsDigest {
    pub fn from_buf(buf: &[u8], algorithm: Algorithm) -> Self {
        let data: DigestData = match algorithm {
            Algorithm::Blake3 => blake3::hash(buf).into(),
            Algorithm::Sha256 => {
                let mut hasher = Sha256::new();
                hasher.update(buf);
                hasher.finalize().into()
            }
        };

        RafsDigest { data }
    }

    pub fn hasher(algorithm: Algorithm) -> RafsDigestHasher {
        match algorithm {
            Algorithm::Blake3 => RafsDigestHasher::Blake3(Box::new(blake3::Hasher::new())),
            Algorithm::Sha256 => RafsDigestHasher::Sha256(Sha256::new()),
        }
    }
}

impl From<DigestData> for RafsDigest {
    fn from(data: DigestData) -> Self {
        Self { data }
    }
}

impl AsRef<[u8]> for RafsDigest {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl fmt::Display for RafsDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in &self.data {
            write!(f, "{:02x}", c).unwrap()
        }
        Ok(())
    }
}

impl From<RafsDigest> for String {
    fn from(d: RafsDigest) -> Self {
        format!("{}", d)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_algorithm() {
        assert_eq!(Algorithm::from_str("blake3").unwrap(), Algorithm::Blake3);
        assert_eq!(Algorithm::from_str("sha256").unwrap(), Algorithm::Sha256);
        Algorithm::from_str("Blake3").unwrap_err();
        Algorithm::from_str("SHA256").unwrap_err();
    }

    #[test]
    fn test_hash_from_buf() {
        let text = b"The quick brown fox jumps over the lazy dog";

        let blake3 = RafsDigest::from_buf(text, Algorithm::Blake3);
        let str: String = blake3.into();
        assert_eq!(
            str.as_bytes(),
            b"2f1514181aadccd913abd94cfa592701a5686ab23f8df1dff1b74710febc6d4a"
        );

        let sha256 = RafsDigest::from_buf(text, Algorithm::Sha256);
        let str: String = sha256.into();
        assert_eq!(
            str.as_bytes(),
            b"d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592"
        );
    }

    #[test]
    fn test_hasher() {
        let text = b"The quick brown fox jumps ";
        let text2 = b"over the lazy dog";

        let mut hasher = RafsDigest::hasher(Algorithm::Blake3);
        hasher.digest_update(text);
        hasher.digest_update(text2);
        let blake3 = hasher.digest_finalize();
        let str: String = blake3.into();
        assert_eq!(
            str.as_bytes(),
            b"2f1514181aadccd913abd94cfa592701a5686ab23f8df1dff1b74710febc6d4a"
        );

        let mut hasher = RafsDigest::hasher(Algorithm::Sha256);
        hasher.digest_update(text);
        hasher.digest_update(text2);
        let sha256 = hasher.digest_finalize();
        let str: String = sha256.into();
        assert_eq!(
            str.as_bytes(),
            b"d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592"
        );
    }
}
