mod boolean;
mod integer;
mod null;
mod sequence;
mod utf8_string;

use crate::de::boolean::Boolean;
use crate::de::integer::UnsignedInteger;
use crate::de::null::Null;
use crate::de::sequence::Sequence;
use crate::de::utf8_string::Utf8String;
use crate::misc::{Length, PeekableReader, ReadExt};
use crate::{Asn1DerError, Asn1RawDer, Result};
use picky_asn1::tag::Tag;
use picky_asn1::wrapper::*;
use picky_asn1::Asn1Type;
use serde::de::Visitor;
use serde::Deserialize;
use std::io::{Cursor, Read};

const DEFAULT_MAX_LEN: usize = 10240;

/// Deserializes `T` from `bytes`
pub fn from_bytes<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T> {
    debug_log!("deserialization using `from_bytes`");
    let mut deserializer = Deserializer::new_from_bytes(bytes);
    T::deserialize(&mut deserializer)
}

/// Deserializes `T` from `reader`
pub fn from_reader<'a, T: Deserialize<'a>>(reader: impl Read + 'a) -> Result<T> {
    from_reader_with_max_len(reader, DEFAULT_MAX_LEN)
}

/// Deserializes `T` from `reader` reading at most n bytes.
pub fn from_reader_with_max_len<'a, T: Deserialize<'a>>(reader: impl Read + 'a, max_len: usize) -> Result<T> {
    debug_log!(
        "deserialization using `from_reader_with_max_len`, max_len = {}",
        max_len
    );
    let mut deserializer = Deserializer::new_from_reader(reader, max_len);
    T::deserialize(&mut deserializer)
}

/// An ASN.1-DER deserializer for `serde`
pub struct Deserializer<'de> {
    reader: PeekableReader<Box<dyn Read + 'de>>,
    buf: Vec<u8>,
    encapsulator_tag_stack: Vec<Tag>,
    header_only: bool,
    raw_der: bool,
    max_len: usize,
}

impl<'de> Deserializer<'de> {
    /// Creates a new deserializer over `bytes`
    pub fn new_from_bytes(bytes: &'de [u8]) -> Self {
        Self::new_from_reader(Cursor::new(bytes), bytes.len())
    }
    /// Creates a new deserializer for `reader`
    pub fn new_from_reader(reader: impl Read + 'de, max_len: usize) -> Self {
        Self {
            reader: PeekableReader::new(Box::new(reader)),
            buf: Vec::new(),
            encapsulator_tag_stack: Vec::with_capacity(3),
            header_only: false,
            raw_der: false,
            max_len,
        }
    }

    /// Reads tag and length of the next DER object
    fn h_next_tag_len(&mut self) -> Result<(Tag, usize)> {
        // Read type and length
        let tag = Tag::from(self.reader.read_one()?);
        let len = Length::deserialized(&mut self.reader)?;
        Ok((tag, len))
    }

    /// Reads the next DER object into `self.buf` and returns the tag
    fn h_next_object(&mut self) -> Result<Tag> {
        let (tag, len) = match self.h_decapsulate()? {
            Some((tag, len)) if tag.is_primitive() && !tag.is_universal() => (tag, len),
            _ => {
                if self.raw_der {
                    self.raw_der = false;
                    let peeked = self.reader.peek_buffer()?;
                    let msg_len = Length::deserialized(&mut Cursor::new(&peeked.buffer()[1..]))?;
                    let header_len = Length::encoded_len(msg_len) + 1;
                    (Tag::from(peeked.buffer()[0]), header_len + msg_len)
                } else {
                    let tag = Tag::from(self.reader.read_one()?);
                    let len = Length::deserialized(&mut self.reader)?;
                    (tag, len)
                }
            }
        };

        debug_log!("object read: {} (len = {})", tag, len);

        if len > self.max_len {
            debug_log!("TRUNCATED DATA (invalid len: found {}, max is {})", len, self.max_len);
            return Err(Asn1DerError::TruncatedData);
        }

        self.buf.resize(len, 0);
        self.reader.read_exact(self.buf.as_mut_slice())?;

        debug_log!("object buffer: {:02X?}", self.buf);

        Ok(tag)
    }

