//! ASN.1 `ANY` type.

use crate::{
    BitString, ByteSlice, Choice, ContextSpecific, Decodable, Decoder, Encodable, Encoder, Error,
    ErrorKind, GeneralizedTime, Header, Ia5String, Length, Null, OctetString, PrintableString,
    Result, Sequence, Tag, UtcTime, Utf8String,
};
use core::convert::{TryFrom, TryInto};

#[cfg(feature = "oid")]
use crate::ObjectIdentifier;

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

    /// Encoded length of this [`Any`] value.
    length: Length,

    /// Inner value encoded as bytes.
    value: ByteSlice<'a>,
}

impl<'a> Any<'a> {
    /// Create a new [`Any`] from the provided [`Tag`] and byte slice.
    pub fn new(tag: Tag, bytes: &'a [u8]) -> Result<Self> {
        let value = ByteSlice::new(bytes).map_err(|_| ErrorKind::Length { tag })?;

        let length = if has_leading_zero_byte(tag) {
            (value.len() + 1u8)?
        } else {
            value.len()
        };

        Ok(Self { tag, length, value })
    }

    /// Infallible creation of an [`Any`] from a [`ByteSlice`].
    pub(crate) fn from_tag_and_value(tag: Tag, value: ByteSlice<'a>) -> Self {
        Self {
            tag,
            length: value.len(),
            value,
        }
    }

    /// Get the tag for this [`Any`] type.
    pub fn tag(self) -> Tag {
        self.tag
    }

    /// Get the [`Length`] of this [`Any`] type's value.
    pub fn len(self) -> Length {
        self.length
    }

    /// Is the body of this [`Any`] type empty?
    pub fn is_empty(self) -> bool {
        self.value.is_empty()
    }

    /// Is this value an ASN.1 NULL value?
    pub fn is_null(self) -> bool {
        Null::try_from(self).is_ok()
    }

    /// Get the raw value for this [`Any`] type as a byte slice.
    pub fn as_bytes(self) -> &'a [u8] {
        self.value.as_bytes()
    }

    /// Attempt to decode an ASN.1 `BIT STRING`.
    pub fn bit_string(self) -> Result<BitString<'a>> {
        self.try_into()
    }

    /// Attempt to decode an ASN.1 `CONTEXT-SPECIFIC` field.
    pub fn context_specific(self) -> Result<ContextSpecific<'a>> {
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
        Sequence::try_from(self)?.decode_nested(f)
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
        let mut value = decoder
            .bytes(header.length)
            .or_else(|_| decoder.error(ErrorKind::Length { tag }))?;

        if has_leading_zero_byte(tag) {
            let (byte, rest) = value
                .split_first()
                .ok_or(ErrorKind::Truncated)
                .or_else(|e| decoder.error(e))?;

            // The first octet of a BIT STRING encodes the number of unused bits.
            // We presently constrain this to 0.
            if *byte != 0 {
                return decoder.error(ErrorKind::Noncanonical);
            }

            value = rest;
        }

        Self::new(tag, value).or_else(|e| decoder.error(e.kind()))
    }
}

impl<'a> Encodable for Any<'a> {
    fn encoded_len(&self) -> Result<Length> {
        self.len().for_tlv()
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        Header::new(self.tag, self.len())?.encode(encoder)?;

        if has_leading_zero_byte(self.tag) {
            encoder.byte(0)?;
        }

        encoder.bytes(self.as_bytes())
    }
}

impl<'a> TryFrom<&'a [u8]> for Any<'a> {
    type Error = Error;

    fn try_from(bytes: &'a [u8]) -> Result<Any<'a>> {
        Any::from_der(bytes)
    }
}

// Special handling for the leading `0` byte on [`BitString`]
impl<'a> TryFrom<Any<'a>> for BitString<'a> {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<BitString<'a>> {
        any.tag().assert_eq(Tag::BitString)?;

        Ok(BitString {
            inner: any.value,
            encoded_len: any.length,
        })
    }
}

// Special handling for the leading `0` byte on [`BitString`]
impl<'a> From<BitString<'a>> for Any<'a> {
    fn from(bit_string: BitString<'a>) -> Any<'a> {
        Any {
            tag: Tag::BitString,
            length: bit_string.encoded_len,
            value: bit_string.inner,
        }
    }
}

/// Does a value with this tag have a leading zero byte?
///
/// This is mostly a hack for `BIT STRING`, and permits simple `From`
/// conversions from `BitString` into `Any`.
// TODO(tarcieri): better generalize this? or is there a better solution?
fn has_leading_zero_byte(tag: Tag) -> bool {
    tag == Tag::BitString
}
