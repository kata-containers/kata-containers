use core::{
    convert::TryFrom,
    fmt::{self, Debug, Display, Formatter},
    panic, str,
};
use serde::{
    de::{Deserialize, Deserializer, Visitor},
    ser::{Serialize, Serializer},
};
use static_assertions::assert_impl_all;
use std::{
    ops::{Bound, RangeBounds},
    sync::Arc,
};

use crate::{signature_parser::SignatureParser, Basic, EncodingFormat, Error, Result, Type};

// A data type similar to Cow and [`bytes::Bytes`] but unlike the former won't allow us to only keep
// the owned bytes in Arc and latter doesn't have a notion of borrowed data and would require API
// breakage.
//
// [`bytes::Bytes`]: https://docs.rs/bytes/0.5.6/bytes/struct.Bytes.html
#[derive(PartialEq, Eq, Hash, Clone)]
enum Bytes<'b> {
    Borrowed(&'b [u8]),
    Static(&'static [u8]),
    Owned(Arc<[u8]>),
}

impl<'b> Bytes<'b> {
    const fn borrowed<'s: 'b>(bytes: &'s [u8]) -> Self {
        Self::Borrowed(bytes)
    }

    fn owned(bytes: Vec<u8>) -> Self {
        Self::Owned(bytes.into())
    }
}

impl<'b> std::ops::Deref for Bytes<'b> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            Bytes::Borrowed(borrowed) => borrowed,
            Bytes::Static(borrowed) => borrowed,
            Bytes::Owned(owned) => owned,
        }
    }
}

/// String that [identifies] the type of an encoded value.
///
/// # Examples
///
/// ```
/// use core::convert::TryFrom;
/// use zvariant::Signature;
///
/// // Valid signatures
/// let s = Signature::try_from("").unwrap();
/// assert_eq!(s, "");
/// let s = Signature::try_from("y").unwrap();
/// assert_eq!(s, "y");
/// let s = Signature::try_from("xs").unwrap();
/// assert_eq!(s, "xs");
/// let s = Signature::try_from("(ysa{sd})").unwrap();
/// assert_eq!(s, "(ysa{sd})");
/// let s = Signature::try_from("a{sd}").unwrap();
/// assert_eq!(s, "a{sd}");
///
/// // Invalid signatures
/// Signature::try_from("z").unwrap_err();
/// Signature::try_from("(xs").unwrap_err();
/// Signature::try_from("xs)").unwrap_err();
/// Signature::try_from("s/").unwrap_err();
/// Signature::try_from("a").unwrap_err();
/// Signature::try_from("a{yz}").unwrap_err();
/// ```
///
/// This is implemented so that multiple instances can share the same underlying signature string.
/// Use [`slice`] method to create new signature that represents a portion of a signature
///
/// [identifies]: https://dbus.freedesktop.org/doc/dbus-specification.html#type-system
/// [`slice`]: #method.slice
#[derive(Eq, Hash, Clone)]
pub struct Signature<'a> {
    bytes: Bytes<'a>,
    pos: usize,
    end: usize,
}

