//! DER encoder.

use crate::{
    asn1::*, Encodable, EncodeValue, Error, ErrorKind, Header, Length, Result, Tag, TagMode,
    TagNumber, Tagged,
};

/// DER encoder.
#[derive(Debug)]
pub struct Encoder<'a> {
    /// Buffer into which DER-encoded message is written
    bytes: Option<&'a mut [u8]>,

    /// Total number of bytes written to buffer so far
    position: Length,
}

impl<'a> Encoder<'a> {
    /// Create a new encoder with the given byte slice as a backing buffer.
    pub fn new(bytes: &'a mut [u8]) -> Self {
        Self {
            bytes: Some(bytes),
            position: Length::ZERO,
        }
    }

    /// Encode a value which impls the [`Encodable`] trait.
    pub fn encode<T: Encodable>(&mut self, encodable: &T) -> Result<()> {
        if self.is_failed() {
            self.error(ErrorKind::Failed)?;
        }

        encodable.encode(self).map_err(|e| {
            self.bytes.take();
            e.nested(self.position)
        })
    }

    /// Return an error with the given [`ErrorKind`], annotating it with
    /// context about where the error occurred.
    // TODO(tarcieri): change return type to `Error`
    pub fn error<T>(&mut self, kind: ErrorKind) -> Result<T> {
        self.bytes.take();
        Err(kind.at(self.position))
    }

    /// Return an error for an invalid value with the given tag.
    // TODO(tarcieri): compose this with `Encoder::error` after changing its return type
    pub fn value_error(&mut self, tag: Tag) -> Error {
        self.bytes.take();
        tag.value_error().kind().at(self.position)
    }

    /// Did the decoding operation fail due to an error?
    pub fn is_failed(&self) -> bool {
        self.bytes.is_none()
    }

    /// Finish encoding to the buffer, returning a slice containing the data
    /// written to the buffer.
    pub fn finish(self) -> Result<&'a [u8]> {
        let pos = self.position;
        let range = ..usize::try_from(self.position)?;

