// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use strum::{Display, EnumProperty, EnumString};

use std::convert::TryFrom;
use std::error::Error;
use std::fmt;
use std::str::FromStr;
use std::string::ToString;

// Supported digest algorithm types
#[derive(EnumString, Display, Debug, PartialEq, Eq, EnumProperty)]
pub enum Algorithm {
    #[strum(serialize = "sha256", props(Length = "64"))]
    Sha256,
    #[strum(serialize = "sha384", props(Length = "96"))]
    Sha384,
    #[strum(serialize = "sha512", props(Length = "128"))]
    Sha512,
}

// Reasons that parsing a string as a Digest can fail.
#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Invalid checksum digest format
    InvalidFormat,
    /// Invalid checksum digest length
    InvalidLength,
    /// Unsupported digest algorithm
    UnsupportedAlgorithm,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidFormat => write!(f, "invalid checksum digest format"),
            ParseError::InvalidLength => write!(f, "invalid checksum digest length"),
            ParseError::UnsupportedAlgorithm => write!(f, "unsupported digest algorithm"),
        }
    }
}

impl Error for ParseError {}

// Digest allows simple protection of hex formatted digest strings, prefixed
// by their algorithm. Strings of type Digest have some guarantee of being in
// the correct format and it provides quick access to the components of a
// digest string.
//
// The following is an example of the contents of Digest types:
//
// 	sha256:7173b809ca12ec5dee4506cd86be934c4596dd234ee82c0662eac04a8c2c71dc
//
// This allows to abstract the digest behind this type and work only in those
// terms.
#[derive(Default, PartialEq, Eq, Debug, Clone)]
pub struct Digest {
    algorithm: String,
    value: String,
}

impl Digest {
    pub fn algorithm(&self) -> String {
        self.algorithm.clone()
    }

    pub fn value(&self) -> String {
        self.value.clone()
    }

    pub fn is_empty(&self) -> bool {
        self.algorithm.is_empty() || self.value.is_empty()
    }
}

impl ToString for Digest {
    fn to_string(&self) -> String {
        format!("{}:{}", self.algorithm, self.value)
    }
}

impl TryFrom<&str> for Digest {
    type Error = ParseError;

    fn try_from(digest: &str) -> Result<Self, Self::Error> {
        let parsed_digest: Vec<&str> = digest.split(':').collect();

        if parsed_digest.len() != 2 {
            return Err(ParseError::InvalidFormat);
        }

        let algorithm = parsed_digest[0];
        let value = parsed_digest[1];

        if algorithm.is_empty() || value.is_empty() {
            return Err(ParseError::InvalidFormat);
        }

        if hex::decode(value).is_err() {
            return Err(ParseError::InvalidFormat);
        }

        if let Some(expect_value_len_str) = match Algorithm::from_str(algorithm) {
            Result::Ok(Algorithm::Sha256) => Algorithm::Sha256.get_str("Length"),
            Result::Ok(Algorithm::Sha384) => Algorithm::Sha384.get_str("Length"),
            Result::Ok(Algorithm::Sha512) => Algorithm::Sha512.get_str("Length"),
            _ => {
                return Err(ParseError::UnsupportedAlgorithm);
            }
        } {
            let expect_value_len = expect_value_len_str.to_string().parse::<usize>();

            if Ok(value.to_string().len()) != expect_value_len {
                return Err(ParseError::InvalidLength);
            }
        }

        let res = Digest {
            algorithm: algorithm.to_string(),
            value: value.to_string(),
        };

        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_digest() {
        #[derive(Debug)]
        struct TestData<'a> {
            digest: &'a str,
            err: Option<ParseError>,
            res: Option<Digest>,
        }

        let unexpect_cases = &[
            TestData {
                digest: "",
                err: Some(ParseError::InvalidFormat),
                res: None,
            },
            TestData {
                digest: "unexpect format",
                err: Some(ParseError::InvalidFormat),
                res: None,
            },
            TestData {
                digest: "sha256@:12345@&:67890",
                err: Some(ParseError::InvalidFormat),
                res: None,
            },
            TestData {
                digest: "sha256:",
                err: Some(ParseError::InvalidFormat),
                res: None,
            },
            TestData {
                digest: ":69704ef328d05a9f806b6b8502915e6a0a4faa4d72018dc42343f511490daf8a",
                err: Some(ParseError::InvalidFormat),
                res: None,
            },
            TestData {
                digest: "sha123:69704ef328d05a9f806b6b8502915e6a0a4faa4d72018dc42343f511490daf8a",
                err: Some(ParseError::UnsupportedAlgorithm),
                res: None,
            },
            TestData {
                digest: "sha256:69704ef328d05a9f806b6b8502915e6a0a4faa4d72018dc42343f511490d",
                err: Some(ParseError::InvalidLength),
                res: None,
            },
            TestData {
                digest: "sha384:69704ef328d05a9f806b6b8502915e6a0a4faa4d72018dc42343f511490daf8a",
                err: Some(ParseError::InvalidLength),
                res: None,
            },
            TestData {
                digest: "sha512:69704ef328d05a9f806b6b8502915e6a0a4faa4d72018dc42343f511490daf8a",
                err: Some(ParseError::InvalidLength),
                res: None,
            },
            TestData {
                digest: "sha256:gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg",
                err: Some(ParseError::InvalidFormat),
                res: None,
            },
        ];

        let expect_cases = &[TestData {
            digest: "sha256:69704ef328d05a9f806b6b8502915e6a0a4faa4d72018dc42343f511490daf8a",
            err: None,
            res: Some(Digest {
                algorithm: "sha256".to_string(),
                value: "69704ef328d05a9f806b6b8502915e6a0a4faa4d72018dc42343f511490daf8a"
                    .to_string(),
            }),
        }];

        for case in unexpect_cases.iter() {
            assert_eq!(
                &Digest::try_from(case.digest).unwrap_err(),
                case.err.as_ref().unwrap()
            );
        }

        for case in expect_cases.iter() {
            assert_eq!(
                &Digest::try_from(case.digest).unwrap(),
                case.res.as_ref().unwrap()
            );
        }
    }
}
