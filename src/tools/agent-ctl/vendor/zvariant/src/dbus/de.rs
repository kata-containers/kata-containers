use core::convert::TryFrom;

use serde::de::{self, DeserializeSeed, EnumAccess, MapAccess, SeqAccess, Visitor};
use static_assertions::assert_impl_all;

use std::{marker::PhantomData, str};

#[cfg(unix)]
use std::os::unix::io::RawFd;

use crate::{
    de::ValueParseStage, signature_parser::SignatureParser, utils::*, Basic, EncodingContext,
    EncodingFormat, Error, ObjectPath, Result, Signature,
};

#[cfg(unix)]
use crate::Fd;

/// Our D-Bus deserialization implementation.
#[derive(Debug)]
pub struct Deserializer<'de, 'sig, 'f, B>(pub(crate) crate::DeserializerCommon<'de, 'sig, 'f, B>);

assert_impl_all!(Deserializer<'_, '_, '_, i32>: Send, Sync, Unpin);

impl<'de, 'sig, 'f, B> Deserializer<'de, 'sig, 'f, B>
where
    B: byteorder::ByteOrder,
{
    /// Create a Deserializer struct instance.
    ///
    /// On Windows, there is no `fds` argument.
    pub fn new<'r: 'de>(
        bytes: &'r [u8],
        #[cfg(unix)] fds: Option<&'f [RawFd]>,
        signature: &Signature<'sig>,
        ctxt: EncodingContext<B>,
    ) -> Self {
        assert_eq!(ctxt.format(), EncodingFormat::DBus);

        let sig_parser = SignatureParser::new(signature.clone());
        Self(crate::DeserializerCommon {
            ctxt,
            sig_parser,
            bytes,
            #[cfg(unix)]
            fds,
            #[cfg(not(unix))]
            fds: PhantomData,
            pos: 0,
            b: PhantomData,
        })
    }
}

macro_rules! deserialize_basic {
    ($method:ident $read_method:ident $visitor_method:ident($type:ty)) => {
        fn $method<V>(self, visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            let v = B::$read_method(self.0.next_const_size_slice::<$type>()?);

            visitor.$visitor_method(v)
        }
    };
}

macro_rules! deserialize_as {
    ($method:ident => $as:ident) => {
        deserialize_as!($method() => $as());
    };
    ($method:ident($($in_arg:ident: $type:ty),*) => $as:ident($($as_arg:expr),*)) => {
        #[inline]
        fn $method<V>(self, $($in_arg: $type,)* visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            self.$as($($as_arg,)* visitor)
        }
    }
}

impl<'de, 'd, 'sig, 'f, B> de::Deserializer<'de> for &'d mut Deserializer<'de, 'sig, 'f, B>
where
    B: byteorder::ByteOrder,
{
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let c = self.0.sig_parser.next_char();

        crate::de::deserialize_any::<B, Self, V>(self, c, visitor)
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let v = B::read_u32(self.0.next_const_size_slice::<bool>()?);
        let b = match v {
            1 => true,
            0 => false,
            // As per D-Bus spec, only 0 and 1 values are allowed
            _ => {
                return Err(de::Error::invalid_value(
                    de::Unexpected::Unsigned(v as u64),
                    &"0 or 1",
                ))
            }
        };

        visitor.visit_bool(b)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_i16(visitor)
    }

    deserialize_basic!(deserialize_i16 read_i16 visit_i16(i16));
    deserialize_basic!(deserialize_i64 read_i64 visit_i64(i64));
    deserialize_basic!(deserialize_u16 read_u16 visit_u16(u16));
    deserialize_basic!(deserialize_u32 read_u32 visit_u32(u32));
    deserialize_basic!(deserialize_u64 read_u64 visit_u64(u64));
    deserialize_basic!(deserialize_f64 read_f64 visit_f64(f64));

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let bytes = deserialize_ay(self)?;
        visitor.visit_byte_buf(bytes.into())
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let bytes = deserialize_ay(self)?;
        visitor.visit_borrowed_bytes(bytes)
    }

    deserialize_as!(deserialize_char => deserialize_str);
    deserialize_as!(deserialize_string => deserialize_str);
    deserialize_as!(deserialize_tuple(_l: usize) => deserialize_struct("", &[]));
    deserialize_as!(deserialize_tuple_struct(n: &'static str, _l: usize) => deserialize_struct(n, &[]));
    deserialize_as!(deserialize_struct(_n: &'static str, _f: &'static [&'static str]) => deserialize_seq());
    deserialize_as!(deserialize_map => deserialize_seq);
    deserialize_as!(deserialize_ignored_any => deserialize_any);

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let v = match self.0.sig_parser.next_char() {
            #[cfg(unix)]
            Fd::SIGNATURE_CHAR => {
                self.0.sig_parser.skip_char()?;
                let alignment = u32::alignment(EncodingFormat::DBus);
                self.0.parse_padding(alignment)?;
                let idx = B::read_u32(self.0.next_slice(alignment)?);
                self.0.get_fd(idx)?
            }
            _ => B::read_i32(self.0.next_const_size_slice::<i32>()?),
        };

        visitor.visit_i32(v)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // Endianness is irrelevant for single bytes.
        visitor.visit_u8(self.0.next_const_size_slice::<u8>().map(|bytes| bytes[0])?)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let v = B::read_f64(self.0.next_const_size_slice::<f64>()?);

        visitor.visit_f32(f64_to_f32(v))
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let len = match self.0.sig_parser.next_char() {
            Signature::SIGNATURE_CHAR | VARIANT_SIGNATURE_CHAR => {
                let len_slice = self.0.next_slice(1)?;

                len_slice[0] as usize
            }
            <&str>::SIGNATURE_CHAR | ObjectPath::SIGNATURE_CHAR => {
                let alignment = u32::alignment(EncodingFormat::DBus);
                self.0.parse_padding(alignment)?;
                let len_slice = self.0.next_slice(alignment)?;

                B::read_u32(len_slice) as usize
            }
            c => {
                let expected = format!(
                    "`{}`, `{}`, `{}` or `{}`",
                    <&str>::SIGNATURE_STR,
                    Signature::SIGNATURE_STR,
                    ObjectPath::SIGNATURE_STR,
                    VARIANT_SIGNATURE_CHAR,
                );
                return Err(de::Error::invalid_type(
                    de::Unexpected::Char(c),
                    &expected.as_str(),
                ));
            }
        };
        let slice = self.0.next_slice(len)?;
        if slice.contains(&0) {
            return Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Char('\0'),
                &"D-Bus string type must not contain interior null bytes",
            ));
        }
        self.0.pos += 1; // skip trailing null byte
        let s = str::from_utf8(slice).map_err(Error::Utf8)?;
        self.0.sig_parser.skip_char()?;

        visitor.visit_borrowed_str(s)
    }

    fn deserialize_option<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unreachable!("DBus format can't support `Option`");
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.0.sig_parser.next_char() {
            VARIANT_SIGNATURE_CHAR => {
                let value_de = ValueDeserializer::new(self);

                visitor.visit_seq(value_de)
            }
            ARRAY_SIGNATURE_CHAR => {
                self.0.sig_parser.skip_char()?;
                let next_signature_char = self.0.sig_parser.next_char();
                let array_de = ArrayDeserializer::new(self)?;

                if next_signature_char == DICT_ENTRY_SIG_START_CHAR {
                    visitor.visit_map(ArrayMapDeserializer(array_de))
                } else {
                    visitor.visit_seq(ArraySeqDeserializer(array_de))
                }
            }
            STRUCT_SIG_START_CHAR => {
                let signature = self.0.sig_parser.next_signature()?;
                let alignment = alignment_for_signature(&signature, EncodingFormat::DBus);
                self.0.parse_padding(alignment)?;

                self.0.sig_parser.skip_char()?;

                visitor.visit_seq(StructureDeserializer { de: self })
            }
            c => Err(de::Error::invalid_type(
                de::Unexpected::Char(c),
                &format!(
                    "`{}`, `{}` or `{}`",
                    VARIANT_SIGNATURE_CHAR, ARRAY_SIGNATURE_CHAR, STRUCT_SIG_START_CHAR,
                )
                .as_str(),
            )),
        }
    }

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let signature = self.0.sig_parser.next_signature()?;
        let alignment = alignment_for_signature(&signature, self.0.ctxt.format());
        self.0.parse_padding(alignment)?;

        let non_unit = if self.0.sig_parser.next_char() == STRUCT_SIG_START_CHAR {
            // This means we've a non-unit enum. Let's skip the `(`.
            self.0.sig_parser.skip_char()?;

            true
        } else {
            false
        };

        let v = visitor.visit_enum(crate::de::Enum {
            de: &mut *self,
            name,
            phantom: PhantomData,
        })?;

        if non_unit {
            // For non-unit enum, we need to skip the closing paren.
            self.0.sig_parser.skip_char()?;
        }

        Ok(v)
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.0.sig_parser.next_char() == <&str>::SIGNATURE_CHAR {
            self.deserialize_str(visitor)
        } else {
            self.deserialize_u32(visitor)
        }
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

struct ArrayDeserializer<'d, 'de, 'sig, 'f, B> {
    de: &'d mut Deserializer<'de, 'sig, 'f, B>,
    len: usize,
    start: usize,
    // alignment of element
    element_alignment: usize,
    // where value signature starts
    element_signature_len: usize,
}

impl<'d, 'de, 'sig, 'f, B> ArrayDeserializer<'d, 'de, 'sig, 'f, B>
where
    B: byteorder::ByteOrder,
{
    fn new(de: &'d mut Deserializer<'de, 'sig, 'f, B>) -> Result<Self> {
        de.0.parse_padding(ARRAY_ALIGNMENT_DBUS)?;

        let len = B::read_u32(de.0.next_slice(4)?) as usize;
        let element_signature = de.0.sig_parser.next_signature()?;
        let element_alignment = alignment_for_signature(&element_signature, EncodingFormat::DBus);
        let mut element_signature_len = element_signature.len();

        // D-Bus requires padding for the first element even when there is no first element
        // (i-e empty array) so we parse padding already.
        de.0.parse_padding(element_alignment)?;
        let start = de.0.pos;

        if de.0.sig_parser.next_char() == DICT_ENTRY_SIG_START_CHAR {
            de.0.sig_parser.skip_char()?;
            element_signature_len -= 1;
        }

        Ok(Self {
            de,
            len,
            start,
            element_alignment,
            element_signature_len,
        })
    }

    fn next<T>(&mut self, seed: T, sig_parser: SignatureParser<'_>) -> Result<T::Value>
    where
        T: DeserializeSeed<'de>,
    {
        let ctxt = EncodingContext::new_dbus(self.de.0.ctxt.position() + self.de.0.pos);

        let mut de = Deserializer::<B>(crate::DeserializerCommon {
            ctxt,
            sig_parser,
            bytes: &self.de.0.bytes[self.de.0.pos..],
            fds: self.de.0.fds,
            pos: 0,
            b: PhantomData,
        });
        let v = seed.deserialize(&mut de);
        self.de.0.pos += de.0.pos;

        if self.de.0.pos > self.start + self.len {
            return Err(serde::de::Error::invalid_length(
                self.len,
                &format!(">= {}", self.de.0.pos - self.start).as_str(),
            ));
        }

        v
    }

    fn next_element<T>(
        &mut self,
        seed: T,
        sig_parser: SignatureParser<'_>,
    ) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        if self.done() {
            self.de
                .0
                .sig_parser
                .skip_chars(self.element_signature_len)?;

            return Ok(None);
        }

        self.de.0.parse_padding(self.element_alignment)?;

        self.next(seed, sig_parser).map(Some)
    }

    fn done(&self) -> bool {
        self.de.0.pos == self.start + self.len
    }
}

fn deserialize_ay<'de, 'sig, 'f, B>(de: &mut Deserializer<'de, 'sig, 'f, B>) -> Result<&'de [u8]>
where
    B: byteorder::ByteOrder,
{
    if de.0.sig_parser.next_signature()? != "ay" {
        return Err(de::Error::invalid_type(de::Unexpected::Seq, &"ay"));
    }

    de.0.sig_parser.skip_char()?;
    let ad = ArrayDeserializer::new(de)?;
    let len = ad.len;
    de.0.sig_parser.skip_char()?;
    de.0.next_slice(len)
}

