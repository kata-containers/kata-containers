use std::fmt;

use crate::util::der::DerClass;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerType {
    EndOfContents,
    Boolean,
    Integer,
    BitString,
    OctetString,
    Null,
    ObjectIdentifier,
    ObjectDescriptor,
    External,
    Real,
    Enumerated,
    EmbeddedPdv,
    Utf8String,
    RelativeOid,
    Time,
    Sequence,
    Set,
    NumericString,
    PrintableString,
    TeletexString,
    VideotexString,
    Ia5String,
    UtcTime,
    GeneralizedTime,
    GraphicString,
    VisibleString,
    GeneralString,
    UniversalString,
    CharacterString,
    BmpString,
    Date,
    TimeOfDay,
    DateTime,
    Duration,
    Other(DerClass, u64),
}

impl DerType {
    pub fn can_primitive(&self) -> bool {
        match self {
            DerType::EndOfContents => true,
            DerType::Boolean => true,
            DerType::Integer => true,
            DerType::BitString => true,
            DerType::OctetString => true,
            DerType::Null => true,
            DerType::ObjectIdentifier => true,
            DerType::ObjectDescriptor => true,
            DerType::Real => true,
            DerType::Enumerated => true,
            DerType::Utf8String => true,
            DerType::RelativeOid => true,
            DerType::Time => true,
            DerType::NumericString => true,
            DerType::PrintableString => true,
            DerType::TeletexString => true,
            DerType::VideotexString => true,
            DerType::Ia5String => true,
            DerType::GraphicString => true,
            DerType::VisibleString => true,
            DerType::GeneralString => true,
            DerType::UniversalString => true,
            DerType::CharacterString => true,
            DerType::BmpString => true,
            DerType::Date => true,
            DerType::TimeOfDay => true,
            DerType::DateTime => true,
            DerType::Duration => true,
            DerType::Other(_, _) => true,
            _ => false,
        }
    }

    pub fn can_constructed(&self) -> bool {
        match self {
            DerType::BitString => true,
            DerType::OctetString => true,
            DerType::External => true,
            DerType::EmbeddedPdv => true,
            DerType::Utf8String => true,
            DerType::Sequence => true,
            DerType::Set => true,
            DerType::NumericString => true,
            DerType::PrintableString => true,
            DerType::TeletexString => true,
            DerType::VideotexString => true,
            DerType::Ia5String => true,
            DerType::GraphicString => true,
            DerType::VisibleString => true,
            DerType::GeneralString => true,
            DerType::UniversalString => true,
            DerType::CharacterString => true,
            DerType::BmpString => true,
            DerType::Other(_, _) => true,
            _ => false,
        }
    }

    pub fn der_class(&self) -> DerClass {
        match self {
            DerType::Other(val, _) => *val,
            _ => DerClass::Universal,
        }
    }

    pub fn tag_no(&self) -> u64 {
        match self {
            DerType::EndOfContents => 0,
            DerType::Boolean => 1,
            DerType::Integer => 2,
            DerType::BitString => 3,
            DerType::OctetString => 4,
            DerType::Null => 5,
            DerType::ObjectIdentifier => 6,
            DerType::ObjectDescriptor => 7,
            DerType::External => 8,
            DerType::Real => 9,
            DerType::Enumerated => 10,
            DerType::EmbeddedPdv => 11,
            DerType::Utf8String => 12,
            DerType::RelativeOid => 13,
            DerType::Time => 14,
            DerType::Sequence => 16,
            DerType::Set => 17,
            DerType::NumericString => 18,
            DerType::PrintableString => 19,
            DerType::TeletexString => 20,
            DerType::VideotexString => 21,
            DerType::Ia5String => 22,
            DerType::UtcTime => 23,
            DerType::GeneralizedTime => 24,
            DerType::GraphicString => 25,
            DerType::VisibleString => 26,
            DerType::GeneralString => 27,
            DerType::UniversalString => 28,
            DerType::CharacterString => 29,
            DerType::BmpString => 30,
            DerType::Date => 31,
            DerType::TimeOfDay => 32,
            DerType::DateTime => 33,
            DerType::Duration => 34,
            DerType::Other(_, val) => *val,
        }
    }
}

impl fmt::Display for DerType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