    /// Peek next DER object tag (ignoring encapsulator)
    fn h_peek_object(&mut self) -> Result<Tag> {
        if self.encapsulator_tag_stack.is_empty() {
            Ok(Tag::from(self.reader.peek_one()?))
        } else {
            let peeked = self.reader.peek_buffer()?;
            let mut cursor = 0;
            for encapsulator_tag in self
                .encapsulator_tag_stack
                .iter()
                .filter(|tag| tag.is_constructed() || **tag == Tag::BIT_STRING || **tag == Tag::OCTET_STRING)
            {
                let encapsulator_tag = *encapsulator_tag;
                debug_log!("encapsulator: {}", encapsulator_tag);

                if peeked.len() < cursor + 2 {
                    debug_log!("peek_object: TRUNCATED DATA (couldn't read encapsulator tag or length)");
                    return Err(Asn1DerError::TruncatedData);
                }

                // check tag
                if peeked.buffer()[cursor] != encapsulator_tag.inner() {
                    debug_log!(
                        "peek_object: INVALID (found {}, expected encapsulator tag {})",
                        Tag::from(peeked.buffer()[cursor]),
                        encapsulator_tag
                    );
                    self.encapsulator_tag_stack.clear();
                    return Err(Asn1DerError::InvalidData);
                }

                let length = {
                    let len = Length::deserialized(&mut Cursor::new(&peeked.buffer()[cursor + 1..]))?;
                    Length::encoded_len(len)
                };

                cursor = if encapsulator_tag == BitStringAsn1Container::<()>::TAG {
                    cursor + length + 2
                } else {
                    cursor + length + 1
                };
            }

            if peeked.len() <= cursor {
                debug_log!("peek_object: TRUNCATED DATA (couldn't read object tag)");
                return Err(Asn1DerError::TruncatedData);
            }

            Ok(Tag::from(peeked.buffer()[cursor]))
        }
    }

    fn h_encapsulate(&mut self, tag: Tag) {
        debug_log!("{} pushed as encapsulator", tag);
        self.encapsulator_tag_stack.push(tag);
    }

    fn h_decapsulate(&mut self) -> Result<Option<(Tag, usize)>> {
        if self.encapsulator_tag_stack.is_empty() {
            Ok(None)
        } else {
            let mut tag = Tag::NULL;
            let mut len = 0;
            for encapsulator_tag in &self.encapsulator_tag_stack {
                let encapsulator_tag = *encapsulator_tag;

                tag = Tag::from(self.reader.peek_one()?);
                if tag == encapsulator_tag {
                    self.reader.read_one()?; // discard it
                } else {
                    debug_log!(
                        "decapsulate: INVALID (found {}, expected encapsulator tag {})",
                        tag,
                        encapsulator_tag
                    );
                    // we need to clear the stack otherwise it'll contain unwanted tags on the next serialization
                    self.encapsulator_tag_stack.clear();
                    return Err(Asn1DerError::InvalidData);
                }

                len = Length::deserialized(&mut self.reader)?;

                if encapsulator_tag == Tag::BIT_STRING {
                    self.reader.read_one()?; // unused bits count
                }
            }

            self.encapsulator_tag_stack.clear();
            Ok(Some((tag, len)))
        }
    }
}

