pub mod der;
pub mod hash_algorithm;
pub mod oid;

use anyhow::bail;
use once_cell::sync::Lazy;
use openssl::bn::BigNumRef;
use openssl::rand;
use regex::{self, bytes};

pub use crate::util::hash_algorithm::HashAlgorithm;

pub use HashAlgorithm::Sha1 as SHA_1;
pub use HashAlgorithm::Sha256 as SHA_256;
pub use HashAlgorithm::Sha384 as SHA_384;
pub use HashAlgorithm::Sha512 as SHA_512;

pub fn random_bytes(len: usize) -> Vec<u8> {
    let mut vec = vec![0; len];
    rand::rand_bytes(&mut vec).unwrap();
    vec
}

pub(crate) fn ceiling(len: usize, div: usize) -> usize {
    (len + (div - 1)) / div
}

pub(crate) fn is_base64_url_safe_nopad(input: &str) -> bool {
    static RE_BASE64: Lazy<regex::Regex> = Lazy::new(|| {
        regex::Regex::new(
            r"^(?:[A-Za-z0-9+/_-]{4})*(?:[A-Za-z0-9+/_-]{2}(==)?|[A-Za-z0-9+/_-]{3}=?)?$",
        )
        .unwrap()
    });

    RE_BASE64.is_match(input)
}

pub(crate) fn parse_pem(input: &[u8]) -> anyhow::Result<(String, Vec<u8>)> {
    static RE_PEM: Lazy<bytes::Regex> = Lazy::new(|| {
        bytes::Regex::new(concat!(
            r"^",
            r"-----BEGIN ([A-Z0-9 -]+)-----[\t ]*(?:\r\n|[\r\n])",
            r"([\t\r\n a-zA-Z0-9+/=]+)",
            r"-----END ([A-Z0-9 -]+)-----[\t ]*(?:\r\n|[\r\n])?",
            r"$"
        ))
        .unwrap()
    });

    static RE_FILTER: Lazy<bytes::Regex> = Lazy::new(|| bytes::Regex::new("[\t\r\n ]").unwrap());

    let result = if let Some(caps) = RE_PEM.captures(input) {
        match (caps.get(1), caps.get(2), caps.get(3)) {
            (Some(ref m1), Some(ref m2), Some(ref m3)) if m1.as_bytes() == m3.as_bytes() => {
                let alg = String::from_utf8(m1.as_bytes().to_vec())?;
                let base64_data = RE_FILTER.replace_all(m2.as_bytes(), bytes::NoExpand(b""));
                let data = base64::decode_config(&base64_data, base64::STANDARD)?;
                (alg, data)
            }
            _ => bail!("Mismatched the begging and ending label."),
        }
    } else {
        bail!("Invalid PEM format.");
    };

    Ok(result)
}

pub(crate) fn num_to_vec(num: &BigNumRef, len: usize) -> Vec<u8> {
    let vec = num.to_vec();
    if vec.len() < len {
        let mut tmp = Vec::with_capacity(len);
        for _ in 0..(len - vec.len()) {
            tmp.push(0);
        }
        tmp.extend_from_slice(&vec);
        tmp
    } else {
        vec
    }
}

#[cfg(test)]
mod tests {
    use super::is_base64_url_safe_nopad;

    #[test]
    fn test_is_base64_url_safe_nopad() {
        assert!(is_base64_url_safe_nopad("MA"));
        assert!(is_base64_url_safe_nopad("MDEyMzQ1Njc4OQ"));
        assert!(is_base64_url_safe_nopad("MDEyMzQ1Njc4OQ=="));
        assert!(!is_base64_url_safe_nopad("AB<>"));
        assert!(!is_base64_url_safe_nopad("MDEyMzQ1Njc4OQ="));
        assert!(!is_base64_url_safe_nopad("MDEyMzQ1Njc4O"));
    }
}