assert_impl_all!(Signature<'_>: Send, Sync, Unpin);

impl<'a> Signature<'a> {
    /// The signature as a string.
    pub fn as_str(&self) -> &str {
        // SAFETY: non-UTF8 characters in Signature are rejected by safe constructors
        unsafe { str::from_utf8_unchecked(self.as_bytes()) }
    }

    /// The signature bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[self.pos..self.end]
    }

    /// Create a new Signature from given bytes.
    ///
    /// Since the passed bytes are not checked for correctness, it's provided for ease of
    /// `Type` implementations.
    ///
    /// # Safety
    ///
    /// This method is unsafe as it allows creating a `str` that is not valid UTF-8.
    pub unsafe fn from_bytes_unchecked<'s: 'a>(bytes: &'s [u8]) -> Self {
        Self {
            bytes: Bytes::borrowed(bytes),
            pos: 0,
            end: bytes.len(),
        }
    }

    /// Same as `from_bytes_unchecked`, except it takes a static reference.
    ///
    /// # Safety
    ///
    /// This method is unsafe as it allows creating a `str` that is not valid UTF-8.
    pub unsafe fn from_static_bytes_unchecked(bytes: &'static [u8]) -> Self {
        Self {
            bytes: Bytes::Static(bytes),
            pos: 0,
            end: bytes.len(),
        }
    }

    /// Same as `from_bytes_unchecked`, except it takes a string reference.
    pub const fn from_str_unchecked<'s: 'a>(signature: &'s str) -> Self {
        Self {
            bytes: Bytes::borrowed(signature.as_bytes()),
            pos: 0,
            end: signature.len(),
        }
    }

    /// Same as `from_str_unchecked`, except it takes a static string reference.
    pub const fn from_static_str_unchecked(signature: &'static str) -> Self {
        Self {
            bytes: Bytes::Static(signature.as_bytes()),
            pos: 0,
            end: signature.len(),
        }
    }

    /// Same as `from_str_unchecked`, except it takes an owned `String`.
    pub fn from_string_unchecked(signature: String) -> Self {
        let bytes = signature.into_bytes();
        let end = bytes.len();

        Self {
            bytes: Bytes::owned(bytes),
            pos: 0,
            end,
        }
    }

    /// Same as `from_static_str_unchecked`, except it checks validity of the signature.
    ///
    /// It's recommended to use this method instead of `TryFrom<&str>` implementation for
    /// `&'static str`. The former will ensure that [`Signature::to_owned`] and
    /// [`Signature::into_owned`] do not clone the underlying bytes.
    pub fn from_static_str(signature: &'static str) -> Result<Self> {
        let bytes = signature.as_bytes();
        ensure_correct_signature_str(bytes)?;

        Ok(Self {
            bytes: Bytes::Static(bytes),
            pos: 0,
            end: signature.len(),
        })
    }

    /// Same as `from_static_bytes_unchecked`, except it checks validity of the signature.
    ///
    /// It's recommended to use this method instead of the `TryFrom<&[u8]>` implementation for
    /// `&'static [u8]`. The former will ensure that [`Signature::to_owned`] and
    /// [`Signature::into_owned`] do not clone the underlying bytes.
    pub fn from_static_bytes(bytes: &'static [u8]) -> Result<Self> {
        ensure_correct_signature_str(bytes)?;

        Ok(Self {
            bytes: Bytes::Static(bytes),
            pos: 0,
            end: bytes.len(),
        })
    }

    /// the signature's length.
    pub fn len(&self) -> usize {
        self.end - self.pos
    }

    /// if the signature is empty.
    pub fn is_empty(&self) -> bool {
        self.as_bytes().is_empty()
    }

    /// Creates an owned clone of `self`.
    pub fn to_owned(&self) -> Signature<'static> {
        match &self.bytes {
            Bytes::Borrowed(_) => {
                let bytes = Bytes::owned(self.as_bytes().to_vec());
                let pos = 0;
                let end = bytes.len();

                Signature { bytes, pos, end }
            }
            Bytes::Static(b) => Signature {
                bytes: Bytes::Static(b),
                pos: self.pos,
                end: self.end,
            },
            Bytes::Owned(owned) => Signature {
                bytes: Bytes::Owned(owned.clone()),
                pos: self.pos,
                end: self.end,
            },
        }
    }

    /// Creates an owned clone of `self`.
    pub fn into_owned(self) -> Signature<'static> {
        self.to_owned()
    }

    /// Returns a slice of `self` for the provided range.
    ///
    /// # Panics
    ///
    /// Requires that begin <= end and end <= self.len(), otherwise slicing will panic.
    #[must_use]
    pub fn slice(&self, range: impl RangeBounds<usize>) -> Self {
        let len = self.len();

        let pos = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n + 1,
            Bound::Unbounded => 0,
        };

        let end = match range.end_bound() {
            Bound::Included(&n) => n + 1,
            Bound::Excluded(&n) => n,
            Bound::Unbounded => len,
        };

        assert!(
            pos <= end,
            "range start must not be greater than end: {:?} > {:?}",
            pos,
            end,
        );
        assert!(end <= len, "range end out of bounds: {:?} > {:?}", end, len,);

        if end == pos {
            return Self::from_str_unchecked("");
        }

        let mut clone = self.clone();
        clone.pos += pos;
        clone.end = self.pos + end;

        clone
    }
}

impl<'a> Debug for Signature<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Signature").field(&self.as_str()).finish()
    }
}

impl<'a> Basic for Signature<'a> {
    const SIGNATURE_CHAR: char = 'g';
    const SIGNATURE_STR: &'static str = "g";

    fn alignment(format: EncodingFormat) -> usize {
        match format {
            EncodingFormat::DBus => 1,
            #[cfg(feature = "gvariant")]
            EncodingFormat::GVariant => 1,
        }
    }
}

impl<'a> Type for Signature<'a> {
    fn signature() -> Signature<'static> {
        Signature::from_static_str_unchecked(Self::SIGNATURE_STR)
    }
}

