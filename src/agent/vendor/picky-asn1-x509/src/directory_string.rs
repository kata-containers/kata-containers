use picky_asn1::restricted_string::{PrintableString, Utf8String};
use picky_asn1::tag::{Tag, TagPeeker};
use picky_asn1::wrapper::PrintableStringAsn1;
use serde::{de, ser};
use std::borrow::Cow;
use std::fmt;

/// [RFC 5280 #4.1.2.4](https://tools.ietf.org/html/rfc5280#section-4.1.2.4)
///
/// TeletexString, UniversalString and BmpString are not supported.
///
/// ```not_rust
/// DirectoryString ::= CHOICE {
///      teletexString       TeletexString   (SIZE (1..MAX)),
///      printableString     PrintableString (SIZE (1..MAX)),
///      universalString     UniversalString (SIZE (1..MAX)),
///      utf8String          UTF8String      (SIZE (1..MAX)),
///      bmpString           BMPString       (SIZE (1..MAX)) }
/// ```
#[derive(Debug, PartialEq, Clone)]
pub enum DirectoryString {
    //TeletexString,
    PrintableString(PrintableStringAsn1),
    //UniversalString,
    Utf8String(String),
    //BmpString,
}

impl fmt::Display for DirectoryString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_utf8_lossy())
    }
}

impl DirectoryString {
    pub fn to_utf8_lossy(&self) -> Cow<str> {
        match &self {
            DirectoryString::PrintableString(string) => String::from_utf8_lossy(string.as_bytes()),
            DirectoryString::Utf8String(string) => Cow::Borrowed(string.as_str()),
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match &self {
            DirectoryString::PrintableString(string) => string.as_bytes(),
            DirectoryString::Utf8String(string) => string.as_bytes(),
        }
    }
}

impl From<&str> for DirectoryString {
    fn from(string: &str) -> Self {
        Self::Utf8String(string.to_owned())
    }
}

impl From<String> for DirectoryString {
    fn from(string: String) -> Self {
        Self::Utf8String(string)
    }
}

impl From<PrintableString> for DirectoryString {
    fn from(string: PrintableString) -> Self {
        Self::PrintableString(string.into())
    }
}

impl From<Utf8String> for DirectoryString {
    fn from(string: Utf8String) -> Self {
        Self::Utf8String(String::from_utf8(string.into_bytes()).expect("Utf8String has the right charset"))
    }
}

impl From<PrintableStringAsn1> for DirectoryString {
    fn from(string: PrintableStringAsn1) -> Self {
        Self::PrintableString(string)
    }
}

impl From<DirectoryString> for String {
    fn from(ds: DirectoryString) -> Self {
        match ds {
            DirectoryString::PrintableString(string) => String::from_utf8_lossy(string.as_bytes()).into(),
            DirectoryString::Utf8String(string) => string,
        }
    }
}

impl ser::Serialize for DirectoryString {
    fn serialize<S>(&self, serializer: S) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
    where
        S: ser::Serializer,
    {
        match &self {
            DirectoryString::PrintableString(string) => string.serialize(serializer),
            DirectoryString::Utf8String(string) => string.serialize(serializer),
        }
    }
}

impl<'de> de::Deserialize<'de> for DirectoryString {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = DirectoryString;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid DER-encoded DirectoryString")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let tag_peeker: TagPeeker = seq_next_element!(seq, DirectoryString, "choice tag");
                match tag_peeker.next_tag {
                    Tag::UTF8_STRING => Ok(DirectoryString::Utf8String(seq_next_element!(
                        seq,
                        DirectoryString,
                        "Utf8String"
                    ))),
                    Tag::PRINTABLE_STRING => Ok(DirectoryString::PrintableString(seq_next_element!(
                        seq,
                        DirectoryString,
                        "PrintableString"
                    ))),
                    Tag::TELETEX_STRING => Err(serde_invalid_value!(
                        DirectoryString,
                        "TeletexString not supported",
                        "a supported string type"
                    )),
                    Tag::VIDEOTEX_STRING => Err(serde_invalid_value!(
                        DirectoryString,
                        "VideotexString not supported",
                        "a supported string type"
                    )),
                    Tag::IA5_STRING => Err(serde_invalid_value!(
                        DirectoryString,
                        "IA5String not supported",
                        "a supported string type"
                    )),
                    _ => Err(serde_invalid_value!(
                        DirectoryString,
                        "unknown string type",
                        "a known supported string type"
                    )),
                }
            }
        }

        deserializer.deserialize_enum("DirectoryString", &["PrintableString", "Utf8String"], Visitor)
    }
}
