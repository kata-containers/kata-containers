use std::fmt;

pub(crate) enum InvalidFormatDescription {
    UnclosedOpeningBracket { index: usize },
    InvalidComponentName { name: String, index: usize },
    InvalidModifier { value: String, index: usize },
    MissingComponentName { index: usize },
}

impl fmt::Display for InvalidFormatDescription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[allow(clippy::enum_glob_use)]
        use InvalidFormatDescription::*;
        match self {
            UnclosedOpeningBracket { index } => {
                write!(f, "unclosed opening bracket at byte index {index}")
            }
            InvalidComponentName { name, index } => {
                write!(f, "invalid component name `{name}` at byte index {index}",)
            }
            InvalidModifier { value, index } => {
                write!(f, "invalid modifier `{value}` at byte index {index}")
            }
            MissingComponentName { index } => {
                write!(f, "missing component name at byte index {index}")
            }
        }
    }
}
