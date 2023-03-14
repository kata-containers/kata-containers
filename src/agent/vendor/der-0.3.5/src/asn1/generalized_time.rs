//! ASN.1 `GeneralizedTime` support.

use crate::{
    datetime::{self, DateTime},
    Any, Encodable, Encoder, Error, ErrorKind, Header, Length, Result, Tag, Tagged,
};
use core::{convert::TryFrom, time::Duration};

#[cfg(feature = "std")]
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum duration since `UNIX_EPOCH` allowable as `GeneralizedTime`.
const MAX_UNIX_DURATION: Duration = Duration::from_secs(253_402_300_800);

/// ASN.1 `GeneralizedTime` type.
///
/// This type implements the validity requirements specified in
/// [RFC 5280 Section 4.1.2.5.2][1], namely:
///
/// > For the purposes of this profile, GeneralizedTime values MUST be
/// > expressed in Greenwich Mean Time (Zulu) and MUST include seconds
/// > (i.e., times are `YYYYMMDDHHMMSSZ`), even where the number of seconds
/// > is zero.  GeneralizedTime values MUST NOT include fractional seconds.
///
/// [1]: https://tools.ietf.org/html/rfc5280#section-4.1.2.5.2
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct GeneralizedTime(Duration);

impl GeneralizedTime {
    /// Length of an RFC 5280-flavored ASN.1 DER-encoded [`GeneralizedTime`].
    pub const LENGTH: Length = Length::new(15);

    /// Length of an RFC 5280-flavored ASN.1 DER-encoded [`GeneralizedTime`].
    #[deprecated(since = "0.3.3", note = "please use GeneralizedTime::LENGTH")]
    pub const fn length() -> Length {
        Self::LENGTH
    }

    /// Create a new [`GeneralizedTime`] given a [`Duration`] since `UNIX_EPOCH`
    /// (a.k.a. "Unix time")
    pub fn new(unix_duration: Duration) -> Result<Self> {
        if unix_duration < MAX_UNIX_DURATION {
            Ok(Self(unix_duration))
        } else {
            Err(ErrorKind::Value {
                tag: Tag::GeneralizedTime,
            }
            .into())
        }
    }

    /// Get the duration of this timestamp since `UNIX_EPOCH`.
    pub fn unix_duration(&self) -> Duration {
        self.0
    }

    /// Instantiate from [`SystemTime`].
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    pub fn from_system_time(time: SystemTime) -> Result<Self> {
        time.duration_since(UNIX_EPOCH)
            .map_err(|_| {
                ErrorKind::Value {
                    tag: Tag::GeneralizedTime,
                }
                .into()
            })
            .and_then(Self::new)
    }

    /// Convert to [`SystemTime`].
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    pub fn to_system_time(&self) -> SystemTime {
        UNIX_EPOCH + self.unix_duration()
    }
}

impl From<&GeneralizedTime> for GeneralizedTime {
    fn from(value: &GeneralizedTime) -> GeneralizedTime {
        *value
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl From<GeneralizedTime> for SystemTime {
    fn from(utc_time: GeneralizedTime) -> SystemTime {
        utc_time.to_system_time()
    }
}

impl TryFrom<Any<'_>> for GeneralizedTime {
    type Error = Error;

    fn try_from(any: Any<'_>) -> Result<GeneralizedTime> {
        any.tag().assert_eq(Self::TAG)?;

        match *any.as_bytes() {
            // RFC 5280 requires mandatory seconds and Z-normalized time zone
            [y1, y2, y3, y4, mon1, mon2, day1, day2, hour1, hour2, min1, min2, sec1, sec2, b'Z'] => {
                let year = datetime::decode_decimal(Self::TAG, y1, y2)? * 100
                    + datetime::decode_decimal(Self::TAG, y3, y4)?;
                let month = datetime::decode_decimal(Self::TAG, mon1, mon2)?;
                let day = datetime::decode_decimal(Self::TAG, day1, day2)?;
                let hour = datetime::decode_decimal(Self::TAG, hour1, hour2)?;
                let minute = datetime::decode_decimal(Self::TAG, min1, min2)?;
                let second = datetime::decode_decimal(Self::TAG, sec1, sec2)?;

                DateTime::new(year, month, day, hour, minute, second)
                    .and_then(|dt| dt.unix_duration())
                    .ok_or_else(|| {
                        ErrorKind::Value {
                            tag: Tag::GeneralizedTime,
                        }
                        .into()
                    })
                    .and_then(Self::new)
            }
            _ => Err(ErrorKind::Value {
                tag: Tag::GeneralizedTime,
            }
            .into()),
        }
    }
}

impl Encodable for GeneralizedTime {
    fn encoded_len(&self) -> Result<Length> {
        Self::LENGTH.for_tlv()
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        Header::new(Self::TAG, Self::LENGTH)?.encode(encoder)?;

        let datetime =
            DateTime::from_unix_duration(self.0).ok_or(ErrorKind::Value { tag: Self::TAG })?;

        let year_hi = datetime.year() / 100;
        let year_lo = datetime.year() % 100;

        datetime::encode_decimal(encoder, Self::TAG, year_hi)?;
        datetime::encode_decimal(encoder, Self::TAG, year_lo)?;
        datetime::encode_decimal(encoder, Self::TAG, datetime.month())?;
        datetime::encode_decimal(encoder, Self::TAG, datetime.day())?;
        datetime::encode_decimal(encoder, Self::TAG, datetime.hour())?;
        datetime::encode_decimal(encoder, Self::TAG, datetime.minute())?;
        datetime::encode_decimal(encoder, Self::TAG, datetime.second())?;
        encoder.byte(b'Z')
    }
}

impl Tagged for GeneralizedTime {
    const TAG: Tag = Tag::GeneralizedTime;
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<'a> TryFrom<Any<'a>> for SystemTime {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<SystemTime> {
        GeneralizedTime::try_from(any).map(|s| s.to_system_time())
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl Encodable for SystemTime {
    fn encoded_len(&self) -> Result<Length> {
        GeneralizedTime::from_system_time(*self)?.encoded_len()
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        GeneralizedTime::from_system_time(*self)?.encode(encoder)
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl Tagged for SystemTime {
    const TAG: Tag = Tag::GeneralizedTime;
}

#[cfg(test)]
mod tests {
    use super::GeneralizedTime;
    use crate::{Decodable, Encodable, Encoder};
    use hex_literal::hex;

    #[test]
    fn round_trip() {
        let example_bytes = hex!("18 0f 31 39 39 31 30 35 30 36 32 33 34 35 34 30 5a");
        let utc_time = GeneralizedTime::from_der(&example_bytes).unwrap();
        assert_eq!(utc_time.unix_duration().as_secs(), 673573540);

        let mut buf = [0u8; 128];
        let mut encoder = Encoder::new(&mut buf);
        utc_time.encode(&mut encoder).unwrap();
        assert_eq!(example_bytes, encoder.finish().unwrap());
    }
}