struct ArraySeqDeserializer<'d, 'de, 'sig, 'f, B>(ArrayDeserializer<'d, 'de, 'sig, 'f, B>);

impl<'d, 'de, 'sig, 'f, B> SeqAccess<'de> for ArraySeqDeserializer<'d, 'de, 'sig, 'f, B>
where
    B: byteorder::ByteOrder,
{
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        let sig_parser = self.0.de.0.sig_parser.clone();
        self.0.next_element(seed, sig_parser)
    }
}

struct ArrayMapDeserializer<'d, 'de, 'sig, 'f, B>(ArrayDeserializer<'d, 'de, 'sig, 'f, B>);

impl<'d, 'de, 'sig, 'f, B> MapAccess<'de> for ArrayMapDeserializer<'d, 'de, 'sig, 'f, B>
where
    B: byteorder::ByteOrder,
{
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de>,
    {
        let sig_parser = self.0.de.0.sig_parser.clone();
        self.0.next_element(seed, sig_parser)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        let mut sig_parser = self.0.de.0.sig_parser.clone();
        // Skip key signature (always 1 char)
        sig_parser.skip_char()?;
        self.0.next(seed, sig_parser)
    }
}

#[derive(Debug)]
struct StructureDeserializer<'d, 'de, 'sig, 'f, B> {
    de: &'d mut Deserializer<'de, 'sig, 'f, B>,
}

