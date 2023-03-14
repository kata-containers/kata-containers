use serde::{de, ser, Deserialize, Serialize};
use std::fmt;

#[derive(Debug, PartialEq, Clone, Copy)]
#[repr(u8)]
pub enum Version {
    V1 = 0x00,
    V2 = 0x01,
    V3 = 0x02,
}

impl Default for Version {
    fn default() -> Self {
        Self::V1
    }
}

impl Version {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x00 => Some(Self::V1),
            0x01 => Some(Self::V2),
            0x02 => Some(Self::V3),
            _ => None,
        }
    }
}

impl Serialize for Version {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for Version {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Version;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "a valid version number")
            }

            fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let version = Version::from_u8(v).ok_or_else(|| {
                    E::invalid_value(
                        de::Unexpected::Other("invalid version number"),
                        &"a valid integer representing a supported version number (0, 1 or 2)",
                    )
                })?;

                Ok(version)
            }
        }

        deserializer.deserialize_u8(Visitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use picky_asn1::wrapper::{ExplicitContextTag9, Optional};
    use picky_asn1_der::Asn1DerError;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct OptionalVersionTestStruct {
        #[serde(skip_serializing_if = "version_is_default")]
        version: Optional<ExplicitContextTag9<Version>>,
        other_non_optional_integer: u8,
    }

    fn version_is_default(version: &Optional<ExplicitContextTag9<Version>>) -> bool {
        (version.0).0 == Version::V1
    }

    #[test]
    fn optional_version() {
        let buffer_with_version: [u8; 10] = [0x30, 0x08, 0xA9, 0x03, 0x02, 0x01, 0x02, 0x02, 0x01, 0x6E];

        let non_default = OptionalVersionTestStruct {
            version: ExplicitContextTag9(Version::V3).into(),
            other_non_optional_integer: 0x6E,
        };

        check_serde!(non_default: OptionalVersionTestStruct in buffer_with_version);

        let buffer_without_version: [u8; 5] = [0x30, 0x03, 0x02, 0x01, 0x6E];

        let default = OptionalVersionTestStruct {
            version: ExplicitContextTag9(Version::default()).into(),
            other_non_optional_integer: 0x6E,
        };

        check_serde!(default: OptionalVersionTestStruct in buffer_without_version);
    }

    #[test]
    fn unsupported_version() {
        let buffer: [u8; 3] = [0x02, 0x01, 0x0F];

        let version: picky_asn1_der::Result<Version> = picky_asn1_der::from_bytes(&buffer);
        match version {
            Err(Asn1DerError::Message(msg)) => assert_eq!(
                msg,
                "invalid value: invalid version number, expected a valid integer \
                 representing a supported version number (0, 1 or 2)"
            ),
            Err(err) => panic!("invalid error: {}", err),
            Ok(_) => panic!("parsing should have failed but did not"),
        }
    }
}
