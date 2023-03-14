//! Date and time functionality shared between various ASN.1 types
//! (e.g. `GeneralizedTime`, `UTCTime`)

// Adapted from the `humantime` crate.
// Copyright (c) 2016 The humantime Developers
// Released under the MIT OR Apache 2.0 licenses

use crate::{Encoder, ErrorKind, Result, Tag};
use core::time::Duration;

/// Minimum year allowed in [`DateTime`] values.
const MIN_YEAR: u16 = 1970;

/// Maximum duration since `UNIX_EPOCH` which can be represented as a
/// [`DateTime`] (non-inclusive).
const MAX_UNIX_DURATION: Duration = Duration::from_secs(253_402_300_800);

/// Decode 2-digit decimal value
pub(crate) fn decode_decimal(tag: Tag, hi: u8, lo: u8) -> Result<u16> {
    if (b'0'..=b'9').contains(&hi) && (b'0'..=b'9').contains(&lo) {
        Ok((hi - b'0') as u16 * 10 + (lo - b'0') as u16)
    } else {
        Err(ErrorKind::Value { tag }.into())
    }
}

/// Encode 2-digit decimal value
pub(crate) fn encode_decimal(encoder: &mut Encoder<'_>, tag: Tag, value: u16) -> Result<()> {
    let hi_val = value / 10;

    if hi_val >= 10 {
        return Err(ErrorKind::Value { tag }.into());
    }

    encoder.byte(hi_val as u8 + b'0')?;
    encoder.byte((value % 10) as u8 + b'0')
}

/// Inner date/time type shared by multiple ASN.1 types
/// (e.g. `GeneralizedTime`, `UTCTime`).
///
/// Following conventions from RFC 5280, this type is always Z-normalized
/// (i.e. represents a UTC time). However, it isn't named "UTC time" in order
/// to prevent confusion with ASN.1 `UTCTime`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct DateTime {
    /// Full year (e.g. 2000).
    ///
    /// Must be >=1970 to permit positive conversions to Unix time.
    year: u16,

    /// Month (1-12)
    month: u16,

    /// Day of the month (1-31)
    day: u16,

    /// Hour (0-23)
    hour: u16,

    /// Minute (0-59)
    minute: u16,

    /// Second (0-59)
    second: u16,
}

impl DateTime {
    /// Create a new [`DateTime`] from the given UTC time components.
    ///
    /// Note that this does not fully validate the components of the date.
    /// To ensure the date is valid, it must be converted to a Unix timestamp
    /// by calling [`DateTime::unix_timestamp`].
    pub(crate) fn new(
        year: u16,
        month: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
    ) -> Option<Self> {
        // Basic validation of the components.
        if year >= MIN_YEAR
            && (1..=12).contains(&month)
            && (1..=31).contains(&day)
            && (0..=23).contains(&hour)
            && (0..=59).contains(&minute)
            && (0..=59).contains(&second)
        {
            Some(Self {
                year,
                month,
                day,
                hour,
                minute,
                second,
            })
        } else {
            None
        }
    }

    /// Compute a [`DateTime`] from the given [`Duration`] since the `UNIX_EPOCH`.
    ///
    /// Returns `None` if the value is outside the supported date range.
    pub fn from_unix_duration(unix_duration: Duration) -> Option<Self> {
        if unix_duration > MAX_UNIX_DURATION {
            return None;
        }

        let secs_since_epoch = unix_duration.as_secs();

        /// 2000-03-01 (mod 400 year, immediately after Feb 29)
        const LEAPOCH: i64 = 11017;
        const DAYS_PER_400Y: i64 = 365 * 400 + 97;
        const DAYS_PER_100Y: i64 = 365 * 100 + 24;
        const DAYS_PER_4Y: i64 = 365 * 4 + 1;

        let days = (secs_since_epoch / 86400) as i64 - LEAPOCH;
        let secs_of_day = secs_since_epoch % 86400;

        let mut qc_cycles = days / DAYS_PER_400Y;
        let mut remdays = days % DAYS_PER_400Y;

        if remdays < 0 {
            remdays += DAYS_PER_400Y;
            qc_cycles -= 1;
        }

        let mut c_cycles = remdays / DAYS_PER_100Y;
        if c_cycles == 4 {
            c_cycles -= 1;
        }
        remdays -= c_cycles * DAYS_PER_100Y;

        let mut q_cycles = remdays / DAYS_PER_4Y;
        if q_cycles == 25 {
            q_cycles -= 1;
        }
        remdays -= q_cycles * DAYS_PER_4Y;

        let mut remyears = remdays / 365;
        if remyears == 4 {
            remyears -= 1;
        }
        remdays -= remyears * 365;

        let mut year = 2000 + remyears + 4 * q_cycles + 100 * c_cycles + 400 * qc_cycles;

        let months = [31, 30, 31, 30, 31, 31, 30, 31, 30, 31, 31, 29];
        let mut mon = 0;
        for mon_len in months.iter() {
            mon += 1;
            if remdays < *mon_len {
                break;
            }
            remdays -= *mon_len;
        }
        let mday = remdays + 1;
        let mon = if mon + 2 > 12 {
            year += 1;
            mon - 10
        } else {
            mon + 2
        };

        let second = secs_of_day % 60;
        let mins_of_day = secs_of_day / 60;
        let minute = mins_of_day % 60;
        let hour = mins_of_day / 60;

        Self::new(
            year as u16,
            mon,
            mday as u16,
            hour as u16,
            minute as u16,
            second as u16,
        )
    }

