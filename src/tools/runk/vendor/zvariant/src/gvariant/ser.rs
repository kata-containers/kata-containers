use serde::{ser, ser::SerializeSeq, Serialize};
use static_assertions::assert_impl_all;
use std::{
    io::{Seek, Write},
    marker::PhantomData,
    str,
};

#[cfg(unix)]
use std::os::unix::io::RawFd;

use crate::{
    framing_offset_size::FramingOffsetSize, framing_offsets::FramingOffsets,
    signature_parser::SignatureParser, utils::*, Basic, EncodingContext, EncodingFormat, Error,
    Result, Signature,
};

/// Our serialization implementation.
pub struct Serializer<'ser, 'sig, B, W>(pub(crate) crate::SerializerCommon<'ser, 'sig, B, W>);

assert_impl_all!(Serializer<'_, '_, i32, i32>: Send, Sync, Unpin);

impl<'ser, 'sig, B, W> Serializer<'ser, 'sig, B, W>
where
    B: byteorder::ByteOrder,
    W: Write + Seek,
{
    /// Create a GVariant Serializer struct instance.
    ///
    /// On Windows, the method doesn't have `fds` argument.
    pub fn new<'w: 'ser, 'f: 'ser>(
        signature: &Signature<'sig>,
        writer: &'w mut W,
        #[cfg(unix)] fds: &'f mut Vec<RawFd>,
        ctxt: EncodingContext<B>,
    ) -> Self {
        assert_eq!(ctxt.format(), EncodingFormat::GVariant);

        let sig_parser = SignatureParser::new(signature.clone());
        Self(crate::SerializerCommon {
            ctxt,
            sig_parser,
            writer,
            #[cfg(unix)]
            fds,
            bytes_written: 0,
            value_sign: None,
            b: PhantomData,
        })
    }

    fn serialize_maybe<T>(&mut self, value: Option<&T>) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let signature = self.0.sig_parser.next_signature()?;
        let alignment = alignment_for_signature(&signature, self.0.ctxt.format());
        let child_sig_parser = self.0.sig_parser.slice(1..);
        let child_signature = child_sig_parser.next_signature()?;
        let child_sig_len = child_signature.len();
        let fixed_sized_child = crate::utils::is_fixed_sized_signature(&child_signature)?;

        self.0.sig_parser.skip_char()?;

        self.0.add_padding(alignment)?;

        match value {
            Some(value) => {
                value.serialize(&mut *self)?;

                if !fixed_sized_child {
                    self.0.write_all(&b"\0"[..]).map_err(Error::Io)?;
                }
            }
            None => {
                self.0.sig_parser.skip_chars(child_sig_len)?;
            }
        }

        Ok(())
    }
}

macro_rules! serialize_basic {
    ($method:ident, $type:ty) => {
        #[inline]
        fn $method(self, v: $type) -> Result<()> {
            let ctxt = EncodingContext::new_dbus(self.0.ctxt.position());
            let bytes_written = self.0.bytes_written;
            let mut dbus_ser = crate::dbus::Serializer(crate::SerializerCommon::<B, W> {
                ctxt,
                sig_parser: self.0.sig_parser.clone(),
                writer: &mut self.0.writer,
                #[cfg(unix)]
                fds: self.0.fds,
                bytes_written,
                value_sign: None,
                b: PhantomData,
            });

            dbus_ser.$method(v)?;

            self.0.bytes_written = dbus_ser.0.bytes_written;
            self.0.sig_parser = dbus_ser.0.sig_parser;

            Ok(())
        }
    };
}

