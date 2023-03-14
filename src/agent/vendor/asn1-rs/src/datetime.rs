use crate::{Result, Tag};
use alloc::format;
use alloc::string::ToString;
use core::fmt;
#[cfg(feature = "datetime")]
use time::OffsetDateTime;

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum ASN1TimeZone {
    /// No timezone provided
    Undefined,
    /// Coordinated universal time
    Z,
    /// Local zone, with offset to coordinated universal time
    ///
    /// `(offset_hour, offset_minute)`
    Offset(i8, i8),
}

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct ASN1DateTime {
    pub year: u32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub millisecond: Option<u16>,
    pub tz: ASN1TimeZone,
}

impl ASN1DateTime {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        year: u32,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        millisecond: Option<u16>,
        tz: ASN1TimeZone,
    ) -> Self {
        ASN1DateTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
            millisecond,
            tz,
        }
    }

    #[cfg(feature = "datetime")]
    fn to_time_datetime(
        &self,
    ) -> core::result::Result<OffsetDateTime, time::error::ComponentRange> {
        use std::convert::TryFrom;
        use time::{Date, Month, PrimitiveDateTime, Time, UtcOffset};

        let month = Month::try_from(self.month as u8)?;
        let date = Date::from_calendar_date(self.year as i32, month, self.day as u8)?;
        let time = Time::from_hms_milli(
            self.hour,
            self.minute,
            self.second,
            self.millisecond.unwrap_or(0),
        )?;
        let primitive_date = PrimitiveDateTime::new(date, time);
        let offset = match self.tz {
            ASN1TimeZone::Offset(h, m) => UtcOffset::from_hms(h, m, 0)?,
            ASN1TimeZone::Undefined | ASN1TimeZone::Z => UtcOffset::UTC,
        };
        Ok(primitive_date.assume_offset(offset))
    }

    #[cfg(feature = "datetime")]
    pub fn to_datetime(&self) -> Result<OffsetDateTime> {
        use crate::Error;

        self.to_time_datetime().map_err(|_| Error::InvalidDateTime)
    }
}

impl fmt::Display for ASN1DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fractional = match self.millisecond {
            None => "".to_string(),
            Some(v) => format!(".{}", v),
        };
        write!(
            f,
            "{:04}{:02}{:02}{:02}{:02}{:02}{}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second, fractional,
        )
    }
}

/// Decode 2-digit decimal value
pub(crate) fn decode_decimal(tag: Tag, hi: u8, lo: u8) -> Result<u8> {
    if (b'0'..=b'9').contains(&hi) && (b'0'..=b'9').contains(&lo) {
        Ok((hi - b'0') as u8 * 10 + (lo - b'0') as u8)
    } else {
        Err(tag.invalid_value("expected digit"))
    }
}