impl<'a, 'b> From<&'b Signature<'a>> for Signature<'a> {
    fn from(signature: &'b Signature<'a>) -> Signature<'a> {
        signature.clone()
    }
}

impl<'a> TryFrom<&'a [u8]> for Signature<'a> {
    type Error = Error;

    fn try_from(value: &'a [u8]) -> Result<Self> {
        ensure_correct_signature_str(value)?;

        // SAFETY: ensure_correct_signature_str checks UTF8
        unsafe { Ok(Self::from_bytes_unchecked(value)) }
    }
}

/// Try to create a Signature from a string.
impl<'a> TryFrom<&'a str> for Signature<'a> {
    type Error = Error;

    fn try_from(value: &'a str) -> Result<Self> {
        Self::try_from(value.as_bytes())
    }
}

impl<'a> TryFrom<String> for Signature<'a> {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        ensure_correct_signature_str(value.as_bytes())?;

        Ok(Self::from_string_unchecked(value))
    }
}

impl<'a> From<Signature<'a>> for String {
    fn from(value: Signature<'a>) -> String {
        String::from(value.as_str())
    }
}

impl<'a> From<&Signature<'a>> for String {
    fn from(value: &Signature<'a>) -> String {
        String::from(value.as_str())
    }
}

impl<'a> std::ops::Deref for Signature<'a> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl<'a, 'b> PartialEq<Signature<'a>> for Signature<'b> {
    fn eq(&self, other: &Signature<'_>) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl<'a> PartialEq<str> for Signature<'a> {
    fn eq(&self, other: &str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl<'a> PartialEq<&str> for Signature<'a> {
    fn eq(&self, other: &&str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl<'a> Display for Signature<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        std::fmt::Display::fmt(&self.as_str(), f)
    }
}

impl<'a> Serialize for Signature<'a> {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for Signature<'a> {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let visitor = SignatureVisitor;

        deserializer.deserialize_str(visitor)
    }
}

struct SignatureVisitor;

impl<'de> Visitor<'de> for SignatureVisitor {
    type Value = Signature<'de>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a Signature")
    }

    #[inline]
    fn visit_borrowed_str<E>(self, value: &'de str) -> core::result::Result<Signature<'de>, E>
    where
        E: serde::de::Error,
    {
        Signature::try_from(value).map_err(serde::de::Error::custom)
    }
}

fn ensure_correct_signature_str(signature: &[u8]) -> Result<()> {
    if signature.len() > 255 {
        return Err(serde::de::Error::invalid_length(
            signature.len(),
            &"<= 255 characters",
        ));
    }

    if signature.is_empty() {
        return Ok(());
    }

    // SAFETY: SignatureParser never calls as_str
    let signature = unsafe { Signature::from_bytes_unchecked(signature) };
    let mut parser = SignatureParser::new(signature);
    while !parser.done() {
        let _ = parser.parse_next_signature()?;
    }

    Ok(())
}

/// Owned [`Signature`](struct.Signature.html)
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Type)]
pub struct OwnedSignature(Signature<'static>);

assert_impl_all!(OwnedSignature: Send, Sync, Unpin);

impl OwnedSignature {
    pub fn into_inner(self) -> Signature<'static> {
        self.0
    }
}

impl std::ops::Deref for OwnedSignature {
    type Target = Signature<'static>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::convert::From<OwnedSignature> for Signature<'static> {
    fn from(o: OwnedSignature) -> Self {
        o.into_inner()
    }
}

impl<'a> std::convert::From<Signature<'a>> for OwnedSignature {
    fn from(o: Signature<'a>) -> Self {
        OwnedSignature(o.into_owned())
    }
}

impl std::convert::From<OwnedSignature> for crate::Value<'static> {
    fn from(o: OwnedSignature) -> Self {
        o.into_inner().into()
    }
}

impl<'de> Deserialize<'de> for OwnedSignature {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let visitor = SignatureVisitor;

        deserializer
            .deserialize_str(visitor)
            .map(|v| OwnedSignature(v.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::Signature;

    #[test]
    fn signature_slicing() {
        let sig = Signature::from_str_unchecked("(asta{sv})");
        assert_eq!(sig, "(asta{sv})");

        let slice = sig.slice(1..);
        assert_eq!(slice.len(), sig.len() - 1);
        assert_eq!(slice, &sig[1..]);
        assert_eq!(slice.as_bytes()[1], b's');
        assert_eq!(slice.as_bytes()[2], b't');

        let slice = slice.slice(2..3);
        assert_eq!(slice.len(), 1);
        assert_eq!(slice, "t");
        assert_eq!(slice.slice(1..), "");
    }
}