impl<'ser, 'sig, 'b, B, W> ser::Serializer for &'b mut Serializer<'ser, 'sig, B, W>
where
    B: byteorder::ByteOrder,
    W: Write + Seek,
{
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SeqSerializer<'ser, 'sig, 'b, B, W>;
    type SerializeTuple = StructSeqSerializer<'ser, 'sig, 'b, B, W>;
    type SerializeTupleStruct = StructSeqSerializer<'ser, 'sig, 'b, B, W>;
    type SerializeTupleVariant = StructSeqSerializer<'ser, 'sig, 'b, B, W>;
    type SerializeMap = SeqSerializer<'ser, 'sig, 'b, B, W>;
    type SerializeStruct = StructSeqSerializer<'ser, 'sig, 'b, B, W>;
    type SerializeStructVariant = StructSeqSerializer<'ser, 'sig, 'b, B, W>;

    serialize_basic!(serialize_bool, bool);
    serialize_basic!(serialize_i16, i16);
    serialize_basic!(serialize_i32, i32);
    serialize_basic!(serialize_i64, i64);

    serialize_basic!(serialize_u8, u8);
    serialize_basic!(serialize_u16, u16);
    serialize_basic!(serialize_u32, u32);
    serialize_basic!(serialize_u64, u64);

    serialize_basic!(serialize_f64, f64);

    fn serialize_i8(self, v: i8) -> Result<()> {
        // No i8 type in GVariant, let's pretend it's i16
        self.serialize_i16(v as i16)
    }

    fn serialize_f32(self, v: f32) -> Result<()> {
        // No f32 type in GVariant, let's pretend it's f64
        self.serialize_f64(v as f64)
    }

    fn serialize_char(self, v: char) -> Result<()> {
        // No char type in GVariant, let's pretend it's a string
        self.serialize_str(&v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        if v.contains('\0') {
            return Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Char('\0'),
                &"GVariant string type must not contain interior null bytes",
            ));
        }

        let c = self.0.sig_parser.next_char();
        if c == VARIANT_SIGNATURE_CHAR {
            self.0.value_sign = Some(signature_string!(v));

            // signature is serialized after the value in GVariant
            return Ok(());
        }

        // Strings in GVariant format require no alignment.

        self.0.sig_parser.skip_char()?;
        self.0.write_all(v.as_bytes()).map_err(Error::Io)?;
        self.0.write_all(&b"\0"[..]).map_err(Error::Io)?;

        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        let seq = self.serialize_seq(Some(v.len()))?;
        seq.ser.0.write(v).map_err(Error::Io)?;
        seq.end()
    }

    fn serialize_none(self) -> Result<()> {
        self.serialize_maybe::<()>(None)
    }

    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.serialize_maybe(Some(value))
    }

    fn serialize_unit(self) -> Result<()> {
        self.0.write_all(&b"\0"[..]).map_err(Error::Io)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        if self.0.sig_parser.next_char() == <&str>::SIGNATURE_CHAR {
            variant.serialize(self)
        } else {
            variant_index.serialize(self)
        }
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)?;

        Ok(())
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.0.prep_serialize_enum_variant(variant_index)?;

        value.serialize(self)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        self.0.sig_parser.skip_char()?;
        let element_signature = self.0.sig_parser.next_signature()?;
        let element_signature_len = element_signature.len();
        let element_alignment = alignment_for_signature(&element_signature, self.0.ctxt.format());

        let fixed_sized_child = crate::utils::is_fixed_sized_signature(&element_signature)?;
        let offsets = if !fixed_sized_child {
            Some(FramingOffsets::new())
        } else {
            None
        };

        let key_start = if self.0.sig_parser.next_char() == DICT_ENTRY_SIG_START_CHAR {
            let key_signature = Signature::from_str_unchecked(&element_signature[1..2]);
            if !crate::utils::is_fixed_sized_signature(&key_signature)? {
                Some(0)
            } else {
                None
            }
        } else {
            None
        };
        self.0.add_padding(element_alignment)?;

        let start = self.0.bytes_written;

        Ok(SeqSerializer {
            ser: self,
            start,
            element_alignment,
            element_signature_len,
            offsets,
            key_start,
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_struct("", len)
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_struct(name, len)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        self.0.prep_serialize_enum_variant(variant_index)?;

        StructSerializer::enum_variant(self).map(StructSeqSerializer::Struct)
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        self.serialize_seq(len)
    }

    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        match self.0.sig_parser.next_char() {
            VARIANT_SIGNATURE_CHAR => {
                StructSerializer::variant(self).map(StructSeqSerializer::Struct)
            }
            ARRAY_SIGNATURE_CHAR => self.serialize_seq(Some(len)).map(StructSeqSerializer::Seq),
            _ => StructSerializer::structure(self).map(StructSeqSerializer::Struct),
        }
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        self.0.prep_serialize_enum_variant(variant_index)?;

        StructSerializer::enum_variant(self).map(StructSeqSerializer::Struct)
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

#[doc(hidden)]
pub struct SeqSerializer<'ser, 'sig, 'b, B, W> {
    ser: &'b mut Serializer<'ser, 'sig, B, W>,
    start: usize,
    // alignment of element
    element_alignment: usize,
    // size of element signature
    element_signature_len: usize,
    // All offsets
    offsets: Option<FramingOffsets>,
    // start of last dict-entry key written
    key_start: Option<usize>,
}

