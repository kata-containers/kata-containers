use crate::datetime::decode_decimal;
use crate::*;
use core::convert::TryFrom;
use core::fmt;
#[cfg(feature = "datetime")]
use time::OffsetDateTime;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UtcTime(pub ASN1DateTime);

impl UtcTime {
    pub const fn new(datetime: ASN1DateTime) -> Self {
        UtcTime(datetime)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        // X.680 section 43 defines a UniversalTime as a VisibleString restricted to:
        //
        // a) the six digits YYMMDD where YY is the two low-order digits of the Christian year, MM is the month
        // (counting January as 01), and DD is the day of the month (01 to 31); and
        // b) either:
        //   1) the four digits hhmm where hh is hour (00 to 23) and mm is minutes (00 to 59); or
        //   2) the six digits hhmmss where hh and mm are as in 1) above, and ss is seconds (00 to 59); and
        // c) either:
        //   1) the character Z ; or
        //   2) one of the characters + or - , followed by hhmm, where hh is hour and mm is minutes.
        //
        // XXX // RFC 5280 requires mandatory seconds and Z-normalized time zone
        let (year, month, day, hour, minute, rem) = match bytes {
            [year1, year2, mon1, mon2, day1, day2, hour1, hour2, min1, min2, rem @ ..] => {
                let year = decode_decimal(Self::TAG, *year1, *year2)?;
                let month = decode_decimal(Self::TAG, *mon1, *mon2)?;
                let day = decode_decimal(Self::TAG, *day1, *day2)?;
                let hour = decode_decimal(Self::TAG, *hour1, *hour2)?;
                let minute = decode_decimal(Self::TAG, *min1, *min2)?;
                (year, month, day, hour, minute, rem)
            }
            _ => return Err(Self::TAG.invalid_value("malformed time string (not yymmddhhmm)")),
        };
        if rem.is_empty() {
            return Err(Self::TAG.invalid_value("malformed time string"));
        }
        // check for seconds
        let (second, rem) = match rem {
            [sec1, sec2, rem @ ..] => {
                let second = decode_decimal(Self::TAG, *sec1, *sec2)?;
                (second, rem)
            }
            _ => (0, rem),
        };
        if month > 12 || day > 31 || hour > 23 || minute > 59 || second > 59 {
            return Err(Self::TAG.invalid_value("time components with invalid values"));
        }
        if rem.is_empty() {
            return Err(Self::TAG.invalid_value("malformed time string"));
        }
        let tz = match rem {
            [b'Z'] => ASN1TimeZone::Z,
            [b'+', h1, h2, m1, m2] => {
                let hh = decode_decimal(Self::TAG, *h1, *h2)?;
                let mm = decode_decimal(Self::TAG, *m1, *m2)?;
                ASN1TimeZone::Offset(hh as i8, mm as i8)
            }
            [b'-', h1, h2, m1, m2] => {
                let hh = decode_decimal(Self::TAG, *h1, *h2)?;
                let mm = decode_decimal(Self::TAG, *m1, *m2)?;
                ASN1TimeZone::Offset(-(hh as i8), mm as i8)
            }
            _ => return Err(Self::TAG.invalid_value("malformed time string: no time zone")),
        };
        Ok(UtcTime(ASN1DateTime::new(
            year as u32,
            month,
            day,
            hour,
            minute,
            second,
            None,
            tz,
        )))
        // match *bytes {
        //     [year1, year2, mon1, mon2, day1, day2, hour1, hour2, min1, min2, sec1, sec2, b'Z'] => {
        //         let year = decode_decimal(Self::TAG, year1, year2)?;
        //         let month = decode_decimal(Self::TAG, mon1, mon2)?;
        //         let day = decode_decimal(Self::TAG, day1, day2)?;
        //         let hour = decode_decimal(Self::TAG, hour1, hour2)?;
        //         let minute = decode_decimal(Self::TAG, min1, min2)?;
        //         let second = decode_decimal(Self::TAG, sec1, sec2)?;

        //         // RFC 5280 rules for interpreting the year
        //         let year = if year >= 50 { year + 1900 } else { year + 2000 };

        //         Ok(UtcTime::new(year, month, day, hour, minute, second))
        //     }
        //     _ => Err(Error::InvalidValue),
        // }
    }

