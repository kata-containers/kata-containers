use core::mem;

use proc_macro::{Ident, Span, TokenStream, TokenTree};

use crate::format_description::error::InvalidFormatDescription;
use crate::format_description::helper;
use crate::to_tokens::{ToTokenStream, ToTokenTree};

macro_rules! to_tokens {
    (
        $(#[$struct_attr:meta])*
        $struct_vis:vis struct $struct_name:ident {$(
            $(#[$field_attr:meta])*
            $field_vis:vis $field_name:ident : $field_ty:ty
        ),+ $(,)?}
    ) => {
        $(#[$struct_attr])*
        $struct_vis struct $struct_name {$(
            $(#[$field_attr])*
            $field_vis $field_name: $field_ty
        ),+}

        impl ToTokenTree for $struct_name {
            fn into_token_tree(self) -> TokenTree {
                let mut tokens = TokenStream::new();
                let Self {$($field_name),+} = self;

                quote_append! { tokens
                    let mut value = ::time::format_description::modifier::$struct_name::default();
                };
                $(
                    quote_append!(tokens value.$field_name =);
                    $field_name.append_to(&mut tokens);
                    quote_append!(tokens ;);
                )+
                quote_append!(tokens value);

                proc_macro::TokenTree::Group(proc_macro::Group::new(
                    proc_macro::Delimiter::Brace,
                    tokens,
                ))
            }
        }
    };

    (
        $(#[$enum_attr:meta])*
        $enum_vis:vis enum $enum_name:ident {$(
            $(#[$variant_attr:meta])*
            $variant_name:ident
        ),+ $(,)?}
    ) => {
        $(#[$enum_attr])*
        $enum_vis enum $enum_name {$(
            $(#[$variant_attr])*
            $variant_name
        ),+}

        impl ToTokenStream for $enum_name {
            fn append_to(self, ts: &mut TokenStream) {
                quote_append! { ts
                    ::time::format_description::modifier::$enum_name::
                };
                let name = match self {
                    $(Self::$variant_name => stringify!($variant_name)),+
                };
                ts.extend([TokenTree::Ident(Ident::new(name, Span::mixed_site()))]);
            }
        }
    }
}

to_tokens! {
    pub(crate) struct Day {
        pub(crate) padding: Padding,
    }
}

to_tokens! {
    pub(crate) enum MonthRepr {
        Numerical,
        Long,
        Short,
    }
}

to_tokens! {
    pub(crate) struct Month {
        pub(crate) padding: Padding,
        pub(crate) repr: MonthRepr,
        pub(crate) case_sensitive: bool,
    }
}

to_tokens! {
    pub(crate) struct Ordinal {
        pub(crate) padding: Padding,
    }
}

to_tokens! {
    pub(crate) enum WeekdayRepr {
        Short,
        Long,
        Sunday,
        Monday,
    }
}

to_tokens! {
    pub(crate) struct Weekday {
        pub(crate) repr: WeekdayRepr,
        pub(crate) one_indexed: bool,
        pub(crate) case_sensitive: bool,
    }
}

to_tokens! {
    pub(crate) enum WeekNumberRepr {
        Iso,
        Sunday,
        Monday,
    }
}

to_tokens! {
    pub(crate) struct WeekNumber {
        pub(crate) padding: Padding,
        pub(crate) repr: WeekNumberRepr,
    }
}

to_tokens! {
    pub(crate) enum YearRepr {
        Full,
        LastTwo,
    }
}

to_tokens! {
    pub(crate) struct Year {
        pub(crate) padding: Padding,
        pub(crate) repr: YearRepr,
        pub(crate) iso_week_based: bool,
        pub(crate) sign_is_mandatory: bool,
    }
}

to_tokens! {
    pub(crate) struct Hour {
        pub(crate) padding: Padding,
        pub(crate) is_12_hour_clock: bool,
    }
}

to_tokens! {
    pub(crate) struct Minute {
        pub(crate) padding: Padding,
    }
}

to_tokens! {
    pub(crate) struct Period {
        pub(crate) is_uppercase: bool,
        pub(crate) case_sensitive: bool,
    }
}

to_tokens! {
    pub(crate) struct Second {
        pub(crate) padding: Padding,
    }
}

to_tokens! {
    pub(crate) enum SubsecondDigits {
        One,
        Two,
        Three,
        Four,
        Five,
        Six,
        Seven,
        Eight,
        Nine,
        OneOrMore,
    }
}

to_tokens! {
    pub(crate) struct Subsecond {
        pub(crate) digits: SubsecondDigits,
    }
}

to_tokens! {
    pub(crate) struct OffsetHour {
        pub(crate) sign_is_mandatory: bool,
        pub(crate) padding: Padding,
    }
}

to_tokens! {
    pub(crate) struct OffsetMinute {
        pub(crate) padding: Padding,
    }
}

to_tokens! {
    pub(crate) struct OffsetSecond {
        pub(crate) padding: Padding,
    }
}

to_tokens! {
    pub(crate) enum Padding {
        Space,
        Zero,
        None,
    }
}

macro_rules! impl_default {
    ($($type:ty => $default:expr;)*) => {$(
        impl Default for $type {
            fn default() -> Self {
                $default
            }
        }
    )*};
}

impl_default! {
    Day => Self { padding: Padding::default() };
    MonthRepr => Self::Numerical;
    Month => Self {
        padding: Padding::default(),
        repr: MonthRepr::default(),
        case_sensitive: true,
    };
    Ordinal => Self { padding: Padding::default() };
    WeekdayRepr => Self::Long;
    Weekday => Self {
        repr: WeekdayRepr::default(),
        one_indexed: true,
        case_sensitive: true,
    };
    WeekNumberRepr => Self::Iso;
    WeekNumber => Self {
        padding: Padding::default(),
        repr: WeekNumberRepr::default(),
    };
    YearRepr => Self::Full;
    Year => Self {
        padding: Padding::default(),
        repr: YearRepr::default(),
        iso_week_based: false,
        sign_is_mandatory: false,
    };
    Hour => Self {
        padding: Padding::default(),
        is_12_hour_clock: false,
    };
    Minute => Self { padding: Padding::default() };
    Period => Self { is_uppercase: true, case_sensitive: true };
    Second => Self { padding: Padding::default() };
    SubsecondDigits => Self::OneOrMore;
    Subsecond => Self { digits: SubsecondDigits::default() };
    OffsetHour => Self {
        sign_is_mandatory: true,
        padding: Padding::default(),
    };
    OffsetMinute => Self { padding: Padding::default() };
    OffsetSecond => Self { padding: Padding::default() };
    Padding => Self::Zero;
}

#[derive(Default)]
pub(crate) struct Modifiers {
    pub(crate) padding: Option<Padding>,
    pub(crate) hour_is_12_hour_clock: Option<bool>,
    pub(crate) period_is_uppercase: Option<bool>,
    pub(crate) month_repr: Option<MonthRepr>,
    pub(crate) subsecond_digits: Option<SubsecondDigits>,
    pub(crate) weekday_repr: Option<WeekdayRepr>,
    pub(crate) weekday_is_one_indexed: Option<bool>,
    pub(crate) week_number_repr: Option<WeekNumberRepr>,
    pub(crate) year_repr: Option<YearRepr>,
    pub(crate) year_is_iso_week_based: Option<bool>,
    pub(crate) sign_is_mandatory: Option<bool>,
    pub(crate) case_sensitive: Option<bool>,
}

impl Modifiers {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn parse(
        component_name: &[u8],
        mut bytes: &[u8],
        index: &mut usize,
    ) -> Result<Self, InvalidFormatDescription> {
        let mut modifiers = Self::default();

        while !bytes.is_empty() {
            // Trim any whitespace between modifiers.
            bytes = helper::consume_whitespace(bytes, index);

            let modifier;
            if let Some(whitespace_loc) = bytes.iter().position(u8::is_ascii_whitespace) {
                *index += whitespace_loc;
                modifier = &bytes[..whitespace_loc];
                bytes = &bytes[whitespace_loc..];
            } else {
                modifier = mem::take(&mut bytes);
            }

            if modifier.is_empty() {
                break;
            }

            match (component_name, modifier) {
                (
                    b"day" | b"hour" | b"minute" | b"month" | b"offset_hour" | b"offset_minute"
                    | b"offset_second" | b"ordinal" | b"second" | b"week_number" | b"year",
                    b"padding:space",
                ) => modifiers.padding = Some(Padding::Space),
                (
                    b"day" | b"hour" | b"minute" | b"month" | b"offset_hour" | b"offset_minute"
                    | b"offset_second" | b"ordinal" | b"second" | b"week_number" | b"year",
                    b"padding:zero",
                ) => modifiers.padding = Some(Padding::Zero),
                (
                    b"day" | b"hour" | b"minute" | b"month" | b"offset_hour" | b"offset_minute"
                    | b"offset_second" | b"ordinal" | b"second" | b"week_number" | b"year",
                    b"padding:none",
                ) => modifiers.padding = Some(Padding::None),
                (b"hour", b"repr:24") => modifiers.hour_is_12_hour_clock = Some(false),
                (b"hour", b"repr:12") => modifiers.hour_is_12_hour_clock = Some(true),
                (b"month" | b"period" | b"weekday", b"case_sensitive:true") => {
                    modifiers.case_sensitive = Some(true)
                }
                (b"month" | b"period" | b"weekday", b"case_sensitive:false") => {
                    modifiers.case_sensitive = Some(false)
                }
                (b"month", b"repr:numerical") => modifiers.month_repr = Some(MonthRepr::Numerical),
                (b"month", b"repr:long") => modifiers.month_repr = Some(MonthRepr::Long),
                (b"month", b"repr:short") => modifiers.month_repr = Some(MonthRepr::Short),
                (b"offset_hour" | b"year", b"sign:automatic") => {
                    modifiers.sign_is_mandatory = Some(false);
                }
                (b"offset_hour" | b"year", b"sign:mandatory") => {
                    modifiers.sign_is_mandatory = Some(true);
                }
                (b"period", b"case:upper") => modifiers.period_is_uppercase = Some(true),
                (b"period", b"case:lower") => modifiers.period_is_uppercase = Some(false),
                (b"subsecond", b"digits:1") => {
                    modifiers.subsecond_digits = Some(SubsecondDigits::One);
                }
                (b"subsecond", b"digits:2") => {
                    modifiers.subsecond_digits = Some(SubsecondDigits::Two);
                }
                (b"subsecond", b"digits:3") => {
                    modifiers.subsecond_digits = Some(SubsecondDigits::Three);
                }
                (b"subsecond", b"digits:4") => {
                    modifiers.subsecond_digits = Some(SubsecondDigits::Four);
                }
                (b"subsecond", b"digits:5") => {
                    modifiers.subsecond_digits = Some(SubsecondDigits::Five);
                }
                (b"subsecond", b"digits:6") => {
                    modifiers.subsecond_digits = Some(SubsecondDigits::Six);
                }
                (b"subsecond", b"digits:7") => {
                    modifiers.subsecond_digits = Some(SubsecondDigits::Seven);
                }
                (b"subsecond", b"digits:8") => {
                    modifiers.subsecond_digits = Some(SubsecondDigits::Eight);
                }
                (b"subsecond", b"digits:9") => {
                    modifiers.subsecond_digits = Some(SubsecondDigits::Nine);
                }
                (b"subsecond", b"digits:1+") => {
                    modifiers.subsecond_digits = Some(SubsecondDigits::OneOrMore);
                }
                (b"weekday", b"repr:short") => modifiers.weekday_repr = Some(WeekdayRepr::Short),
                (b"weekday", b"repr:long") => modifiers.weekday_repr = Some(WeekdayRepr::Long),
                (b"weekday", b"repr:sunday") => modifiers.weekday_repr = Some(WeekdayRepr::Sunday),
                (b"weekday", b"repr:monday") => modifiers.weekday_repr = Some(WeekdayRepr::Monday),
                (b"weekday", b"one_indexed:true") => modifiers.weekday_is_one_indexed = Some(true),
                (b"weekday", b"one_indexed:false") => {
                    modifiers.weekday_is_one_indexed = Some(false);
                }
                (b"week_number", b"repr:iso") => {
                    modifiers.week_number_repr = Some(WeekNumberRepr::Iso);
                }
                (b"week_number", b"repr:sunday") => {
                    modifiers.week_number_repr = Some(WeekNumberRepr::Sunday);
                }
                (b"week_number", b"repr:monday") => {
                    modifiers.week_number_repr = Some(WeekNumberRepr::Monday);
                }
                (b"year", b"repr:full") => modifiers.year_repr = Some(YearRepr::Full),
                (b"year", b"repr:last_two") => modifiers.year_repr = Some(YearRepr::LastTwo),
                (b"year", b"base:calendar") => modifiers.year_is_iso_week_based = Some(false),
                (b"year", b"base:iso_week") => modifiers.year_is_iso_week_based = Some(true),
                _ => {
                    return Err(InvalidFormatDescription::InvalidModifier {
                        value: String::from_utf8_lossy(modifier).into_owned(),
                        index: *index,
                    });
                }
            }
        }

        Ok(modifiers)
    }
}
