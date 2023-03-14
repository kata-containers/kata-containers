use core::convert::TryFrom;
use core::fmt;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct BerClassFromIntError(pub(crate) ());

/// BER Object class of tag
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum Class {
    /// `Universal` class of tags (`0b00`)
    Universal = 0b00,
    /// `Application` class of tags (`0b01`)
    Application = 0b01,
    /// `Context-Specific` class of tags (`0b10`)
    ContextSpecific = 0b10,
    /// `Private` class of tags (`0b11`)
    Private = 0b11,
}

impl Class {
    /// `Universal` class of tags (`0b00`)
    pub const UNIVERSAL: u8 = 0b00;
    /// `Application` class of tags (`0b01`)
    pub const APPLICATION: u8 = 0b01;
    /// `Context-Specific` class of tags (`0b10`)
    pub const CONTEXT_SPECIFIC: u8 = 0b10;
    /// `Private` class of tags (`0b11`)
    pub const PRIVATE: u8 = 0b11;

    pub const fn assert_eq(&self, class: Class) -> Result<(), crate::error::Error> {
        if *self as u8 == class as u8 {
            Ok(())
        } else {
            Err(crate::error::Error::UnexpectedClass {
                expected: Some(class),
                actual: *self,
            })
        }
    }
}

impl fmt::Display for Class {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Class::Universal => "UNIVERSAL",
            Class::Application => "APPLICATION",
            Class::ContextSpecific => "CONTEXT-SPECIFIC",
            Class::Private => "PRIVATE",
        };
        write!(f, "{}", s)
    }
}

impl TryFrom<u8> for Class {
    type Error = BerClassFromIntError;

    #[inline]
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0b00 => Ok(Class::Universal),
            0b01 => Ok(Class::Application),
            0b10 => Ok(Class::ContextSpecific),
            0b11 => Ok(Class::Private),
            _ => Err(BerClassFromIntError(())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn methods_class() {
        let c = Class::Universal;
        assert!(c.assert_eq(Class::Universal).is_ok());
        assert!(c.assert_eq(Class::Private).is_err());

        assert_eq!(Class::Universal.to_string().as_str(), "UNIVERSAL");
        assert_eq!(Class::Application.to_string().as_str(), "APPLICATION");
        assert_eq!(
            Class::ContextSpecific.to_string().as_str(),
            "CONTEXT-SPECIFIC"
        );
        assert_eq!(Class::Private.to_string().as_str(), "PRIVATE");

        assert!(Class::try_from(0b00).is_ok());
        assert!(Class::try_from(0b01).is_ok());
        assert!(Class::try_from(0b10).is_ok());
        assert!(Class::try_from(0b11).is_ok());
        assert!(Class::try_from(4).is_err());
    }
}
