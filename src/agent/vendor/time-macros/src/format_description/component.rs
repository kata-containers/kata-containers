use proc_macro::{Ident, Span, TokenStream};

use crate::format_description::error::InvalidFormatDescription;
use crate::format_description::modifier;
use crate::format_description::modifier::Modifiers;
use crate::to_tokens::ToTokenStream;

pub(crate) enum Component {
    Day(modifier::Day),
    Month(modifier::Month),
    Ordinal(modifier::Ordinal),
    Weekday(modifier::Weekday),
    WeekNumber(modifier::WeekNumber),
    Year(modifier::Year),
    Hour(modifier::Hour),
    Minute(modifier::Minute),
    Period(modifier::Period),
    Second(modifier::Second),
    Subsecond(modifier::Subsecond),
    OffsetHour(modifier::OffsetHour),
    OffsetMinute(modifier::OffsetMinute),
    OffsetSecond(modifier::OffsetSecond),
}

impl ToTokenStream for Component {
    fn append_to(self, ts: &mut TokenStream) {
        let mut mts = TokenStream::new();

        macro_rules! component_name_and_append {
            ($($name:ident)*) => {
                match self {
                    $(Self::$name(modifier) => {
                        modifier.append_to(&mut mts);
                        stringify!($name)
                    })*
                }
            };
        }

        let component = component_name_and_append![
            Day
            Month
            Ordinal
            Weekday
            WeekNumber
            Year
            Hour
            Minute
            Period
            Second
            Subsecond
            OffsetHour
            OffsetMinute
            OffsetSecond
        ];
        let component = Ident::new(component, Span::mixed_site());

        quote_append! { ts
            ::time::format_description::Component::#(component)(#S(mts))
        }
    }
}

pub(crate) enum NakedComponent {
    Day,
    Month,
    Ordinal,
    Weekday,
    WeekNumber,
    Year,
    Hour,
    Minute,
    Period,
    Second,
    Subsecond,
    OffsetHour,
    OffsetMinute,
    OffsetSecond,
}

impl NakedComponent {
    pub(crate) fn parse(
        component_name: &[u8],
        component_index: usize,
    ) -> Result<Self, InvalidFormatDescription> {
        match component_name {
            b"day" => Ok(Self::Day),
            b"month" => Ok(Self::Month),
            b"ordinal" => Ok(Self::Ordinal),
            b"weekday" => Ok(Self::Weekday),
            b"week_number" => Ok(Self::WeekNumber),
            b"year" => Ok(Self::Year),
            b"hour" => Ok(Self::Hour),
            b"minute" => Ok(Self::Minute),
            b"period" => Ok(Self::Period),
            b"second" => Ok(Self::Second),
            b"subsecond" => Ok(Self::Subsecond),
            b"offset_hour" => Ok(Self::OffsetHour),
            b"offset_minute" => Ok(Self::OffsetMinute),
            b"offset_second" => Ok(Self::OffsetSecond),
            b"" => Err(InvalidFormatDescription::MissingComponentName {
                index: component_index,
            }),
            _ => Err(InvalidFormatDescription::InvalidComponentName {
                name: String::from_utf8_lossy(component_name).into_owned(),
                index: component_index,
            }),
        }
    }

    pub(crate) fn attach_modifiers(self, modifiers: Modifiers) -> Component {
        match self {
            Self::Day => Component::Day(modifier::Day {
                padding: modifiers.padding.unwrap_or_default(),
            }),
            Self::Month => Component::Month(modifier::Month {
                padding: modifiers.padding.unwrap_or_default(),
                repr: modifiers.month_repr.unwrap_or_default(),
                case_sensitive: modifiers.case_sensitive.unwrap_or(true),
            }),
            Self::Ordinal => Component::Ordinal(modifier::Ordinal {
                padding: modifiers.padding.unwrap_or_default(),
            }),
            Self::Weekday => Component::Weekday(modifier::Weekday {
                repr: modifiers.weekday_repr.unwrap_or_default(),
                one_indexed: modifiers.weekday_is_one_indexed.unwrap_or(true),
                case_sensitive: modifiers.case_sensitive.unwrap_or(true),
            }),
            Self::WeekNumber => Component::WeekNumber(modifier::WeekNumber {
                padding: modifiers.padding.unwrap_or_default(),
                repr: modifiers.week_number_repr.unwrap_or_default(),
            }),
            Self::Year => Component::Year(modifier::Year {
                padding: modifiers.padding.unwrap_or_default(),
                repr: modifiers.year_repr.unwrap_or_default(),
                iso_week_based: modifiers.year_is_iso_week_based.unwrap_or_default(),
                sign_is_mandatory: modifiers.sign_is_mandatory.unwrap_or_default(),
            }),
            Self::Hour => Component::Hour(modifier::Hour {
                padding: modifiers.padding.unwrap_or_default(),
                is_12_hour_clock: modifiers.hour_is_12_hour_clock.unwrap_or_default(),
            }),
            Self::Minute => Component::Minute(modifier::Minute {
                padding: modifiers.padding.unwrap_or_default(),
            }),
            Self::Period => Component::Period(modifier::Period {
                is_uppercase: modifiers.period_is_uppercase.unwrap_or(true),
                case_sensitive: modifiers.case_sensitive.unwrap_or(true),
            }),
            Self::Second => Component::Second(modifier::Second {
                padding: modifiers.padding.unwrap_or_default(),
            }),
            Self::Subsecond => Component::Subsecond(modifier::Subsecond {
                digits: modifiers.subsecond_digits.unwrap_or_default(),
            }),
            Self::OffsetHour => Component::OffsetHour(modifier::OffsetHour {
                sign_is_mandatory: modifiers.sign_is_mandatory.unwrap_or_default(),
                padding: modifiers.padding.unwrap_or_default(),
            }),
            Self::OffsetMinute => Component::OffsetMinute(modifier::OffsetMinute {
                padding: modifiers.padding.unwrap_or_default(),
            }),
            Self::OffsetSecond => Component::OffsetSecond(modifier::OffsetSecond {
                padding: modifiers.padding.unwrap_or_default(),
            }),
        }
    }
}
