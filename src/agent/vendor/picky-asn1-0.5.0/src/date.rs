use serde::{de, ser, Deserializer, Serializer};
use std::fmt;

pub trait TimeRepr
where
    Self: Sized,
{
    fn serialize<S>(
        date: &Date<Self>,
        serializer: S,
    ) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
    where
        S: ser::Serializer;

    fn deserialize<'de, D>(deserializer: D) -> Result<Date<Self>, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>;
}

/// A basic Date struct.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date<TR: TimeRepr> {
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    _pd: std::marker::PhantomData<TR>,
}

impl<TR: TimeRepr> Date<TR> {
    /// Create a new Date without validation.
    ///
    /// # Safety
    ///
    /// You have to make sure you're not building an invalid date.
    pub unsafe fn new_unchecked(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Date<TR> {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            _pd: std::marker::PhantomData,
        }
    }

    pub fn new(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Option<Date<TR>> {
        if (1..=12).contains(&month) && (1..=32).contains(&day) && hour < 24 && minute < 60 && second < 60 {
            Some(Self {
                year,
                month,
                day,
                hour,
                minute,
                second,
                _pd: std::marker::PhantomData,
            })
        } else {
            None
        }
    }

    pub fn year(&self) -> u16 {
        self.year
    }

    pub fn month(&self) -> u8 {
        self.month
    }

    pub fn day(&self) -> u8 {
        self.day
    }

    pub fn hour(&self) -> u8 {
        self.hour
    }

    pub fn minute(&self) -> u8 {
        self.minute
    }

    pub fn second(&self) -> u8 {
        self.second
    }
}

impl<TR: TimeRepr> ser::Serialize for Date<TR> {
    fn serialize<S>(&self, serializer: S) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
    where
        S: ser::Serializer,
    {
        TR::serialize(self, serializer)
    }
}

impl<'de, TR: TimeRepr> de::Deserialize<'de> for Date<TR> {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        TR::deserialize(deserializer)
    }
}

trait DateDigitReader {
    fn read_digit(&self, idx: usize) -> u8;

    #[inline]
    fn read_and_merge_with_next(&self, idx: usize) -> u8 {
        self.read_digit(idx) * 10 + self.read_digit(idx + 1)
    }
}

impl DateDigitReader for [u8] {
    #[inline]
    fn read_digit(&self, idx: usize) -> u8 {
        self[idx] & 0x0F
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct UTCTimeRepr;
pub type UTCTime = Date<UTCTimeRepr>;

impl TimeRepr for UTCTimeRepr {
    fn serialize<S>(date: &Date<UTCTimeRepr>, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: ser::Serializer,
    {
        let mut encoded = [
            0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x5A,
        ];

        let year = if date.year() >= 2000 {
            date.year() - 2000
        } else {
            date.year() - 1900
        };

        encoded[0] |= (year / 10) as u8;
        encoded[1] |= (year % 10) as u8;
        encoded[2] |= date.month() / 10;
        encoded[3] |= date.month() % 10;
        encoded[4] |= date.day() / 10;
        encoded[5] |= date.day() % 10;
        encoded[6] |= date.hour() / 10;
        encoded[7] |= date.hour() % 10;
        encoded[8] |= date.minute() / 10;
        encoded[9] |= date.minute() % 10;
        encoded[10] |= date.second() / 10;
        encoded[11] |= date.second() % 10;

        serializer.serialize_bytes(&encoded)
    }

    fn deserialize<'de, D>(deserializer: D) -> Result<Date<UTCTimeRepr>, <D as Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Date<UTCTimeRepr>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid buffer representing an Asn1 UTCTime")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if v.len() != 13 {
                    return Err(E::invalid_value(
                        de::Unexpected::Other("unsupported date format"),
                        &"a valid buffer representing an Asn1 UTCTime (exactly 13 bytes required)",
                    ));
                }

                let yyyy = {
                    let yy = v.read_and_merge_with_next(0) as u16;
                    if yy >= 50 {
                        1900 + yy
                    } else {
                        2000 + yy
                    }
                };
                let month = v.read_and_merge_with_next(2);
                let day = v.read_and_merge_with_next(4);
                let hour = v.read_and_merge_with_next(6);
                let minute = v.read_and_merge_with_next(8);
                let second = v.read_and_merge_with_next(10);
                let dt = Date::new(yyyy, month, day, hour, minute, second).ok_or_else(|| {
                    E::invalid_value(
                        de::Unexpected::Other("invalid parameters provided to Date constructor"),
                        &"valid parameters for Date",
                    )
                })?;

                Ok(dt)
            }
        }

        deserializer.deserialize_bytes(Visitor)
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct GeneralizedTimeRepr;
pub type GeneralizedTime = Date<GeneralizedTimeRepr>;

impl TimeRepr for GeneralizedTimeRepr {
    fn serialize<S>(
        date: &Date<GeneralizedTimeRepr>,
        serializer: S,
    ) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: ser::Serializer,
    {
        let mut encoded = [
            0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x5A,
        ];

        encoded[0] |= (date.year() / 1000) as u8;
        encoded[1] |= ((date.year() % 1000) / 100) as u8;
        encoded[2] |= ((date.year() % 100) / 10) as u8;
        encoded[3] |= (date.year() % 10) as u8;
        encoded[4] |= date.month() / 10;
        encoded[5] |= date.month() % 10;
        encoded[6] |= date.day() / 10;
        encoded[7] |= date.day() % 10;
        encoded[8] |= date.hour() / 10;
        encoded[9] |= date.hour() % 10;
        encoded[10] |= date.minute() / 10;
        encoded[11] |= date.minute() % 10;
        encoded[12] |= date.second() / 10;
        encoded[13] |= date.second() % 10;

        serializer.serialize_bytes(&encoded)
    }