    /// Return a ISO 8601 combined date and time with time zone.
    #[cfg(feature = "datetime")]
    #[cfg_attr(docsrs, doc(cfg(feature = "datetime")))]
    #[inline]
    pub fn utc_datetime(&self) -> Result<OffsetDateTime> {
        self.0.to_datetime()
    }

    /// Return an adjusted ISO 8601 combined date and time with time zone.
    /// According to Universal time definition in X.680 we add 2000 years
    /// from 0 to 49 year and 1900 otherwise.
    #[cfg(feature = "datetime")]
    #[cfg_attr(docsrs, doc(cfg(feature = "datetime")))]
    #[inline]
    pub fn utc_adjusted_datetime(&self) -> Result<OffsetDateTime> {
        self.0.to_datetime().and_then(|dt| {
            let year = dt.year();
            // We follow the Universal time definition in X.680 for interpreting
            // the adjusted year
            let year = if year >= 50 { year + 1900 } else { year + 2000 };
            time::Date::from_calendar_date(year, dt.month(), dt.day())
                .map(|d| dt.replace_date(d))
                .map_err(|_e| Self::TAG.invalid_value("Invalid adjusted date"))
        })
    }

    /// Returns the number of non-leap seconds since the midnight on January 1, 1970.
    #[cfg(feature = "datetime")]
    #[cfg_attr(docsrs, doc(cfg(feature = "datetime")))]
    pub fn timestamp(&self) -> Result<i64> {
        let dt = self.0.to_datetime()?;
        Ok(dt.unix_timestamp())
    }
}

impl<'a> TryFrom<Any<'a>> for UtcTime {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<UtcTime> {
        TryFrom::try_from(&any)
    }
}

impl<'a, 'b> TryFrom<&'b Any<'a>> for UtcTime {
    type Error = Error;

    fn try_from(any: &'b Any<'a>) -> Result<UtcTime> {
        any.tag().assert_eq(Self::TAG)?;
        #[allow(clippy::trivially_copy_pass_by_ref)]
        fn is_visible(b: &u8) -> bool {
            0x20 <= *b && *b <= 0x7f
        }
        if !any.data.iter().all(is_visible) {
            return Err(Error::StringInvalidCharset);
        }

        UtcTime::from_bytes(any.data)
    }
}

impl fmt::Display for UtcTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dt = &self.0;
        match dt.tz {
            ASN1TimeZone::Z | ASN1TimeZone::Undefined => write!(
                f,
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}Z",
                dt.year, dt.month, dt.day, dt.hour, dt.minute, dt.second
            ),
            ASN1TimeZone::Offset(hh, mm) => {
                let (s, hh) = if hh > 0 { ('+', hh) } else { ('-', -hh) };
                write!(
                    f,
                    "{:04}-{:02}-{:02} {:02}:{:02}:{:02}{}{:02}{:02}",
                    dt.year, dt.month, dt.day, dt.hour, dt.minute, dt.second, s, hh, mm
                )
            }
        }
    }
}

impl CheckDerConstraints for UtcTime {
    fn check_constraints(_any: &Any) -> Result<()> {
        Ok(())
    }
}

impl DerAutoDerive for UtcTime {}

impl Tagged for UtcTime {
    const TAG: Tag = Tag::UtcTime;
}

#[cfg(feature = "std")]
impl ToDer for UtcTime {
    fn to_der_len(&self) -> Result<usize> {
        // data:
        // - 6 bytes for YYMMDD
        // - 6 for hhmmss in DER (X.690 section 11.8.2)
        // - 1 for the character Z in DER (X.690 section 11.8.1)
        // data length: 13
        //
        // thus, length will always be on 1 byte (short length) and
        // class+structure+tag also on 1
        //
        // total: 15 = 1 (class+constructed+tag) + 1 (length) + 13
        Ok(15)
    }

    fn write_der_header(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        // see above for length value
        writer.write(&[Self::TAG.0 as u8, 13]).map_err(Into::into)
    }

    fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        let _ = write!(
            writer,
            "{:02}{:02}{:02}{:02}{:02}{:02}Z",
            self.0.year, self.0.month, self.0.day, self.0.hour, self.0.minute, self.0.second,
        )?;
        // write_fmt returns (), see above for length value
        Ok(13)
    }
}
