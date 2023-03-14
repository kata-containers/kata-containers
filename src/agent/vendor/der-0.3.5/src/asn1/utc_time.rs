//! ASN.1 `UTCTime` support.

use crate::{
    datetime::{self, DateTime},
    Any, Encodable, Encoder, Error, ErrorKind, Header, Length, Result, Tag, Tagged,
};
use core::{convert::TryFrom, time::Duration};

#[cfg(feature = "std")]
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum duration since `UNIX_EPOCH` which can be represented as a `UTCTime`
/// (non-inclusive) according to RFC 5280 rules.
///
/// This corresponds to the RFC3339 date: `2050-01-01T00:00:00Z`
const MAX_UNIX_DURATION: Duration = Duration::from_secs(2_524_608_000);

/// ASN.1 `UTCTime` type.
///
/// This type implements the validity requirements specified in
/// [RFC 5280 Section 4.1.2.5.1][1], namely:
///
/// > For the purposes of this profile, UTCTime values MUST be expressed in
/// > Greenwich Mean Time (Zulu) and MUST include seconds (i.e., times are
/// > `YYMMDDHHMMSSZ`), even where the number of seconds is zero.  Conforming
/// > systems MUST interpret the year field (`YY`) as follows:
/// >
/// > - Where `YY` is greater than or equal to 50, the year SHALL be
/// >   interpreted as `19YY`; and
/// > - Where `YY` is less than 50, the year SHALL be interpreted as `20YY`.
///
/// [1]: https://tools.ietf.org/html/rfc5280#section-4.1.2.5.1
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct UtcTime(Duration);

impl UtcTime {
    /// Length of an RFC 5280-flavored ASN.1 DER-encoded [`UtcTime`].
    pub const LENGTH: Length = Length::new(13);

    /// Length of an RFC 5280-flavored ASN.1 DER-encoded [`UtcTime`].
    #[deprecated(since = "0.3.3", note = "please use UtcTime::LENGTH")]
    pub const fn length() -> Length {
        Self::LENGTH
    }

    /// Create a new [`UtcTime`] given a [`Duration`] since `UNIX_EPOCH`
    /// (a.k.a. "Unix time")
    pub fn new(unix_duration: Duration) -> Result<Self> {
        if unix_duration < MAX_UNIX_DURATION {
            Ok(Self(unix_duration))
        } else {
            Err(ErrorKind::Value { tag: Tag::UtcTime }.into())
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

impl From<&UtcTime> for UtcTime {
    fn from(value: &UtcTime) -> UtcTime {
        *value
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl From<UtcTime> for SystemTime {
    fn from(utc_time: UtcTime) -> SystemTime {
        utc_time.to_system_time()
    }
}

impl TryFrom<Any<'_>> for UtcTime {
    type Error = Error;

    fn try_from(any: Any<'_>) -> Result<UtcTime> {
        any.tag().assert_eq(Self::TAG)?;

        match *any.as_bytes() {
            // RFC 5280 requires mandatory seconds and Z-normalized time zone
            [year1, year2, mon1, mon2, day1, day2, hour1, hour2, min1, min2, sec1, sec2, b'Z'] => {
                let year = datetime::decode_decimal(Self::TAG, year1, year2)?;
                let month = datetime::decode_decimal(Self::TAG, mon1, mon2)?;
                let day = datetime::decode_decimal(Self::TAG, day1, day2)?;
                let hour = datetime::decode_decimal(Self::TAG, hour1, hour2)?;
                let minute = datetime::decode_decimal(Self::TAG, min1, min2)?;
                let second = datetime::decode_decimal(Self::TAG, sec1, sec2)?;

                // RFC 5280 rules for interpreting the year
                let year = if year >= 50 { year + 1900 } else { year + 2000 };

                DateTime::new(year, month, day, hour, minute, second)
                    .and_then(|dt| dt.unix_duration())
                    .ok_or_else(|| ErrorKind::Value { tag: Tag::UtcTime }.into())
                    .and_then(Self::new)
            }
            _ => Err(ErrorKind::Value { tag: Tag::UtcTime }.into()),
        }
    }
}

impl Encodable for UtcTime {
    fn encoded_len(&self) -> Result<Length> {
        Self::LENGTH.for_tlv()
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        Header::new(Self::TAG, Self::LENGTH)?.encode(encoder)?;

        let datetime =
            DateTime::from_unix_duration(self.0).ok_or(ErrorKind::Value { tag: Self::TAG })?;

        debug_assert!((1950..2050).contains(&datetime.year()));
        datetime::encode_decimal(encoder, Self::TAG, datetime.year() - 1900)?;
        datetime::encode_decimal(encoder, Self::TAG, datetime.month())?;
        datetime::encode_decimal(encoder, Self::TAG, datetime.day())?;
        datetime::encode_decimal(encoder, Self::TAG, datetime.hour())?;
        datetime::encode_decimal(encoder, Self::TAG, datetime.minute())?;
        datetime::encode_decimal(encoder, Self::TAG, datetime.second())?;
        encoder.byte(b'Z')
    }
}

impl Tagged for UtcTime {
    const TAG: Tag = Tag::UtcTime;
}

#[cfg(test)]
mod tests {
    use super::UtcTime;
    use crate::{Decodable, Encodable, Encoder};
    use hex_literal::hex;

    #[test]
    fn round_trip() {
        let example_bytes = hex!("17 0d 39 31 30 35 30 36 32 33 34 35 34 30 5a");
        let utc_time = UtcTime::from_der(&example_bytes).unwrap();
        assert_eq!(utc_time.unix_duration().as_secs(), 673573540);

        let mut buf = [0u8; 128];
        let mut encoder = Encoder::new(&mut buf);
        utc_time.encode(&mut encoder).unwrap();
        assert_eq!(example_bytes, encoder.finish().unwrap());
    }
}
