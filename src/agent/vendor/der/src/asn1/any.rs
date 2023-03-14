//! ASN.1 `ANY` type.

use crate::{
    asn1::*, ByteSlice, Choice, Decodable, DecodeValue, Decoder, DerOrd, EncodeValue, Encoder,
    Error, ErrorKind, FixedTag, Header, Length, Result, Tag, Tagged, ValueOrd,
};
use core::cmp::Ordering;

#[cfg(feature = "oid")]
use crate::asn1::ObjectIdentifier;

/// ASN.1 `ANY`: represents any explicitly tagged ASN.1 value.
///
/// Technically `ANY` hasn't been a recommended part of ASN.1 since the X.209
/// revision from 1988. It was deprecated and replaced by Information Object
/// Classes in X.680 in 1994, and X.690 no longer refers to it whatsoever.
///
/// Nevertheless, this crate defines an [`Any`] type as it remains a familiar
/// and useful concept which is still extensively used in things like
/// PKI-related RFCs.
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct Any<'a> {
    /// Tag representing the type of the encoded value.
    tag: Tag,

    /// Inner value encoded as bytes.
    value: ByteSlice<'a>,
}

impl<'a> Any<'a> {
    /// [`Any`] representation of the ASN.1 `NULL` type.
    pub const NULL: Self = Self {
        tag: Tag::Null,
        value: ByteSlice::EMPTY,
    };

    /// Create a new [`Any`] from the provided [`Tag`] and byte slice.
    pub fn new(tag: Tag, bytes: &'a [u8]) -> Result<Self> {
        let value = ByteSlice::new(bytes).map_err(|_| ErrorKind::Length { tag })?;
        Ok(Self { tag, value })
    }

    /// Infallible creation of an [`Any`] from a [`ByteSlice`].
    pub(crate) fn from_tag_and_value(tag: Tag, value: ByteSlice<'a>) -> Self {
        Self { tag, value }
    }

    /// Get the raw value for this [`Any`] type as a byte slice.
    pub fn value(self) -> &'a [u8] {
        self.value.as_bytes()
    }

    /// Attempt to decode this [`Any`] type into the inner value.
    pub fn decode_into<T>(self) -> Result<T>
    where
        T: DecodeValue<'a> + FixedTag,
    {
        self.tag.assert_eq(T::TAG)?;
        let mut decoder = Decoder::new(self.value())?;
        let result = T::decode_value(&mut decoder, self.value.len())?;
        decoder.finish(result)
    }

    /// Is this value an ASN.1 `NULL` value?
    pub fn is_null(self) -> bool {
        self == Self::NULL
    }

    /// Attempt to decode an ASN.1 `BIT STRING`.
    pub fn bit_string(self) -> Result<BitString<'a>> {
        self.try_into()
    }

    /// Attempt to decode an ASN.1 `CONTEXT-SPECIFIC` field.
    pub fn context_specific<T>(self) -> Result<ContextSpecific<T>>
    where
        T: Decodable<'a>,
    {
        self.try_into()
    }

    /// Attempt to decode an ASN.1 `GeneralizedTime`.
    pub fn generalized_time(self) -> Result<GeneralizedTime> {
        self.try_into()
    }

    /// Attempt to decode an ASN.1 `IA5String`.
    pub fn ia5_string(self) -> Result<Ia5String<'a>> {
        self.try_into()
    }

    /// Attempt to decode an ASN.1 `OCTET STRING`.
    pub fn octet_string(self) -> Result<OctetString<'a>> {
        self.try_into()
    }

    /// Attempt to decode an ASN.1 `OBJECT IDENTIFIER`.
    #[cfg(feature = "oid")]
    #[cfg_attr(docsrs, doc(cfg(feature = "oid")))]
    pub fn oid(self) -> Result<ObjectIdentifier> {
        self.try_into()
    }

    /// Attempt to decode an ASN.1 `OPTIONAL` value.
    pub fn optional<T>(self) -> Result<Option<T>>
    where
        T: Choice<'a> + TryFrom<Self, Error = Error>,
    {
        if T::can_decode(self.tag) {
            T::try_from(self).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Attempt to decode an ASN.1 `PrintableString`.
    pub fn printable_string(self) -> Result<PrintableString<'a>> {
        self.try_into()
    }

    /// Attempt to decode this value an ASN.1 `SEQUENCE`, creating a new
    /// nested [`Decoder`] and calling the provided argument with it.
    pub fn sequence<F, T>(self, f: F) -> Result<T>
    where
        F: FnOnce(&mut Decoder<'a>) -> Result<T>,
    {
        self.tag.assert_eq(Tag::Sequence)?;
        let mut seq_decoder = Decoder::new(self.value.as_bytes())?;
        let result = f(&mut seq_decoder)?;
        seq_decoder.finish(result)
    }

    /// Attempt to decode an ASN.1 `UTCTime`.
    pub fn utc_time(self) -> Result<UtcTime> {
        self.try_into()
    }

    /// Attempt to decode an ASN.1 `UTF8String`.
    pub fn utf8_string(self) -> Result<Utf8String<'a>> {
        self.try_into()
    }
}

impl<'a> Choice<'a> for Any<'a> {
    fn can_decode(_: Tag) -> bool {
        true
    }
}

impl<'a> Decodable<'a> for Any<'a> {
    fn decode(decoder: &mut Decoder<'a>) -> Result<Any<'a>> {
        let header = Header::decode(decoder)?;
        let tag = header.tag;
        let value = ByteSlice::decode_value(decoder, header.length)?;
        Ok(Self { tag, value })
    }
}

impl EncodeValue for Any<'_> {
    fn value_len(&self) -> Result<Length> {
        Ok(self.value.len())
    }

    fn encode_value(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        encoder.bytes(self.value())
    }
}

impl Tagged for Any<'_> {
    fn tag(&self) -> Tag {
        self.tag
    }
}

impl ValueOrd for Any<'_> {
    fn value_cmp(&self, other: &Self) -> Result<Ordering> {
        self.value.der_cmp(&other.value)
    }
}

impl<'a> From<Any<'a>> for ByteSlice<'a> {
    fn from(any: Any<'a>) -> ByteSlice<'a> {
        any.value
    }
}

impl<'a> TryFrom<&'a [u8]> for Any<'a> {
    type Error = Error;

    fn try_from(bytes: &'a [u8]) -> Result<Any<'a>> {
        Any::from_der(bytes)
    }
}
