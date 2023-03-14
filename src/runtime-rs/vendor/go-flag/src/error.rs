use std::fmt;

/// Common errors for flag parsing.
#[derive(Debug)]
pub enum FlagError {
    /// Bad flag syntax. E.g. `--=foo`
    BadFlag { flag: String },
    /// Flag provided but not defined.
    UnknownFlag { name: String },
    /// Flag needs an argument.
    ArgumentNeeded { name: String },
    /// Failed to parse a flag argument. E.g. `--lines=10XYZ`
    ParseError { error: FlagParseError },
}

impl fmt::Display for FlagError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use FlagError::*;
        match self {
            BadFlag { flag } => write!(f, "bad flag syntax: {}", flag),
            UnknownFlag { name } => write!(f, "flag provided but not defined: -{}", name),
            ArgumentNeeded { name } => write!(f, "flag needs an argument: -{}", name),
            ParseError { .. } => write!(f, "parse error"),
        }
    }
}

impl std::error::Error for FlagError {
    fn description(&self) -> &str {
        use FlagError::*;
        match self {
            BadFlag { .. } => "bad flag syntax",
            UnknownFlag { .. } => "flag provided but not defined",
            ArgumentNeeded { .. } => "flag needs an argument",
            ParseError { .. } => "parse error",
        }
    }

    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        use FlagError::*;
        match self {
            BadFlag { .. } => None,
            UnknownFlag { .. } => None,
            ArgumentNeeded { .. } => None,
            ParseError { error } => Some(error),
        }
    }
}

/// Common warnings for flag parsing.
#[derive(Debug)]
pub enum FlagWarning {
    /// Flag-like syntax appearing after argument.
    FlagAfterArg { flag: String },
    /// Long flag with single minus. E.g. `-lines`
    ShortLong { flag: String },
    /// Short flag with double minus. E.g. `--f`
    LongShort { flag: String },
    /// Nonstandard value format for flag argument. E.g. `--lines=0x10`
    FlagValue { value: String },
}

impl fmt::Display for FlagWarning {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use FlagWarning::*;
        match self {
            FlagAfterArg { flag } => {
                write!(f, "flag-like syntax appearing after argument: {}", flag)
            }
            ShortLong { flag } => write!(f, "long flag with single minus: {}", flag),
            LongShort { flag } => write!(f, "short flag with double minuses: {}", flag),
            FlagValue { value } => write!(f, "nonstandard value format: {}", value),
        }
    }
}

impl std::error::Error for FlagWarning {
    fn description(&self) -> &str {
        use FlagWarning::*;
        match self {
            FlagAfterArg { .. } => "flag-like syntax appearing after argument",
            ShortLong { .. } => "long flag with single minus",
            LongShort { .. } => "short flag with double minuses",
            FlagValue { .. } => "nonstandard value format: {}",
        }
    }
}

/// Common errors for flag argument parsing.
#[derive(Debug)]
pub enum FlagParseError {
    /// Invalid bool. E.g. `yes`
    BoolParseError,
    /// Invalid integer. E.g. `100XYZ`
    IntegerParseError,
    /// Invalid string. Invalid UTF-8 in unix-like platform or invalid UTF-16 in Windows.
    StringParseError,
}

impl fmt::Display for FlagParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use FlagParseError::*;
        match self {
            BoolParseError => write!(f, "invalid bool"),
            IntegerParseError => write!(f, "invalid integer"),
            StringParseError => write!(f, "invalid unicode string"),
        }
    }
}

impl std::error::Error for FlagParseError {
    fn description(&self) -> &str {
        use FlagParseError::*;
        match self {
            BoolParseError => "invalid bool",
            IntegerParseError => "invalid integer",
            StringParseError => "invalid unicode string",
        }
    }
}