impl<'de, 'a> serde::de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Asn1DerError;

    fn is_human_readable(&self) -> bool {
        false
    }

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_any");
        match self.h_peek_object()? {
            Tag::BOOLEAN => self.deserialize_bool(visitor),
            Tag::INTEGER => {
                debug_log!("deserialize_any: can't be used on INTEGER");
                Err(Asn1DerError::InvalidData)
            }
            Tag::NULL => self.deserialize_unit(visitor),
            Tag::OCTET_STRING => self.deserialize_byte_buf(visitor),
            Tag::SEQUENCE => self.deserialize_seq(visitor),
            Tag::UTF8_STRING => self.deserialize_string(visitor),
            Tag::BMP_STRING => self.deserialize_string(visitor),
            Tag::OID => self.deserialize_bytes(visitor),
            Tag::BIT_STRING => self.deserialize_byte_buf(visitor),
            Tag::UTC_TIME => self.deserialize_bytes(visitor),
            Tag::GENERALIZED_TIME => self.deserialize_bytes(visitor),
            Tag::PRINTABLE_STRING => self.deserialize_byte_buf(visitor),
            Tag::NUMERIC_STRING => self.deserialize_byte_buf(visitor),
            Tag::IA5_STRING => self.deserialize_byte_buf(visitor),
            Tag::GENERAL_STRING => self.deserialize_byte_buf(visitor),
            ExplicitContextTag0::<()>::TAG => self.deserialize_newtype_struct(ExplicitContextTag0::<()>::NAME, visitor),
            ExplicitContextTag1::<()>::TAG => self.deserialize_newtype_struct(ExplicitContextTag1::<()>::NAME, visitor),
            ExplicitContextTag2::<()>::TAG => self.deserialize_newtype_struct(ExplicitContextTag2::<()>::NAME, visitor),
            ExplicitContextTag3::<()>::TAG => self.deserialize_newtype_struct(ExplicitContextTag3::<()>::NAME, visitor),
            ExplicitContextTag4::<()>::TAG => self.deserialize_newtype_struct(ExplicitContextTag4::<()>::NAME, visitor),
            ExplicitContextTag5::<()>::TAG => self.deserialize_newtype_struct(ExplicitContextTag5::<()>::NAME, visitor),
            ExplicitContextTag6::<()>::TAG => self.deserialize_newtype_struct(ExplicitContextTag6::<()>::NAME, visitor),
            ExplicitContextTag7::<()>::TAG => self.deserialize_newtype_struct(ExplicitContextTag7::<()>::NAME, visitor),
            ExplicitContextTag8::<()>::TAG => self.deserialize_newtype_struct(ExplicitContextTag8::<()>::NAME, visitor),
            ExplicitContextTag9::<()>::TAG => self.deserialize_newtype_struct(ExplicitContextTag9::<()>::NAME, visitor),
            ExplicitContextTag10::<()>::TAG => {
                self.deserialize_newtype_struct(ExplicitContextTag10::<()>::NAME, visitor)
            }
            ExplicitContextTag11::<()>::TAG => {
                self.deserialize_newtype_struct(ExplicitContextTag11::<()>::NAME, visitor)
            }
            ExplicitContextTag12::<()>::TAG => {
                self.deserialize_newtype_struct(ExplicitContextTag12::<()>::NAME, visitor)
            }
            ExplicitContextTag13::<()>::TAG => {
                self.deserialize_newtype_struct(ExplicitContextTag13::<()>::NAME, visitor)
            }
            ExplicitContextTag14::<()>::TAG => {
                self.deserialize_newtype_struct(ExplicitContextTag14::<()>::NAME, visitor)
            }
            ExplicitContextTag15::<()>::TAG => {
                self.deserialize_newtype_struct(ExplicitContextTag15::<()>::NAME, visitor)
            }
            ImplicitContextTag0::<()>::TAG => self.deserialize_newtype_struct(ImplicitContextTag0::<()>::NAME, visitor),
            ImplicitContextTag1::<()>::TAG => self.deserialize_newtype_struct(ImplicitContextTag1::<()>::NAME, visitor),
            ImplicitContextTag2::<()>::TAG => self.deserialize_newtype_struct(ImplicitContextTag2::<()>::NAME, visitor),
            ImplicitContextTag3::<()>::TAG => self.deserialize_newtype_struct(ImplicitContextTag3::<()>::NAME, visitor),
            ImplicitContextTag4::<()>::TAG => self.deserialize_newtype_struct(ImplicitContextTag4::<()>::NAME, visitor),
            ImplicitContextTag5::<()>::TAG => self.deserialize_newtype_struct(ImplicitContextTag5::<()>::NAME, visitor),
            ImplicitContextTag6::<()>::TAG => self.deserialize_newtype_struct(ImplicitContextTag6::<()>::NAME, visitor),
            ImplicitContextTag7::<()>::TAG => self.deserialize_newtype_struct(ImplicitContextTag7::<()>::NAME, visitor),
            ImplicitContextTag8::<()>::TAG => self.deserialize_newtype_struct(ImplicitContextTag8::<()>::NAME, visitor),
            ImplicitContextTag9::<()>::TAG => self.deserialize_newtype_struct(ImplicitContextTag9::<()>::NAME, visitor),
            ImplicitContextTag10::<()>::TAG => {
                self.deserialize_newtype_struct(ImplicitContextTag10::<()>::NAME, visitor)
            }
            ImplicitContextTag11::<()>::TAG => {
                self.deserialize_newtype_struct(ImplicitContextTag11::<()>::NAME, visitor)
            }
            ImplicitContextTag12::<()>::TAG => {
                self.deserialize_newtype_struct(ImplicitContextTag12::<()>::NAME, visitor)
            }
            ImplicitContextTag13::<()>::TAG => {
                self.deserialize_newtype_struct(ImplicitContextTag13::<()>::NAME, visitor)
            }
            ImplicitContextTag14::<()>::TAG => {
                self.deserialize_newtype_struct(ImplicitContextTag14::<()>::NAME, visitor)
            }
            ImplicitContextTag15::<()>::TAG => {
                self.deserialize_newtype_struct(ImplicitContextTag15::<()>::NAME, visitor)
            }
            _ => {
                debug_log!("deserialize_any: INVALID");
                Err(Asn1DerError::InvalidData)
            }
        }
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_bool");
        match self.h_peek_object()? {
            Tag::BOOLEAN => {}
            tag if tag.is_primitive() && !tag.is_universal() => {}
            _tag => {
                debug_log!("deserialize_bool: INVALID (found {})", _tag);
                return Err(Asn1DerError::InvalidData);
            }
        }
        self.h_next_object()?;
        visitor.visit_bool(Boolean::deserialize(&self.buf)?)
    }

    fn deserialize_i8<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_i8: UNSUPPORTED");
        Err(Asn1DerError::UnsupportedType)
    }

    fn deserialize_i16<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_i16: UNSUPPORTED");
        Err(Asn1DerError::UnsupportedType)
    }

    fn deserialize_i32<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_i32: UNSUPPORTED");
        Err(Asn1DerError::UnsupportedType)
    }

    fn deserialize_i64<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_i64: UNSUPPORTED");
        Err(Asn1DerError::UnsupportedType)
    }

    fn deserialize_i128<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_i128: UNSUPPORTED");
        Err(Asn1DerError::UnsupportedType)
    }

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_u8");
        match self.h_peek_object()? {
            Tag::INTEGER => {}
            tag if tag.is_primitive() && !tag.is_universal() => {}
            _tag => {
                debug_log!("deserialize_u8: INVALID (found {})", _tag);
                return Err(Asn1DerError::InvalidData);
            }
        }
        self.h_next_object()?;
        visitor.visit_u8(UnsignedInteger::deserialize(&self.buf)?)
    }

    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_u16");
        match self.h_peek_object()? {
            Tag::INTEGER => {}
            tag if tag.is_primitive() && !tag.is_universal() => {}
            _tag => {
                debug_log!("deserialize_u16: INVALID (found {})", _tag);
                return Err(Asn1DerError::InvalidData);
            }
        }
        self.h_next_object()?;
        visitor.visit_u16(UnsignedInteger::deserialize(&self.buf)?)
    }

    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_u32");
        match self.h_peek_object()? {
            Tag::INTEGER => {}
            tag if tag.is_primitive() && !tag.is_universal() => {}
            _tag => {
                debug_log!("deserialize_u32: INVALID (found {})", _tag);
                return Err(Asn1DerError::InvalidData);
            }
        }
        self.h_next_object()?;
        visitor.visit_u32(UnsignedInteger::deserialize(&self.buf)?)
    }

    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_u64");
        match self.h_peek_object()? {
            Tag::INTEGER => {}
            tag if tag.is_primitive() && !tag.is_universal() => {}
            _tag => {
                debug_log!("deserialize_u64: INVALID (found {})", _tag);
                return Err(Asn1DerError::InvalidData);
            }
        }
        self.h_next_object()?;
        visitor.visit_u64(UnsignedInteger::deserialize(&self.buf)?)
    }

    fn deserialize_u128<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_u128");
        match self.h_peek_object()? {
            Tag::INTEGER => {}
            tag if tag.is_primitive() && !tag.is_universal() => {}
            _tag => {
                debug_log!("deserialize_u128: INVALID (found {})", _tag);
                return Err(Asn1DerError::InvalidData);
            }
        }
        self.h_next_object()?;
        visitor.visit_u128(UnsignedInteger::deserialize(&self.buf)?)
    }

    fn deserialize_f32<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_f32: UNSUPPORTED");
        Err(Asn1DerError::UnsupportedType)
    }

    fn deserialize_f64<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_f64: UNSUPPORTED");
        Err(Asn1DerError::UnsupportedType)
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_char");
        match self.h_peek_object()? {
            Tag::UTF8_STRING => {}
            Tag::BMP_STRING => {}
            tag if tag.is_primitive() && !tag.is_universal() => {}
            _tag => {
                debug_log!("deserialize_char: INVALID (found {})", _tag);
                return Err(Asn1DerError::InvalidData);
            }
        }

        self.h_next_object()?;
        let s = Utf8String::deserialize(&self.buf)?;

        let c = s.chars().next().ok_or(Asn1DerError::UnsupportedValue)?;
        visitor.visit_char(c)
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_str");
        match self.h_peek_object()? {
            Tag::UTF8_STRING => {}
            Tag::BMP_STRING => {}
            tag if tag.is_primitive() && !tag.is_universal() => {}
            _tag => {
                debug_log!("deserialize_str: INVALID (found {})", _tag);
                return Err(Asn1DerError::InvalidData);
            }
        }
        self.h_next_object()?;
        visitor.visit_str(Utf8String::deserialize(&self.buf)?)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_string");
        match self.h_peek_object()? {
            Tag::UTF8_STRING => {}
            Tag::BMP_STRING => {}
            tag if tag.is_primitive() && !tag.is_universal() => {}
            _tag => {
                debug_log!("deserialize_string: INVALID (found {})", _tag);
                return Err(Asn1DerError::InvalidData);
            }
        }
        self.h_next_object()?;
        visitor.visit_string(Utf8String::deserialize(&self.buf)?.to_string())
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_bytes");
        match self.h_peek_object()? {
            Tag::OCTET_STRING => {}
            Tag::OID => {}
            Tag::BIT_STRING => {}
            Tag::INTEGER => {}
            Tag::UTC_TIME => {}
            Tag::GENERALIZED_TIME => {}
            tag if tag.is_primitive() && !tag.is_universal() => {}
            _tag => {
                if self.header_only {
                    self.header_only = false;
                    self.buf.resize(2, 0);
                    self.reader.read_exact(&mut self.buf)?;
                    return visitor.visit_bytes(&self.buf);
                }

                debug_log!("deserialize_bytes: INVALID (found {})", _tag);
                return Err(Asn1DerError::InvalidData);
            }
        }

        self.h_next_object()?;
        visitor.visit_bytes(&self.buf)
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_byte_buf");
        match self.h_peek_object()? {
            Tag::OCTET_STRING => {}
            Tag::BIT_STRING => {}
            Tag::INTEGER => {}
            Tag::UTF8_STRING => {}
            Tag::BMP_STRING => {}
            Tag::PRINTABLE_STRING => {}
            Tag::NUMERIC_STRING => {}
            Tag::IA5_STRING => {}
            Tag::GENERAL_STRING => {}
            tag if (tag.is_primitive() && !tag.is_universal()) || self.raw_der => {}
            _tag => {
                debug_log!("deserialize_byte_buf: INVALID (found {})", _tag);
                return Err(Asn1DerError::InvalidData);
            }
        }
        self.h_next_object()?;
        visitor.visit_byte_buf(self.buf.to_vec())
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_option");
        visitor.visit_some(self)
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_unit");
        match self.h_peek_object()? {
            Tag::NULL => {}
            tag if tag.is_primitive() && !tag.is_universal() => {}
            _tag => {
                debug_log!("deserialize_unit: INVALID (found {})", _tag);
                return Err(Asn1DerError::InvalidData);
            }
        }
        self.h_next_object()?;
        Null::deserialize(&self.buf)?;
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(self, _name: &'static str, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_unit_struct");
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(self, name: &'static str, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_newtype_struct: {}", name);
        match name {
            BitStringAsn1Container::<()>::NAME => self.h_encapsulate(Tag::BIT_STRING),
            OctetStringAsn1Container::<()>::NAME => self.h_encapsulate(Tag::OCTET_STRING),
            ExplicitContextTag0::<()>::NAME => self.h_encapsulate(ExplicitContextTag0::<()>::TAG),
            ExplicitContextTag1::<()>::NAME => self.h_encapsulate(ExplicitContextTag1::<()>::TAG),
            ExplicitContextTag2::<()>::NAME => self.h_encapsulate(ExplicitContextTag2::<()>::TAG),
            ExplicitContextTag3::<()>::NAME => self.h_encapsulate(ExplicitContextTag3::<()>::TAG),
            ExplicitContextTag4::<()>::NAME => self.h_encapsulate(ExplicitContextTag4::<()>::TAG),
            ExplicitContextTag5::<()>::NAME => self.h_encapsulate(ExplicitContextTag5::<()>::TAG),
            ExplicitContextTag6::<()>::NAME => self.h_encapsulate(ExplicitContextTag6::<()>::TAG),
            ExplicitContextTag7::<()>::NAME => self.h_encapsulate(ExplicitContextTag7::<()>::TAG),
            ExplicitContextTag8::<()>::NAME => self.h_encapsulate(ExplicitContextTag8::<()>::TAG),
            ExplicitContextTag9::<()>::NAME => self.h_encapsulate(ExplicitContextTag9::<()>::TAG),
            ExplicitContextTag10::<()>::NAME => self.h_encapsulate(ExplicitContextTag10::<()>::TAG),
            ExplicitContextTag11::<()>::NAME => self.h_encapsulate(ExplicitContextTag11::<()>::TAG),
            ExplicitContextTag12::<()>::NAME => self.h_encapsulate(ExplicitContextTag12::<()>::TAG),
            ExplicitContextTag13::<()>::NAME => self.h_encapsulate(ExplicitContextTag13::<()>::TAG),
            ExplicitContextTag14::<()>::NAME => self.h_encapsulate(ExplicitContextTag14::<()>::TAG),
            ExplicitContextTag15::<()>::NAME => self.h_encapsulate(ExplicitContextTag15::<()>::TAG),
            ImplicitContextTag0::<()>::NAME => self.h_encapsulate(ImplicitContextTag0::<()>::TAG),
            ImplicitContextTag1::<()>::NAME => self.h_encapsulate(ImplicitContextTag1::<()>::TAG),
            ImplicitContextTag2::<()>::NAME => self.h_encapsulate(ImplicitContextTag2::<()>::TAG),
            ImplicitContextTag3::<()>::NAME => self.h_encapsulate(ImplicitContextTag3::<()>::TAG),
            ImplicitContextTag4::<()>::NAME => self.h_encapsulate(ImplicitContextTag4::<()>::TAG),
            ImplicitContextTag5::<()>::NAME => self.h_encapsulate(ImplicitContextTag5::<()>::TAG),
            ImplicitContextTag6::<()>::NAME => self.h_encapsulate(ImplicitContextTag6::<()>::TAG),
            ImplicitContextTag7::<()>::NAME => self.h_encapsulate(ImplicitContextTag7::<()>::TAG),
            ImplicitContextTag8::<()>::NAME => self.h_encapsulate(ImplicitContextTag8::<()>::TAG),
            ImplicitContextTag9::<()>::NAME => self.h_encapsulate(ImplicitContextTag9::<()>::TAG),
            ImplicitContextTag10::<()>::NAME => self.h_encapsulate(ImplicitContextTag10::<()>::TAG),
            ImplicitContextTag11::<()>::NAME => self.h_encapsulate(ImplicitContextTag11::<()>::TAG),
            ImplicitContextTag12::<()>::NAME => self.h_encapsulate(ImplicitContextTag12::<()>::TAG),
            ImplicitContextTag13::<()>::NAME => self.h_encapsulate(ImplicitContextTag13::<()>::TAG),
            ImplicitContextTag14::<()>::NAME => self.h_encapsulate(ImplicitContextTag14::<()>::TAG),
            ImplicitContextTag15::<()>::NAME => self.h_encapsulate(ImplicitContextTag15::<()>::TAG),
            HeaderOnly::<()>::NAME => self.header_only = true,
            Asn1RawDer::NAME => self.raw_der = true,
            _ => {}
        }

        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_seq");

        self.h_decapsulate()?;

        // Read tag and length
        let (tag, len) = self.h_next_tag_len()?;
        debug_log!("tag: {}, len: {}", tag, len);
        if !tag.is_constructed() {
            debug_log!("deserialize_seq: INVALID (found {})", tag);
            return Err(Asn1DerError::InvalidData);
        }

        visitor.visit_seq(Sequence::deserialize_lazy(self, len))
    }
    fn deserialize_tuple<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_tuple: {}", _len);
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value> {
        debug_log!("deserialize_tuple_struct: {}({})", _name, _len);
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_map: UNSUPPORTED");
        Err(Asn1DerError::UnsupportedType)
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        debug_log!("deserialize_struct: {}", _name);
        self.deserialize_seq(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        debug_log!("deserialize_enum: deserialize sequence as choice");
        let peeked = self.reader.peek_buffer()?;
        if peeked.len() < 2 {
            debug_log!("TRUNCATED DATA (couldn't read length)");
            return Err(Asn1DerError::TruncatedData);
        }
        let payload_len = Length::deserialized(&mut Cursor::new(&peeked.buffer()[1..]))?;
        let len = 1 + payload_len + Length::encoded_len(payload_len);
        visitor.visit_seq(Sequence::deserialize_lazy(self, len))
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_identifier: peek next tag id");
        let tag = self.h_peek_object()?;
        debug_log!("next tag id: {}", tag);
        visitor.visit_u8(tag.inner())
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        debug_log!("deserialize_ignored_any");

        // Skip tag
        self.reader.read_one()?;

        // Read len and copy payload into `self.buf`
        let len = Length::deserialized(&mut self.reader)?;
        self.buf.resize(len, 0);
        self.reader.read_exact(&mut self.buf)?;

        visitor.visit_unit()
    }
}