impl<'ser, 'sig, 'b, B, W> SeqSerializer<'ser, 'sig, 'b, B, W>
where
    B: byteorder::ByteOrder,
    W: Write + Seek,
{
    pub(self) fn end_seq(self) -> Result<()> {
        self.ser
            .0
            .sig_parser
            .skip_chars(self.element_signature_len)?;

        let offsets = match self.offsets {
            Some(offsets) => offsets,
            None => return Ok(()),
        };
        let array_len = self.ser.0.bytes_written - self.start;
        if array_len == 0 {
            // Empty sequence
            assert!(offsets.is_empty());

            return Ok(());
        }

        offsets.write_all(&mut self.ser.0, array_len)?;

        Ok(())
    }
}

impl<'ser, 'sig, 'b, B, W> ser::SerializeSeq for SeqSerializer<'ser, 'sig, 'b, B, W>
where
    B: byteorder::ByteOrder,
    W: Write + Seek,
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        // We want to keep parsing the same signature repeatedly for each element so we use a
        // disposable clone.
        let sig_parser = self.ser.0.sig_parser.clone();
        self.ser.0.sig_parser = sig_parser.clone();

        value.serialize(&mut *self.ser)?;
        self.ser.0.sig_parser = sig_parser;

        if let Some(ref mut offsets) = self.offsets {
            let offset = self.ser.0.bytes_written - self.start;

            offsets.push(offset);
        }

        Ok(())
    }

    fn end(self) -> Result<()> {
        self.end_seq()
    }
}

#[doc(hidden)]
pub struct StructSerializer<'ser, 'sig, 'b, B, W> {
    ser: &'b mut Serializer<'ser, 'sig, B, W>,
    start: usize,
    // The number of `)` in the signature to skip at the end.
    end_parens: u8,
    // All offsets
    offsets: Option<FramingOffsets>,
}