impl<'d, 'de, 'sig, 'f, B> SeqAccess<'de> for StructureDeserializer<'d, 'de, 'sig, 'f, B>
where
    B: byteorder::ByteOrder,
{
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        let v = seed.deserialize(&mut *self.de).map(Some);

        if self.de.0.sig_parser.next_char() == STRUCT_SIG_END_CHAR {
            // Last item in the struct
            self.de.0.sig_parser.skip_char()?;
        }

        v
    }
}

#[derive(Debug)]
struct ValueDeserializer<'d, 'de, 'sig, 'f, B> {
    de: &'d mut Deserializer<'de, 'sig, 'f, B>,
    stage: ValueParseStage,
    sig_start: usize,
}

impl<'d, 'de, 'sig, 'f, B> ValueDeserializer<'d, 'de, 'sig, 'f, B>
where
    B: byteorder::ByteOrder,
{
    fn new(de: &'d mut Deserializer<'de, 'sig, 'f, B>) -> Self {
        let sig_start = de.0.pos;
        ValueDeserializer::<B> {
            de,
            stage: ValueParseStage::Signature,
            sig_start,
        }
    }
}

impl<'d, 'de, 'sig, 'f, B> SeqAccess<'de> for ValueDeserializer<'d, 'de, 'sig, 'f, B>
where
    B: byteorder::ByteOrder,
{
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        match self.stage {
            ValueParseStage::Signature => {
                self.stage = ValueParseStage::Value;

                seed.deserialize(&mut *self.de).map(Some)
            }
            ValueParseStage::Value => {
                self.stage = ValueParseStage::Done;

                let sig_len = self.de.0.bytes[self.sig_start] as usize;
                // skip length byte
                let sig_start = self.sig_start + 1;
                let sig_end = sig_start + sig_len;
                // Skip trailing nul byte
                let value_start = sig_end + 1;

                let slice = &self.de.0.bytes[sig_start..sig_end];
                // FIXME: Can we just use `Signature::from_bytes_unchecked`?
                let signature = Signature::try_from(slice)?;
                let sig_parser = SignatureParser::new(signature);

                let ctxt = EncodingContext::new(
                    EncodingFormat::DBus,
                    self.de.0.ctxt.position() + value_start,
                );
                let mut de = Deserializer::<B>(crate::DeserializerCommon {
                    ctxt,
                    sig_parser,
                    bytes: &self.de.0.bytes[value_start..],
                    fds: self.de.0.fds,
                    pos: 0,
                    b: PhantomData,
                });

                let v = seed.deserialize(&mut de).map(Some);
                self.de.0.pos += de.0.pos;

                v
            }
            ValueParseStage::Done => Ok(None),
        }
    }
}

impl<'de, 'd, 'sig, 'f, B> crate::de::GetDeserializeCommon<'de, 'sig, 'f, B>
    for &'d mut Deserializer<'de, 'sig, 'f, B>
where
    B: byteorder::ByteOrder,
{
    fn common_mut<'dr>(self) -> &'dr mut crate::de::DeserializerCommon<'de, 'sig, 'f, B>
    where
        Self: 'dr,
    {
        &mut self.0
    }
}

impl<'de, 'd, 'sig, 'f, B> EnumAccess<'de>
    for crate::de::Enum<B, &'d mut Deserializer<'de, 'sig, 'f, B>>
where
    B: byteorder::ByteOrder,
{
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: DeserializeSeed<'de>,
    {
        seed.deserialize(&mut *self.de).map(|v| (v, self))
    }
}
