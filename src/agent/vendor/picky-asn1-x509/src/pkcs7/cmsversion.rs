use serde::{de, ser, Deserialize, Serialize};
use std::fmt;

/// [RFC 5682 #10.2.5](https://datatracker.ietf.org/doc/html/rfc5652#section-10.2.5)
/// ``` not_rust
/// CmsVersion ::= INTEGER
///                      { v0(0), v1(1), v2(2), v3(3), v4(4), v5(5) }
/// ```
#[derive(Debug, PartialEq, Clone, Copy)]
#[repr(u8)]
pub enum CmsVersion {
    V0 = 0x00,
    V1 = 0x01,
    V2 = 0x02,
    V3 = 0x03,
    V4 = 0x04,
    V5 = 0x05,
}

impl CmsVersion {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x00 => Some(Self::V0),
            0x01 => Some(Self::V1),
            0x02 => Some(Self::V2),
            0x03 => Some(Self::V3),
            0x04 => Some(Self::V4),
            0x05 => Some(Self::V5),
            _ => None,
        }
    }
}

impl Serialize for CmsVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for CmsVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = CmsVersion;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "a valid cms version number")
            }

            fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let cms_version = CmsVersion::from_u8(v).ok_or_else(|| {
                    E::invalid_value(
                        de::Unexpected::Other("invalid cms version number"),
                        &"a valid integer representing a supported cms version number (0, 1, 2, 3, 4 or 5)",
                    )
                })?;

                Ok(cms_version)
            }
        }

        deserializer.deserialize_u8(Visitor)
    }
}
