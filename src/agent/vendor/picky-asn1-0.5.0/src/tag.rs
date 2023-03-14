use serde::de;
use std::fmt;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Encoding {
    Primitive,
    Constructed,
}

impl fmt::Display for Encoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primitive => write!(f, "PRIMITIVE"),
            Self::Constructed => write!(f, "CONSTRUCTED"),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TagClass {
    Universal,
    Application,
    ContextSpecific,
    Private,
}

impl fmt::Display for TagClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Universal => write!(f, "UNIVERSAL"),
            Self::Application => write!(f, "APPLICATION"),
            Self::ContextSpecific => write!(f, "CONTEXT_SPECIFIC"),
            Self::Private => write!(f, "PRIVATE"),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Tag(u8);

impl Tag {
    pub const BOOLEAN: Self = Tag(0x01);
    pub const INTEGER: Self = Tag(0x02);
    pub const BIT_STRING: Self = Tag(0x03);
    pub const OCTET_STRING: Self = Tag(0x04);
    pub const NULL: Self = Tag(0x05);
    pub const OID: Self = Tag(0x06);
    pub const REAL: Self = Tag(0x09);
    pub const UTF8_STRING: Self = Tag(0x0C);
    pub const RELATIVE_OID: Self = Tag(0xD);
    pub const NUMERIC_STRING: Self = Tag(0x12);
    pub const PRINTABLE_STRING: Self = Tag(0x13);
    pub const TELETEX_STRING: Self = Tag(0x14);
    pub const VIDEOTEX_STRING: Self = Tag(0x15);
    pub const IA5_STRING: Self = Tag(0x16);
    pub const BMP_STRING: Self = Tag(0x1E);
    pub const UTC_TIME: Self = Tag(0x17);
    pub const GENERALIZED_TIME: Self = Tag(0x18);
    pub const SEQUENCE: Self = Tag(0x30);
    pub const SET: Self = Tag(0x31);
    pub const GENERAL_STRING: Self = Tag(0x1b);

    #[inline]
    pub const fn application_primitive(number: u8) -> Self {
        Tag(number & 0x1F | 0x40)
    }

    #[inline]
    pub const fn application_constructed(number: u8) -> Self {
        Tag(number & 0x1F | 0x60)
    }

    #[inline]
    pub const fn context_specific_primitive(number: u8) -> Self {
        Tag(number & 0x1F | 0x80)
    }

    #[inline]
    pub const fn context_specific_constructed(number: u8) -> Self {
        Tag(number & 0x1F | 0xA0)
    }

    /// Identifier octets as u8
    #[inline]
    pub const fn inner(self) -> u8 {
        self.0
    }

    /// Tag number of the ASN.1 value (filtering class bits and constructed bit with a mask)
    #[inline]
    pub const fn number(self) -> u8 {
        self.0 & 0x1F
    }

    // TODO: need version bump to be made const
    pub fn class(self) -> TagClass {
        match self.0 & 0xC0 {
            0x00 => TagClass::Universal,
            0x40 => TagClass::Application,
            0x80 => TagClass::ContextSpecific,
            _ /* 0xC0 */ => TagClass::Private,
        }
    }

    // TODO: need version bump to be made const
    pub fn class_and_number(self) -> (TagClass, u8) {
        (self.class(), self.number())
    }

    // TODO: need version bump to be made const
    pub fn components(self) -> (TagClass, Encoding, u8) {
        (self.class(), self.encoding(), self.number())
    }

    #[inline]
    pub const fn is_application(self) -> bool {
        self.0 & 0xC0 == 0x40
    }

    #[inline]
    pub const fn is_context_specific(self) -> bool {
        self.0 & 0xC0 == 0x80
    }

    #[inline]
    pub const fn is_universal(self) -> bool {
        self.0 & 0xC0 == 0x00
    }

    #[inline]
    pub const fn is_private(self) -> bool {
        self.0 & 0xC0 == 0xC0
    }

    #[inline]
    pub const fn is_constructed(self) -> bool {
        self.0 & 0x20 == 0x20
    }

    #[inline]
    pub const fn is_primitive(self) -> bool {
        !self.is_constructed()
    }

    // TODO: need version bump to be made const
    #[inline]
    pub fn encoding(self) -> Encoding {
        if self.is_constructed() {
            Encoding::Constructed
        } else {
            Encoding::Primitive
        }
    }
}