impl<'ser, 'sig, 'b, B, W> StructSerializer<'ser, 'sig, 'b, B, W>
where
    B: byteorder::ByteOrder,
    W: Write + Seek,
{
    fn variant(ser: &'b mut Serializer<'ser, 'sig, B, W>) -> Result<Self> {
        ser.0.add_padding(VARIANT_ALIGNMENT_GVARIANT)?;
        let offsets = if ser.0.sig_parser.next_char() == STRUCT_SIG_START_CHAR {
            Some(FramingOffsets::new())
        } else {
            None
        };
        let start = ser.0.bytes_written;

        Ok(Self {
            ser,
            end_parens: 0,
            offsets,
            start,
        })
    }

    fn structure(ser: &'b mut Serializer<'ser, 'sig, B, W>) -> Result<Self> {
        let c = ser.0.sig_parser.next_char();
        if c != STRUCT_SIG_START_CHAR && c != DICT_ENTRY_SIG_START_CHAR {
            let expected = format!(
                "`{}` or `{}`",
                STRUCT_SIG_START_STR, DICT_ENTRY_SIG_START_STR,
            );

            return Err(serde::de::Error::invalid_type(
                serde::de::Unexpected::Char(c),
                &expected.as_str(),
            ));
        }

        let signature = ser.0.sig_parser.next_signature()?;
        let alignment = alignment_for_signature(&signature, EncodingFormat::GVariant);
        ser.0.add_padding(alignment)?;

        ser.0.sig_parser.skip_char()?;

        let offsets = if c == STRUCT_SIG_START_CHAR {
            Some(FramingOffsets::new())
        } else {
            None
        };
        let start = ser.0.bytes_written;

        Ok(Self {
            ser,
            end_parens: 1,
            offsets,
            start,
        })
    }

    fn enum_variant(ser: &'b mut Serializer<'ser, 'sig, B, W>) -> Result<Self> {
        let mut ser = Self::structure(ser)?;
        ser.end_parens += 1;

        Ok(ser)
    }

    fn serialize_struct_element<T>(&mut self, name: Option<&'static str>, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        match name {
            Some("zvariant::Value::Value") => {
                // Serializing the value of a Value, which means signature was serialized
                // already, and also put aside for us to be picked here.
                let signature = self
                    .ser
                    .0
                    .value_sign
                    .take()
                    .expect("Incorrect Value encoding");

                let sig_parser = SignatureParser::new(signature.clone());
                let bytes_written = self.ser.0.bytes_written;
                let mut ser = Serializer(crate::SerializerCommon::<B, W> {
                    ctxt: self.ser.0.ctxt,
                    sig_parser,
                    writer: self.ser.0.writer,
                    #[cfg(unix)]
                    fds: self.ser.0.fds,
                    bytes_written,
                    value_sign: None,
                    b: PhantomData,
                });
                value.serialize(&mut ser)?;
                self.ser.0.bytes_written = ser.0.bytes_written;

                self.ser.0.write_all(&b"\0"[..]).map_err(Error::Io)?;
                self.ser
                    .0
                    .write_all(signature.as_bytes())
                    .map_err(Error::Io)?;

                Ok(())
            }
            _ => {
                let element_signature = self.ser.0.sig_parser.next_signature()?;
                let fixed_sized_element =
                    crate::utils::is_fixed_sized_signature(&element_signature)?;

                value.serialize(&mut *self.ser)?;

                if let Some(ref mut offsets) = self.offsets {
                    if !fixed_sized_element {
                        offsets.push_front(self.ser.0.bytes_written - self.start);
                    }
                }

                Ok(())
            }
        }
    }

    fn end_struct(self) -> Result<()> {
        if self.end_parens > 0 {
            self.ser.0.sig_parser.skip_chars(self.end_parens as usize)?;
        }
        let mut offsets = match self.offsets {
            Some(offsets) => offsets,
            None => return Ok(()),
        };
        let struct_len = self.ser.0.bytes_written - self.start;
        if struct_len == 0 {
            // Empty sequence
            assert!(offsets.is_empty());

            return Ok(());
        }
        if offsets.peek() == Some(struct_len) {
            // For structs, we don't want offset of last element
            offsets.pop();
        }

        offsets.write_all(&mut self.ser.0, struct_len)?;

        Ok(())
    }
}

