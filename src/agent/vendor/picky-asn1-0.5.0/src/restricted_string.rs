use serde::{de, ser};
use std::error::Error;
use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;
use std::str::FromStr;

// === CharSetError === //

#[derive(Debug)]
pub struct CharSetError;

impl Error for CharSetError {}

impl fmt::Display for CharSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        writeln!(f, "invalid charset")
    }
}

// === CharSet === //

pub trait CharSet {
    /// Checks whether a sequence is a valid string or not.
    fn check(data: &[u8]) -> bool;
}

// === RestrictedString === //

/// A generic restricted character string.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct RestrictedString<C> {
    data: Vec<u8>,
    marker: PhantomData<C>,
}

impl<C: CharSet> RestrictedString<C> {
    /// Create a new RestrictedString without CharSet validation.
    ///
    /// # Safety
    ///
    /// You have to make sure the right CharSet is used.
    pub unsafe fn new_unchecked<V>(data: V) -> Self
    where
        V: Into<Vec<u8>>,
    {
        RestrictedString {
            data: data.into(),
            marker: PhantomData,
        }
    }

    pub fn new<V>(data: V) -> Result<Self, CharSetError>
    where
        V: Into<Vec<u8>>,
    {
        let data = data.into();
        if !C::check(&data) {
            return Err(CharSetError);
        };
        Ok(RestrictedString {
            data,
            marker: PhantomData,
        })
    }

    pub fn from_string(s: String) -> Result<Self, CharSetError> {
        Self::new(s.into_bytes())
    }

    /// Converts into underlying bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    /// Returns underlying bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

impl<C: CharSet> Deref for RestrictedString<C> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<C: CharSet> FromStr for RestrictedString<C> {
    type Err = CharSetError;

    fn from_str(s: &str) -> Result<Self, CharSetError> {
        Self::new(s.as_bytes())
    }
}

impl<C: CharSet> AsRef<[u8]> for RestrictedString<C> {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl<C: CharSet> fmt::Display for RestrictedString<C> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&String::from_utf8_lossy(&self.data), fmt)
    }
}

impl<C: CharSet> From<RestrictedString<C>> for Vec<u8> {
    fn from(rs: RestrictedString<C>) -> Self {
        rs.into_bytes()
    }
}

impl<'de, C> de::Deserialize<'de> for RestrictedString<C>
where
    C: CharSet,
{
    fn deserialize<D>(deserializer: D) -> Result<RestrictedString<C>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor<C>(std::marker::PhantomData<C>);

        impl<'de, C> de::Visitor<'de> for Visitor<C>
        where
            C: CharSet,
        {
            type Value = RestrictedString<C>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid buffer representing a restricted string")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_byte_buf(v.to_vec())
            }

            fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                RestrictedString::new(v).map_err(|_| {
                    E::invalid_value(
                        de::Unexpected::Other("invalid charset"),
                        &"a buffer representing a string using the right charset",
                    )
                })
            }
        }

        deserializer.deserialize_byte_buf(Visitor(std::marker::PhantomData))
    }
}

impl<C> ser::Serialize for RestrictedString<C> {
    fn serialize<S>(&self, serializer: S) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_bytes(&self.data)
    }
}

// === NumericString === //

/// 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, and SPACE
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct NumericCharSet;
pub type NumericString = RestrictedString<NumericCharSet>;

impl CharSet for NumericCharSet {
    fn check(data: &[u8]) -> bool {
        for &c in data {
            if c != b' ' && !c.is_ascii_digit() {
                return false;
            }
        }
        true
    }
}

// === PrintableString === //

/// a-z, A-Z, ' () +,-.?:/= and SPACE
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct PrintableCharSet;
pub type PrintableString = RestrictedString<PrintableCharSet>;

impl CharSet for PrintableCharSet {
    fn check(data: &[u8]) -> bool {
        for &c in data {
            if !(c.is_ascii_alphanumeric()
                || c == b' '
                || c == b'\''
                || c == b'('
                || c == b')'
                || c == b'+'
                || c == b','
                || c == b'-'
                || c == b'.'
                || c == b'/'
                || c == b':'
                || c == b'='
                || c == b'?')
            {
                return false;
            }
        }
        true
    }
}

// === Utf8String === //

/// any character from a recognized alphabet (including ASCII control characters)
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Utf8CharSet;

pub type Utf8String = RestrictedString<Utf8CharSet>;

impl CharSet for Utf8CharSet {
    fn check(data: &[u8]) -> bool {
        std::str::from_utf8(data).is_ok()
    }
}

// === IA5String === //

/// First 128 ASCII characters (values from `0x00` to `0x7F`)
/// Used to represent ISO 646 (IA5) characters.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct IA5CharSet;

pub type IA5String = RestrictedString<IA5CharSet>;

impl CharSet for IA5CharSet {
    fn check(data: &[u8]) -> bool {
        for &c in data {
            if !c.is_ascii() {
                return false;
            }
        }
        true
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct BMPCharSet;

pub type BMPString = RestrictedString<BMPCharSet>;

impl CharSet for BMPCharSet {
    fn check(data: &[u8]) -> bool {
        if data.len() % 2 != 0 {
            return false;
        }

        let u16_it = data
            .chunks_exact(2)
            .into_iter()
            .map(|elem| u16::from_be_bytes([elem[1], elem[0]]));

        core::char::decode_utf16(u16_it).all(|c| matches!(c, Ok(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_printable_string() {
        PrintableString::from_str("29INRUSAET3snre?:=tanui83  9283019").expect("invalid string");
    }

    #[test]
    fn invalid_printable_string() {
        assert!(PrintableString::from_str("1224na÷日本語はむずかちー−×—«BUeisuteurnt").is_err());
    }

    #[test]
    fn valid_numeric_string() {
        NumericString::from_str("2983  9283019").expect("invalid string");
    }

    #[test]
    fn invalid_numeric_string() {
        assert!(NumericString::from_str("1224na÷日本語はむずかちー−×—«BUeisuteurnt").is_err());
    }

    #[test]
    fn valid_ia5_string() {
        IA5String::from_str("BUeisuteurnt").expect("invalid string");
    }

    #[test]
    fn invalid_ia5_string() {
        assert!(IA5String::from_str("BUéisuteurnt").is_err());
    }

    #[test]
    fn valid_utf8_string() {
        Utf8String::from_str("1224na÷日本語はむずかちー−×—«BUeisuteurnt").expect("invalid string");
    }

    #[test]
    fn valid_bmp_string() {
        BMPString::from_str("语言处理").expect("valid unicode string");
    }

    #[test]
    fn invalid_bmp_string() {
        assert!(BMPString::from_str("1224na÷日本語はむずかちー−×—«BUeisuteurnt").is_err())
    }
}