    fn deserialize<'de, D>(deserializer: D) -> Result<Date<GeneralizedTimeRepr>, <D as Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Date<GeneralizedTimeRepr>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid buffer representing an Asn1 GeneralizedTime")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if v.len() != 15 {
                    return Err(E::invalid_value(
                        de::Unexpected::Other("unsupported date format"),
                        &"a valid buffer representing an Asn1 GeneralizedTime (exactly 15 bytes required)",
                    ));
                }

                let yyyy = v.read_and_merge_with_next(0) as u16 * 100 + v.read_and_merge_with_next(2) as u16;
                let month = v.read_and_merge_with_next(4);
                let day = v.read_and_merge_with_next(6);
                let hour = v.read_and_merge_with_next(8);
                let minute = v.read_and_merge_with_next(10);
                let second = v.read_and_merge_with_next(12);
                let dt = Date::new(yyyy, month, day, hour, minute, second).ok_or_else(|| {
                    E::invalid_value(
                        de::Unexpected::Other("invalid parameters provided to Date constructor"),
                        &"valid parameters for Date",
                    )
                })?;

                Ok(dt)
            }
        }

        deserializer.deserialize_bytes(Visitor)
    }
}

#[cfg(feature = "time_conversion")]
mod time_convert {
    use super::*;
    use time::{OffsetDateTime, PrimitiveDateTime};

    impl<TR: TimeRepr> From<PrimitiveDateTime> for Date<TR> {
        fn from(d: PrimitiveDateTime) -> Self {
            Self::from(d.assume_utc())
        }
    }

    impl<TR: TimeRepr> TryFrom<Date<TR>> for PrimitiveDateTime {
        type Error = time::error::ComponentRange;

        fn try_from(d: Date<TR>) -> Result<Self, Self::Error> {
            let date = time::Date::from_calendar_date(i32::from(d.year), time::Month::try_from(d.month)?, d.day)?;
            let time = time::Time::from_hms(d.hour, d.minute, d.second)?;
            Ok(Self::new(date, time))
        }
    }

    impl<TR: TimeRepr> From<OffsetDateTime> for Date<TR> {
        fn from(d: OffsetDateTime) -> Self {
            Self {
                year: u16::try_from(d.year()).unwrap(),
                month: u8::from(d.month()),
                day: d.day(),
                hour: d.hour(),
                minute: d.minute(),
                second: d.second(),
                _pd: std::marker::PhantomData,
            }
        }
    }

    impl<TR: TimeRepr> TryFrom<Date<TR>> for OffsetDateTime {
        type Error = time::error::ComponentRange;

        fn try_from(d: Date<TR>) -> Result<Self, Self::Error> {
            Ok(PrimitiveDateTime::try_from(d)?.assume_utc())
        }
    }
}

#[cfg(feature = "chrono_conversion")]
mod chrono_convert {
    use super::*;
    use chrono::naive::NaiveDateTime;
    use chrono::{DateTime, Datelike, NaiveDate, Timelike, Utc};

    impl<TR: TimeRepr> From<NaiveDateTime> for Date<TR> {
        fn from(d: NaiveDateTime) -> Self {
            Self {
                year: u16::try_from(d.year()).unwrap(),
                month: u8::try_from(d.month()).unwrap(),
                day: u8::try_from(d.day()).unwrap(),
                hour: u8::try_from(d.hour()).unwrap(),
                minute: u8::try_from(d.minute()).unwrap(),
                second: u8::try_from(d.second()).unwrap(),
                _pd: std::marker::PhantomData,
            }
        }
    }

    impl<TR: TimeRepr> From<Date<TR>> for NaiveDateTime {
        fn from(date: Date<TR>) -> Self {
            NaiveDate::from_ymd(i32::from(date.year), u32::from(date.month), u32::from(date.day)).and_hms(
                u32::from(date.hour),
                u32::from(date.minute),
                u32::from(date.second),
            )
        }
    }

    impl<TR: TimeRepr> From<DateTime<Utc>> for Date<TR> {
        fn from(d: DateTime<Utc>) -> Self {
            Self {
                year: u16::try_from(d.year()).unwrap(),
                month: u8::try_from(d.month()).unwrap(),
                day: u8::try_from(d.day()).unwrap(),
                hour: u8::try_from(d.hour()).unwrap(),
                minute: u8::try_from(d.minute()).unwrap(),
                second: u8::try_from(d.second()).unwrap(),
                _pd: std::marker::PhantomData,
            }
        }
    }

    impl<TR: TimeRepr> From<Date<TR>> for DateTime<Utc> {
        fn from(date: Date<TR>) -> Self {
            DateTime::<Utc>::from_utc(date.into(), Utc)
        }
    }
}
