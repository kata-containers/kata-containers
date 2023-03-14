use asn1_rs::nom::Err;
use asn1_rs::{Error, FromDer, GeneralizedTime, Header, ParseResult, UtcTime};
use der_parser::ber::{Tag, MAX_OBJECT_SIZE};
use std::fmt;
use std::ops::{Add, Sub};
use time::macros::format_description;
use time::{Duration, OffsetDateTime};

use crate::error::{X509Error, X509Result};

/// An ASN.1 timestamp.
#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct ASN1Time(OffsetDateTime);

impl ASN1Time {
    pub(crate) fn from_der_opt(i: &[u8]) -> X509Result<Option<Self>> {
        if i.is_empty() {
            return Ok((i, None));
        }
        match parse_choice_of_time(i) {
            Ok((rem, dt)) => Ok((rem, Some(ASN1Time(dt)))),
            Err(Err::Error(Error::InvalidTag)) | Err(Err::Error(Error::UnexpectedTag { .. })) => {
                Ok((i, None))
            }
            Err(_) => Err(Err::Error(X509Error::InvalidDate)),
        }
    }

    #[inline]
    pub const fn new(dt: OffsetDateTime) -> Self {
        Self(dt)
    }

    #[inline]
    pub const fn to_datetime(&self) -> OffsetDateTime {
        self.0
    }

    /// Makes a new `ASN1Time` from the number of non-leap seconds since Epoch
    pub fn from_timestamp(secs: i64) -> Result<Self, X509Error> {
        let dt = OffsetDateTime::from_unix_timestamp(secs).map_err(|_| X509Error::InvalidDate)?;
        Ok(ASN1Time(dt))
    }

    /// Returns the number of non-leap seconds since January 1, 1970 0:00:00 UTC (aka "UNIX timestamp").
    #[inline]
    pub fn timestamp(&self) -> i64 {
        self.0.unix_timestamp()
    }

    /// Returns a `ASN1Time` which corresponds to the current date.
    #[inline]
    pub fn now() -> Self {
        ASN1Time(OffsetDateTime::now_utc())
    }

    /// Returns an RFC 2822 date and time string such as `Tue, 1 Jul 2003 10:52:37 +0200`.
    ///
    /// Conversion to RFC2822 date can fail if date cannot be represented in this format,
    /// for example if year < 1900.
    ///
    /// For an infallible conversion to string, use `.to_string()`.
    #[inline]
    pub fn to_rfc2822(self) -> Result<String, String> {
        self.0
            .format(&time::format_description::well_known::Rfc2822)
            .map_err(|e| e.to_string())
    }
}

impl<'a> FromDer<'a, X509Error> for ASN1Time {
    fn from_der(i: &[u8]) -> X509Result<Self> {
        let (rem, dt) = parse_choice_of_time(i).map_err(|_| X509Error::InvalidDate)?;
        Ok((rem, ASN1Time(dt)))
    }
}

pub(crate) fn parse_choice_of_time(i: &[u8]) -> ParseResult<OffsetDateTime> {
    if let Ok((rem, t)) = UtcTime::from_der(i) {
        let dt = t.utc_adjusted_datetime()?;
        return Ok((rem, dt));
    }
    if let Ok((rem, t)) = GeneralizedTime::from_der(i) {
        let dt = t.utc_datetime()?;
        return Ok((rem, dt));
    }
    parse_malformed_date(i)
}

// allow relaxed parsing of UTCTime (ex: 370116130016+0000)
fn parse_malformed_date(i: &[u8]) -> ParseResult<OffsetDateTime> {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    // fn check_char(b: &u8) -> bool {
    //     (0x20 <= *b && *b <= 0x7f) || (*b == b'+')
    // }
    let (_rem, hdr) = Header::from_der(i)?;
    let len = hdr.length().definite()?;
    if len > MAX_OBJECT_SIZE {
        return Err(nom::Err::Error(Error::InvalidLength));
    }
    match hdr.tag() {
        Tag::UtcTime => {
            // // if we are in this function, the PrintableString could not be validated.
            // // Accept it without validating charset, because some tools do not respect the charset
            // // restrictions (for ex. they use '*' while explicingly disallowed)
            // let (rem, data) = take(len as usize)(rem)?;
            // if !data.iter().all(check_char) {
            //     return Err(nom::Err::Error(BerError::BerValueError));
            // }
            // let s = std::str::from_utf8(data).map_err(|_| BerError::BerValueError)?;
            // let content = BerObjectContent::UTCTime(s);
            // let obj = DerObject::from_header_and_content(hdr, content);
            // Ok((rem, obj))
            Err(nom::Err::Error(Error::BerValueError))
        }
        _ => Err(nom::Err::Error(Error::unexpected_tag(None, hdr.tag()))),
    }
}

impl fmt::Display for ASN1Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let format = format_description!("[month repr:short] [day padding:space] [hour]:[minute]:[second] [year padding:none] [offset_hour sign:mandatory]:[offset_minute]");
        let s = self
            .0
            .format(format)
            .unwrap_or_else(|e| format!("Invalid date: {}", e));
        f.write_str(&s)
    }
}

impl Add<Duration> for ASN1Time {
    type Output = Option<ASN1Time>;

    #[inline]
    fn add(self, rhs: Duration) -> Option<ASN1Time> {
        Some(ASN1Time(self.0 + rhs))
    }
}

impl Sub<ASN1Time> for ASN1Time {
    type Output = Option<Duration>;

    #[inline]
    fn sub(self, rhs: ASN1Time) -> Option<Duration> {
        if self.0 > rhs.0 {
            Some(self.0 - rhs.0)
        } else {
            None
        }
    }
}

impl From<OffsetDateTime> for ASN1Time {
    fn from(dt: OffsetDateTime) -> Self {
        ASN1Time(dt)
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::ASN1Time;

    #[test]
    fn test_time_to_string() {
        let d = datetime!(1 - 1 - 1 12:34:56 UTC);
        let t = ASN1Time::from(d);
        assert_eq!(t.to_string(), "Jan  1 12:34:56 1 +00:00".to_string());
    }

    #[test]
    fn test_nonrfc2822_date() {
        // test year < 1900
        let d = datetime!(1 - 1 - 1 00:00:00 UTC);
        let t = ASN1Time::from(d);
        assert!(t.to_rfc2822().is_err());
    }
}