    /// Get the year
    pub fn year(&self) -> u16 {
        self.year
    }

    /// Get the month
    pub fn month(&self) -> u16 {
        self.month
    }

    /// Get the day
    pub fn day(&self) -> u16 {
        self.day
    }

    /// Get the hour
    pub fn hour(&self) -> u16 {
        self.hour
    }

    /// Get the minute
    pub fn minute(&self) -> u16 {
        self.minute
    }

    /// Get the second
    pub fn second(&self) -> u16 {
        self.second
    }

    /// Compute [`Duration`] since `UNIX_EPOCH` from the given calendar date.
    pub(crate) fn unix_duration(&self) -> Option<Duration> {
        let leap_years = ((self.year - 1) - 1968) / 4 - ((self.year - 1) - 1900) / 100
            + ((self.year - 1) - 1600) / 400;

        let is_leap_year = self.is_leap_year();

        let (mut ydays, mdays) = match self.month {
            1 => (0, 31),
            2 if is_leap_year => (31, 29),
            2 => (31, 28),
            3 => (59, 31),
            4 => (90, 30),
            5 => (120, 31),
            6 => (151, 30),
            7 => (181, 31),
            8 => (212, 31),
            9 => (243, 30),
            10 => (273, 31),
            11 => (304, 30),
            12 => (334, 31),
            _ => return None,
        };

        if self.day > mdays || self.day == 0 {
            return None;
        }

        ydays += self.day - 1;

        if is_leap_year && self.month > 2 {
            ydays += 1;
        }

        let days = (self.year - 1970) as u64 * 365 + leap_years as u64 + ydays as u64;
        let time = self.second as u64 + (self.minute as u64 * 60) + (self.hour as u64 * 3600);
        Some(Duration::from_secs(time + days * 86400))
    }

    /// Is the year a leap year?
    fn is_leap_year(&self) -> bool {
        self.year % 4 == 0 && (self.year % 100 != 0 || self.year % 400 == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::DateTime;

    /// Ensure a day is OK
    fn is_date_valid(year: u16, month: u16, day: u16, hour: u16, minute: u16, second: u16) -> bool {
        DateTime::new(year, month, day, hour, minute, second)
            .and_then(|dt| dt.unix_duration())
            .is_some()
    }

    #[test]
    fn feb_leap_year_handling() {
        assert!(is_date_valid(2000, 2, 29, 0, 0, 0));
        assert!(!is_date_valid(2001, 2, 29, 0, 0, 0));
        assert!(!is_date_valid(2100, 2, 29, 0, 0, 0));
    }

    #[test]
    fn round_trip() {
        for year in 1970..=2100 {
            for month in 1..=12 {
                let max_day = if month == 2 { 28 } else { 30 };

                for day in 1..=max_day {
                    for hour in 0..=23 {
                        let datetime1 = DateTime::new(year, month, day, hour, 0, 0).unwrap();
                        let unix_duration = datetime1.unix_duration().unwrap();
                        let datetime2 = DateTime::from_unix_duration(unix_duration).unwrap();
                        assert_eq!(datetime1, datetime2);
                    }
                }
            }
        }
    }
}