#[doc(hidden)]
/// Allows us to serialize a struct as an ARRAY.
pub enum StructSeqSerializer<'ser, 'sig, 'b, B, W> {
    Struct(StructSerializer<'ser, 'sig, 'b, B, W>),
    Seq(SeqSerializer<'ser, 'sig, 'b, B, W>),
}

macro_rules! serialize_struct_anon_fields {
    ($trait:ident $method:ident) => {
        impl<'ser, 'sig, 'b, B, W> ser::$trait for StructSerializer<'ser, 'sig, 'b, B, W>
        where
            B: byteorder::ByteOrder,
            W: Write + Seek,
        {
            type Ok = ();
            type Error = Error;

            fn $method<T>(&mut self, value: &T) -> Result<()>
            where
                T: ?Sized + Serialize,
            {
                self.serialize_struct_element(None, value)
            }

            fn end(self) -> Result<()> {
                self.end_struct()
            }
        }

        impl<'ser, 'sig, 'b, B, W> ser::$trait for StructSeqSerializer<'ser, 'sig, 'b, B, W>
        where
            B: byteorder::ByteOrder,
            W: Write + Seek,
        {
            type Ok = ();
            type Error = Error;

            fn $method<T>(&mut self, value: &T) -> Result<()>
            where
                T: ?Sized + Serialize,
            {
                match self {
                    StructSeqSerializer::Struct(ser) => ser.$method(value),
                    StructSeqSerializer::Seq(ser) => ser.serialize_element(value),
                }
            }

            fn end(self) -> Result<()> {
                match self {
                    StructSeqSerializer::Struct(ser) => ser.end_struct(),
                    StructSeqSerializer::Seq(ser) => ser.end_seq(),
                }
            }
        }
    };
}
serialize_struct_anon_fields!(SerializeTuple serialize_element);
serialize_struct_anon_fields!(SerializeTupleStruct serialize_field);
serialize_struct_anon_fields!(SerializeTupleVariant serialize_field);

impl<'ser, 'sig, 'b, B, W> ser::SerializeMap for SeqSerializer<'ser, 'sig, 'b, B, W>
where
    B: byteorder::ByteOrder,
    W: Write + Seek,
{
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.ser.0.add_padding(self.element_alignment)?;

        if self.key_start.is_some() {
            self.key_start.replace(self.ser.0.bytes_written);
        }

        // We want to keep parsing the same signature repeatedly for each key so we use a
        // disposable clone.
        let sig_parser = self.ser.0.sig_parser.clone();
        self.ser.0.sig_parser = sig_parser.clone();

        // skip `{`
        self.ser.0.sig_parser.skip_char()?;

        key.serialize(&mut *self.ser)?;
        self.ser.0.sig_parser = sig_parser;

        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        // For non-fixed-sized keys, we must add the key offset after the value
        let key_offset = self.key_start.map(|start| self.ser.0.bytes_written - start);

        // We want to keep parsing the same signature repeatedly for each key so we use a
        // disposable clone.
        let sig_parser = self.ser.0.sig_parser.clone();
        self.ser.0.sig_parser = sig_parser.clone();

        // skip `{` and key char
        self.ser.0.sig_parser.skip_chars(2)?;

        value.serialize(&mut *self.ser)?;
        // Restore the original parser
        self.ser.0.sig_parser = sig_parser;

        if let Some(key_offset) = key_offset {
            let entry_size = self.ser.0.bytes_written - self.key_start.unwrap_or(0);
            let offset_size = FramingOffsetSize::for_encoded_container(entry_size);
            offset_size.write_offset(&mut self.ser.0, key_offset)?;
        }

        // And now the offset of the array element end (which is encoded later)
        if let Some(ref mut offsets) = self.offsets {
            let offset = self.ser.0.bytes_written - self.start;

            offsets.push(offset);
        }

        Ok(())
    }

    fn end(self) -> Result<()> {
        self.end_seq()
    }
}

macro_rules! serialize_struct_named_fields {
    ($trait:ident) => {
        impl<'ser, 'sig, 'b, B, W> ser::$trait for StructSerializer<'ser, 'sig, 'b, B, W>
        where
            B: byteorder::ByteOrder,
            W: Write + Seek,
        {
            type Ok = ();
            type Error = Error;

            fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
            where
                T: ?Sized + Serialize,
            {
                self.serialize_struct_element(Some(key), value)
            }

            fn end(self) -> Result<()> {
                self.end_struct()
            }
        }

        impl<'ser, 'sig, 'b, B, W> ser::$trait for StructSeqSerializer<'ser, 'sig, 'b, B, W>
        where
            B: byteorder::ByteOrder,
            W: Write + Seek,
        {
            type Ok = ();
            type Error = Error;

            fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
            where
                T: ?Sized + Serialize,
            {
                match self {
                    StructSeqSerializer::Struct(ser) => ser.serialize_field(key, value),
                    StructSeqSerializer::Seq(ser) => ser.serialize_element(value),
                }
            }

            fn end(self) -> Result<()> {
                match self {
                    StructSeqSerializer::Struct(ser) => ser.end_struct(),
                    StructSeqSerializer::Seq(ser) => ser.end_seq(),
                }
            }
        }
    };
}
serialize_struct_named_fields!(SerializeStruct);
serialize_struct_named_fields!(SerializeStructVariant);