impl From<u8> for Tag {
    fn from(tag: u8) -> Self {
        Self(tag)
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Tag::BOOLEAN => write!(f, "BOOLEAN"),
            Tag::INTEGER => write!(f, "INTEGER"),
            Tag::BIT_STRING => write!(f, "BIT STRING"),
            Tag::OCTET_STRING => write!(f, "OCTET STRING"),
            Tag::NULL => write!(f, "NULL"),
            Tag::OID => write!(f, "OBJECT IDENTIFIER"),
            Tag::REAL => write!(f, "REAL"),
            Tag::UTF8_STRING => write!(f, "UTF8String"),
            Tag::RELATIVE_OID => write!(f, "RELATIVE-OID"),
            Tag::NUMERIC_STRING => write!(f, "NumericString"),
            Tag::PRINTABLE_STRING => write!(f, "PrintableString"),
            Tag::TELETEX_STRING => write!(f, "TeletexString"),
            Tag::VIDEOTEX_STRING => write!(f, "VideotexString"),
            Tag::IA5_STRING => write!(f, "IA5String"),
            Tag::BMP_STRING => write!(f, "BMPString"),
            Tag::UTC_TIME => write!(f, "UTCTime"),
            Tag::GENERALIZED_TIME => write!(f, "GeneralizedTime"),
            Tag::SEQUENCE => write!(f, "SEQUENCE"),
            Tag::SET => write!(f, "SET"),
            Tag::GENERAL_STRING => write!(f, "GeneralString"),
            other => write!(f, "{}({:02X}) {}", other.class(), other.number(), other.encoding()),
        }
    }
}

impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Tag({}[{:02X}])", self, self.0)
    }
}

/// Used to peek next tag by using `Deserializer::deserialize_identifier`.
///
/// Can be used to implement ASN.1 Choice.
///
/// # Examples
/// ```
/// use serde::de;
/// use picky_asn1::{
///     wrapper::{IntegerAsn1, Utf8StringAsn1},
///     tag::{Tag, TagPeeker},
/// };
/// use std::fmt;
///
/// pub enum MyChoice {
///     Integer(u32),
///     Utf8String(String),
/// }
///
/// impl<'de> de::Deserialize<'de> for MyChoice {
///     fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
///     where
///         D: de::Deserializer<'de>,
///     {
///         struct Visitor;
///
///         impl<'de> de::Visitor<'de> for Visitor {
///             type Value = MyChoice;
///
///             fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
///                 formatter.write_str("a valid MyChoice")
///             }
///
///             fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
///             where
///                 A: de::SeqAccess<'de>,
///             {
///                 match seq.next_element::<TagPeeker>()?.unwrap().next_tag {
///                     Tag::INTEGER => {
///                         let value = seq.next_element::<u32>()?.unwrap();
///                         Ok(MyChoice::Integer(value))
///                     }
///                     Tag::UTF8_STRING => {
///                         let value = seq.next_element::<String>()?.unwrap();
///                         Ok(MyChoice::Utf8String(value))
///                     }
///                     _ => Err(de::Error::invalid_value(
///                         de::Unexpected::Other(
///                             "[MyChoice] unsupported or unknown choice value",
///                         ),
///                         &"a supported choice value",
///                     ))
///                 }
///             }
///         }
///
///         deserializer.deserialize_enum("MyChoice", &["Integer", "Utf8String"], Visitor)
///     }
/// }
///
/// let buffer = b"\x0C\x06\xE8\x8B\x97\xE5\xAD\x97";
/// let my_choice: MyChoice = picky_asn1_der::from_bytes(buffer).unwrap();
/// match my_choice {
///     MyChoice::Integer(_) => panic!("wrong variant"),
///     MyChoice::Utf8String(string) => assert_eq!(string, "苗字"),
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TagPeeker {
    pub next_tag: Tag,
}

impl<'de> de::Deserialize<'de> for TagPeeker {
    fn deserialize<D>(deserializer: D) -> Result<TagPeeker, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = TagPeeker;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid ASN.1 tag")
            }

            fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(TagPeeker { next_tag: v.into() })
            }
        }

        deserializer.deserialize_identifier(Visitor)
    }
}