        match self.bytes {
            Some(bytes) => bytes
                .get(range)
                .ok_or_else(|| ErrorKind::Overlength.at(pos)),
            None => Err(ErrorKind::Failed.at(pos)),
        }
    }

    /// Encode the provided value as an ASN.1 `BIT STRING`.
    pub fn bit_string(&mut self, value: impl TryInto<BitString<'a>>) -> Result<()> {
        value
            .try_into()
            .map_err(|_| self.value_error(Tag::BitString))
            .and_then(|value| self.encode(&value))
    }

    /// Encode a `CONTEXT-SPECIFIC` field with `EXPLICIT` tagging.
    pub fn context_specific<T>(
        &mut self,
        tag_number: TagNumber,
        tag_mode: TagMode,
        value: &T,
    ) -> Result<()>
    where
        T: EncodeValue + Tagged,
    {
        ContextSpecificRef {
            tag_number,
            tag_mode,
            value,
        }
        .encode(self)
    }

    /// Encode the provided value as an ASN.1 `GeneralizedTime`
    pub fn generalized_time(&mut self, value: impl TryInto<GeneralizedTime>) -> Result<()> {
        value
            .try_into()
            .map_err(|_| self.value_error(Tag::GeneralizedTime))
            .and_then(|value| self.encode(&value))
    }

    /// Encode the provided value as an ASN.1 `IA5String`.
    pub fn ia5_string(&mut self, value: impl TryInto<Ia5String<'a>>) -> Result<()> {
        value
            .try_into()
            .map_err(|_| self.value_error(Tag::Ia5String))
            .and_then(|value| self.encode(&value))
    }

    /// Encode an ASN.1 `NULL` value.
    pub fn null(&mut self) -> Result<()> {
        self.encode(&Null)
    }

    /// Encode the provided value as an ASN.1 `OCTET STRING`
    pub fn octet_string(&mut self, value: impl TryInto<OctetString<'a>>) -> Result<()> {
        value
            .try_into()
            .map_err(|_| self.value_error(Tag::OctetString))
            .and_then(|value| self.encode(&value))
    }

    /// Encode an ASN.1 [`ObjectIdentifier`]
    #[cfg(feature = "oid")]
    #[cfg_attr(docsrs, doc(cfg(feature = "oid")))]
    pub fn oid(&mut self, value: impl TryInto<ObjectIdentifier>) -> Result<()> {
        value
            .try_into()
            .map_err(|_| self.value_error(Tag::ObjectIdentifier))
            .and_then(|value| self.encode(&value))
    }

    /// Encode the provided value as an ASN.1 `PrintableString`
    pub fn printable_string(&mut self, value: impl TryInto<PrintableString<'a>>) -> Result<()> {
        value
            .try_into()
            .map_err(|_| self.value_error(Tag::PrintableString))
            .and_then(|value| self.encode(&value))
    }

    /// Encode an ASN.1 `SEQUENCE` of the given length.
    ///
    /// Spawns a nested [`Encoder`] which is expected to be exactly the
    /// specified length upon completion.
    pub fn sequence<F>(&mut self, length: Length, f: F) -> Result<()>
    where
        F: FnOnce(&mut Encoder<'_>) -> Result<()>,
    {
        Header::new(Tag::Sequence, length).and_then(|header| header.encode(self))?;

        let mut nested_encoder = Encoder::new(self.reserve(length)?);
        f(&mut nested_encoder)?;

        if nested_encoder.finish()?.len() == usize::try_from(length)? {
            Ok(())
        } else {
            self.error(ErrorKind::Length { tag: Tag::Sequence })
        }
    }

    /// Encode the provided value as an ASN.1 `UTCTime`
    pub fn utc_time(&mut self, value: impl TryInto<UtcTime>) -> Result<()> {
        value
            .try_into()
            .map_err(|_| self.value_error(Tag::UtcTime))
            .and_then(|value| self.encode(&value))
    }

    /// Encode the provided value as an ASN.1 `Utf8String`
    pub fn utf8_string(&mut self, value: impl TryInto<Utf8String<'a>>) -> Result<()> {
        value
            .try_into()
            .map_err(|_| self.value_error(Tag::Utf8String))
            .and_then(|value| self.encode(&value))
    }

    /// Reserve a portion of the internal buffer, updating the internal cursor
    /// position and returning a mutable slice.
    fn reserve(&mut self, len: impl TryInto<Length>) -> Result<&mut [u8]> {
        let len = len
            .try_into()
            .or_else(|_| self.error(ErrorKind::Overflow))?;

        if len > self.remaining_len()? {
            self.error(ErrorKind::Overlength)?;
        }

        let end = (self.position + len).or_else(|e| self.error(e.kind()))?;
        let range = self.position.try_into()?..end.try_into()?;
        let position = &mut self.position;

        // TODO(tarcieri): non-panicking version of this code
        // We ensure above that the buffer is untainted and there is sufficient
        // space to perform this slicing operation, however it would be nice to
        // have fully panic-free code.
        //
        // Unfortunately tainting the buffer on error is tricky to do when
        // potentially holding a reference to the buffer, and failure to taint
        // it would not uphold the invariant that any errors should taint it.
        let slice = &mut self.bytes.as_mut().expect("DER encoder tainted")[range];
        *position = end;

        Ok(slice)
    }

    /// Encode a single byte into the backing buffer.
    pub(crate) fn byte(&mut self, byte: u8) -> Result<()> {
        match self.reserve(1u8)?.first_mut() {
            Some(b) => {
                *b = byte;
                Ok(())
            }
            None => self.error(ErrorKind::Overlength),
        }
    }

    /// Encode the provided byte slice into the backing buffer.
    pub(crate) fn bytes(&mut self, slice: &[u8]) -> Result<()> {
        self.reserve(slice.len())?.copy_from_slice(slice);
        Ok(())
    }

    /// Get the size of the buffer in bytes.
    fn buffer_len(&self) -> Result<Length> {
        self.bytes
            .as_ref()
            .map(|bytes| bytes.len())
            .ok_or_else(|| ErrorKind::Failed.at(self.position))
            .and_then(TryInto::try_into)
    }

    /// Get the number of bytes still remaining in the buffer.
    fn remaining_len(&self) -> Result<Length> {
        let buffer_len = usize::try_from(self.buffer_len()?)?;

        buffer_len
            .checked_sub(self.position.try_into()?)
            .ok_or_else(|| ErrorKind::Overlength.at(self.position))
            .and_then(TryInto::try_into)
    }
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use crate::{asn1::BitString, Encodable, ErrorKind, Length, TagMode, TagNumber};

    use super::Encoder;

    #[test]
    fn overlength_message() {
        let mut buffer = [];
        let mut encoder = Encoder::new(&mut buffer);
        let err = false.encode(&mut encoder).err().unwrap();
        assert_eq!(err.kind(), ErrorKind::Overlength);
        assert_eq!(err.position(), Some(Length::ZERO));
    }

    #[test]
    fn context_specific_with_implicit_field() {
        // From RFC8410 Section 10.3:
        // <https://datatracker.ietf.org/doc/html/rfc8410#section-10.3>
        //
        //    81  33:   [1] 00 19 BF 44 09 69 84 CD FE 85 41 BA C1 67 DC 3B
        //                  96 C8 50 86 AA 30 B6 B6 CB 0C 5C 38 AD 70 31 66
        //                  E1
        const EXPECTED_BYTES: &[u8] =
            &hex!("81210019BF44096984CDFE8541BAC167DC3B96C85086AA30B6B6CB0C5C38AD703166E1");

        let tag_number = TagNumber::new(1);
        let bit_string = BitString::from_bytes(&EXPECTED_BYTES[3..]).unwrap();

        let mut buf = [0u8; EXPECTED_BYTES.len()];
        let mut encoder = Encoder::new(&mut buf);
        encoder
            .context_specific(tag_number, TagMode::Implicit, &bit_string)
            .unwrap();

        assert_eq!(EXPECTED_BYTES, encoder.finish().unwrap());
    }
}
