//! YAML Serialization
//!
//! This module provides YAML serialization with the type `Serializer`.

use crate::{error, Error, Result};
use serde::ser;
use std::{fmt, io, num, str};
use yaml_rust::{yaml, Yaml, YamlEmitter};

/// A structure for serializing Rust values into YAML.
///
/// # Example
///
/// ```
/// use anyhow::Result;
/// use serde::Serialize;
/// use std::collections::BTreeMap;
///
/// fn main() -> Result<()> {
///     let mut buffer = Vec::new();
///     let mut ser = serde_yaml::Serializer::new(&mut buffer);
///
///     let mut object = BTreeMap::new();
///     object.insert("k", 107);
///     object.serialize(&mut ser)?;
///
///     object.insert("J", 74);
///     object.serialize(&mut ser)?;
///
///     assert_eq!(buffer, b"---\nk: 107\n...\n---\nJ: 74\nk: 107\n");
///     Ok(())
/// }
/// ```
pub struct Serializer<W> {
    documents: usize,
    writer: W,
}

impl<W> Serializer<W>
where
    W: io::Write,
{
    /// Creates a new YAML serializer.
    pub fn new(writer: W) -> Self {
        Serializer {
            documents: 0,
            writer,
        }
    }

    /// Calls [`.flush()`](io::Write::flush) on the underlying `io::Write`
    /// object.
    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    /// Unwrap the underlying `io::Write` object from the `Serializer`.
    pub fn into_inner(self) -> W {
        self.writer
    }

    fn write(&mut self, doc: Yaml) -> Result<()> {
        if self.documents > 0 {
            self.writer.write_all(b"...\n").map_err(error::io)?;
        }
        self.documents += 1;
        let mut writer_adapter = FmtToIoWriter {
            writer: &mut self.writer,
        };
        YamlEmitter::new(&mut writer_adapter)
            .dump(&doc)
            .map_err(error::emitter)?;
        writer_adapter.writer.write_all(b"\n").map_err(error::io)?;
        Ok(())
    }
}

impl<'a, W> ser::Serializer for &'a mut Serializer<W>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    type SerializeSeq = ThenWrite<'a, W, SerializeArray>;
    type SerializeTuple = ThenWrite<'a, W, SerializeArray>;
    type SerializeTupleStruct = ThenWrite<'a, W, SerializeArray>;
    type SerializeTupleVariant = ThenWrite<'a, W, SerializeTupleVariant>;
    type SerializeMap = ThenWrite<'a, W, SerializeMap>;
    type SerializeStruct = ThenWrite<'a, W, SerializeStruct>;
    type SerializeStructVariant = ThenWrite<'a, W, SerializeStructVariant>;

    fn serialize_bool(self, v: bool) -> Result<()> {
        let doc = SerializerToYaml.serialize_bool(v)?;
        self.write(doc)
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        let doc = SerializerToYaml.serialize_i8(v)?;
        self.write(doc)
    }

    fn serialize_i16(self, v: i16) -> Result<()> {
        let doc = SerializerToYaml.serialize_i16(v)?;
        self.write(doc)
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        let doc = SerializerToYaml.serialize_i32(v)?;
        self.write(doc)
    }

    fn serialize_i64(self, v: i64) -> Result<()> {
        let doc = SerializerToYaml.serialize_i64(v)?;
        self.write(doc)
    }

    fn serialize_i128(self, v: i128) -> Result<()> {
        let doc = SerializerToYaml.serialize_i128(v)?;
        self.write(doc)
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        let doc = SerializerToYaml.serialize_u8(v)?;
        self.write(doc)
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        let doc = SerializerToYaml.serialize_u16(v)?;
        self.write(doc)
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        let doc = SerializerToYaml.serialize_u32(v)?;
        self.write(doc)
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        let doc = SerializerToYaml.serialize_u64(v)?;
        self.write(doc)
    }

    fn serialize_u128(self, v: u128) -> Result<()> {
        let doc = SerializerToYaml.serialize_u128(v)?;
        self.write(doc)
    }

    fn serialize_f32(self, v: f32) -> Result<()> {
        let doc = SerializerToYaml.serialize_f32(v)?;
        self.write(doc)
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        let doc = SerializerToYaml.serialize_f64(v)?;
        self.write(doc)
    }

    fn serialize_char(self, value: char) -> Result<()> {
        let doc = SerializerToYaml.serialize_char(value)?;
        self.write(doc)
    }

    fn serialize_str(self, value: &str) -> Result<()> {
        let doc = SerializerToYaml.serialize_str(value)?;
        self.write(doc)
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<()> {
        let doc = SerializerToYaml.serialize_bytes(value)?;
        self.write(doc)
    }

    fn serialize_unit(self) -> Result<()> {
        let doc = SerializerToYaml.serialize_unit()?;
        self.write(doc)
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<()> {
        let doc = SerializerToYaml.serialize_unit_struct(name)?;
        self.write(doc)
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        let doc = SerializerToYaml.serialize_unit_variant(name, variant_index, variant)?;
        self.write(doc)
    }

    fn serialize_newtype_struct<T>(self, name: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        let doc = SerializerToYaml.serialize_newtype_struct(name, value)?;
        self.write(doc)
    }

    fn serialize_newtype_variant<T>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        let doc =
            SerializerToYaml.serialize_newtype_variant(name, variant_index, variant, value)?;
        self.write(doc)
    }

    fn serialize_none(self) -> Result<()> {
        let doc = SerializerToYaml.serialize_none()?;
        self.write(doc)
    }

    fn serialize_some<V>(self, value: &V) -> Result<()>
    where
        V: ?Sized + ser::Serialize,
    {
        let doc = SerializerToYaml.serialize_some(value)?;
        self.write(doc)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        let delegate = SerializerToYaml.serialize_seq(len)?;
        Ok(ThenWrite::new(self, delegate))
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        let delegate = SerializerToYaml.serialize_tuple(len)?;
        Ok(ThenWrite::new(self, delegate))
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        let delegate = SerializerToYaml.serialize_tuple_struct(name, len)?;
        Ok(ThenWrite::new(self, delegate))
    }

    fn serialize_tuple_variant(
        self,
        enm: &'static str,
        idx: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        let delegate = SerializerToYaml.serialize_tuple_variant(enm, idx, variant, len)?;
        Ok(ThenWrite::new(self, delegate))
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        let delegate = SerializerToYaml.serialize_map(len)?;
        Ok(ThenWrite::new(self, delegate))
    }

    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        let delegate = SerializerToYaml.serialize_struct(name, len)?;
        Ok(ThenWrite::new(self, delegate))
    }

    fn serialize_struct_variant(
        self,
        enm: &'static str,
        idx: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        let delegate = SerializerToYaml.serialize_struct_variant(enm, idx, variant, len)?;
        Ok(ThenWrite::new(self, delegate))
    }
}

pub struct ThenWrite<'a, W, D> {
    serializer: &'a mut Serializer<W>,
    delegate: D,
}

impl<'a, W, D> ThenWrite<'a, W, D> {
    fn new(serializer: &'a mut Serializer<W>, delegate: D) -> Self {
        ThenWrite {
            serializer,
            delegate,
        }
    }
}

impl<'a, W> ser::SerializeSeq for ThenWrite<'a, W, SerializeArray>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, elem: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        self.delegate.serialize_element(elem)
    }

    fn end(self) -> Result<()> {
        let doc = self.delegate.end()?;
        self.serializer.write(doc)
    }
}

impl<'a, W> ser::SerializeTuple for ThenWrite<'a, W, SerializeArray>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, elem: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        self.delegate.serialize_element(elem)
    }

    fn end(self) -> Result<()> {
        let doc = self.delegate.end()?;
        self.serializer.write(doc)
    }
}

impl<'a, W> ser::SerializeTupleStruct for ThenWrite<'a, W, SerializeArray>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<V>(&mut self, value: &V) -> Result<()>
    where
        V: ?Sized + ser::Serialize,
    {
        self.delegate.serialize_field(value)
    }

    fn end(self) -> Result<()> {
        let doc = self.delegate.end()?;
        self.serializer.write(doc)
    }
}

impl<'a, W> ser::SerializeTupleVariant for ThenWrite<'a, W, SerializeTupleVariant>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<V>(&mut self, v: &V) -> Result<()>
    where
        V: ?Sized + ser::Serialize,
    {
        self.delegate.serialize_field(v)
    }

    fn end(self) -> Result<()> {
        let doc = self.delegate.end()?;
        self.serializer.write(doc)
    }
}

impl<'a, W> ser::SerializeMap for ThenWrite<'a, W, SerializeMap>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        self.delegate.serialize_key(key)
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        self.delegate.serialize_value(value)
    }

    fn serialize_entry<K, V>(&mut self, key: &K, value: &V) -> Result<()>
    where
        K: ?Sized + ser::Serialize,
        V: ?Sized + ser::Serialize,
    {
        self.delegate.serialize_entry(key, value)
    }

    fn end(self) -> Result<()> {
        let doc = self.delegate.end()?;
        self.serializer.write(doc)
    }
}

impl<'a, W> ser::SerializeStruct for ThenWrite<'a, W, SerializeStruct>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<V>(&mut self, key: &'static str, value: &V) -> Result<()>
    where
        V: ?Sized + ser::Serialize,
    {
        self.delegate.serialize_field(key, value)
    }

    fn end(self) -> Result<()> {
        let doc = self.delegate.end()?;
        self.serializer.write(doc)
    }
}

impl<'a, W> ser::SerializeStructVariant for ThenWrite<'a, W, SerializeStructVariant>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<V>(&mut self, field: &'static str, v: &V) -> Result<()>
    where
        V: ?Sized + ser::Serialize,
    {
        self.delegate.serialize_field(field, v)
    }

    fn end(self) -> Result<()> {
        let doc = self.delegate.end()?;
        self.serializer.write(doc)
    }
}

pub struct SerializerToYaml;

impl ser::Serializer for SerializerToYaml {
    type Ok = Yaml;
    type Error = Error;

    type SerializeSeq = SerializeArray;
    type SerializeTuple = SerializeArray;
    type SerializeTupleStruct = SerializeArray;
    type SerializeTupleVariant = SerializeTupleVariant;
    type SerializeMap = SerializeMap;
    type SerializeStruct = SerializeStruct;
    type SerializeStructVariant = SerializeStructVariant;

    fn serialize_bool(self, v: bool) -> Result<Yaml> {
        Ok(Yaml::Boolean(v))
    }

    fn serialize_i8(self, v: i8) -> Result<Yaml> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i16(self, v: i16) -> Result<Yaml> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i32(self, v: i32) -> Result<Yaml> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i64(self, v: i64) -> Result<Yaml> {
        Ok(Yaml::Integer(v))
    }

    #[allow(clippy::cast_possible_truncation)]
    fn serialize_i128(self, v: i128) -> Result<Yaml> {
        if v <= i64::max_value() as i128 && v >= i64::min_value() as i128 {
            self.serialize_i64(v as i64)
        } else {
            Ok(Yaml::Real(v.to_string()))
        }
    }

    fn serialize_u8(self, v: u8) -> Result<Yaml> {
        self.serialize_i64(v as i64)
    }

    fn serialize_u16(self, v: u16) -> Result<Yaml> {
        self.serialize_i64(v as i64)
    }

    fn serialize_u32(self, v: u32) -> Result<Yaml> {
        self.serialize_i64(v as i64)
    }

    fn serialize_u64(self, v: u64) -> Result<Yaml> {
        if v <= i64::max_value() as u64 {
            self.serialize_i64(v as i64)
        } else {
            Ok(Yaml::Real(v.to_string()))
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn serialize_u128(self, v: u128) -> Result<Yaml> {
        if v <= i64::max_value() as u128 {
            self.serialize_i64(v as i64)
        } else {
            Ok(Yaml::Real(v.to_string()))
        }
    }

    fn serialize_f32(self, v: f32) -> Result<Yaml> {
        Ok(Yaml::Real(match v.classify() {
            num::FpCategory::Infinite if v.is_sign_positive() => ".inf".into(),
            num::FpCategory::Infinite => "-.inf".into(),
            num::FpCategory::Nan => ".nan".into(),
            _ => ryu::Buffer::new().format_finite(v).into(),
        }))
    }

    fn serialize_f64(self, v: f64) -> Result<Yaml> {
        Ok(Yaml::Real(match v.classify() {
            num::FpCategory::Infinite if v.is_sign_positive() => ".inf".into(),
            num::FpCategory::Infinite => "-.inf".into(),
            num::FpCategory::Nan => ".nan".into(),
            _ => ryu::Buffer::new().format_finite(v).into(),
        }))
    }

    fn serialize_char(self, value: char) -> Result<Yaml> {
        Ok(Yaml::String(value.to_string()))
    }

    fn serialize_str(self, value: &str) -> Result<Yaml> {
        Ok(Yaml::String(value.to_owned()))
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Yaml> {
        let vec = value.iter().map(|&b| Yaml::Integer(b as i64)).collect();
        Ok(Yaml::Array(vec))
    }

    fn serialize_unit(self) -> Result<Yaml> {
        Ok(Yaml::Null)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Yaml> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &str,
        _variant_index: u32,
        variant: &str,
    ) -> Result<Yaml> {
        Ok(Yaml::String(variant.to_owned()))
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<Yaml>
    where
        T: ?Sized + ser::Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &str,
        _variant_index: u32,
        variant: &str,
        value: &T,
    ) -> Result<Yaml>
    where
        T: ?Sized + ser::Serialize,
    {
        Ok(singleton_hash(to_yaml(variant)?, to_yaml(value)?))
    }

    fn serialize_none(self) -> Result<Yaml> {
        self.serialize_unit()
    }

    fn serialize_some<V>(self, value: &V) -> Result<Yaml>
    where
        V: ?Sized + ser::Serialize,
    {
        value.serialize(self)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<SerializeArray> {
        let array = match len {
            None => yaml::Array::new(),
            Some(len) => yaml::Array::with_capacity(len),
        };
        Ok(SerializeArray { array })
    }

    fn serialize_tuple(self, len: usize) -> Result<SerializeArray> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(self, _name: &'static str, len: usize) -> Result<SerializeArray> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _enum: &'static str,
        _idx: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<SerializeTupleVariant> {
        Ok(SerializeTupleVariant {
            name: variant,
            array: yaml::Array::with_capacity(len),
        })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<SerializeMap> {
        Ok(SerializeMap {
            hash: yaml::Hash::new(),
            next_key: None,
        })
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<SerializeStruct> {
        Ok(SerializeStruct {
            hash: yaml::Hash::new(),
        })
    }

    fn serialize_struct_variant(
        self,
        _enum: &'static str,
        _idx: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<SerializeStructVariant> {
        Ok(SerializeStructVariant {
            name: variant,
            hash: yaml::Hash::new(),
        })
    }
}

#[doc(hidden)]
pub struct SerializeArray {
    array: yaml::Array,
}

#[doc(hidden)]
pub struct SerializeTupleVariant {
    name: &'static str,
    array: yaml::Array,
}

#[doc(hidden)]
pub struct SerializeMap {
    hash: yaml::Hash,
    next_key: Option<yaml::Yaml>,
}

#[doc(hidden)]
pub struct SerializeStruct {
    hash: yaml::Hash,
}

#[doc(hidden)]
pub struct SerializeStructVariant {
    name: &'static str,
    hash: yaml::Hash,
}

impl ser::SerializeSeq for SerializeArray {
    type Ok = yaml::Yaml;
    type Error = Error;

    fn serialize_element<T>(&mut self, elem: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        self.array.push(to_yaml(elem)?);
        Ok(())
    }

    fn end(self) -> Result<Yaml> {
        Ok(Yaml::Array(self.array))
    }
}

impl ser::SerializeTuple for SerializeArray {
    type Ok = yaml::Yaml;
    type Error = Error;

    fn serialize_element<T>(&mut self, elem: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        ser::SerializeSeq::serialize_element(self, elem)
    }

    fn end(self) -> Result<Yaml> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleStruct for SerializeArray {
    type Ok = yaml::Yaml;
    type Error = Error;

    fn serialize_field<V>(&mut self, value: &V) -> Result<()>
    where
        V: ?Sized + ser::Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Yaml> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleVariant for SerializeTupleVariant {
    type Ok = yaml::Yaml;
    type Error = Error;

    fn serialize_field<V>(&mut self, v: &V) -> Result<()>
    where
        V: ?Sized + ser::Serialize,
    {
        self.array.push(to_yaml(v)?);
        Ok(())
    }

    fn end(self) -> Result<Yaml> {
        Ok(singleton_hash(to_yaml(self.name)?, Yaml::Array(self.array)))
    }
}

impl ser::SerializeMap for SerializeMap {
    type Ok = yaml::Yaml;
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        self.next_key = Some(to_yaml(key)?);
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        match self.next_key.take() {
            Some(key) => self.hash.insert(key, to_yaml(value)?),
            None => panic!("serialize_value called before serialize_key"),
        };
        Ok(())
    }

    fn serialize_entry<K, V>(&mut self, key: &K, value: &V) -> Result<()>
    where
        K: ?Sized + ser::Serialize,
        V: ?Sized + ser::Serialize,
    {
        self.hash.insert(to_yaml(key)?, to_yaml(value)?);
        Ok(())
    }

    fn end(self) -> Result<Yaml> {
        Ok(Yaml::Hash(self.hash))
    }
}

impl ser::SerializeStruct for SerializeStruct {
    type Ok = yaml::Yaml;
    type Error = Error;

    fn serialize_field<V>(&mut self, key: &'static str, value: &V) -> Result<()>
    where
        V: ?Sized + ser::Serialize,
    {
        self.hash.insert(to_yaml(key)?, to_yaml(value)?);
        Ok(())
    }

    fn end(self) -> Result<Yaml> {
        Ok(Yaml::Hash(self.hash))
    }
}

impl ser::SerializeStructVariant for SerializeStructVariant {
    type Ok = yaml::Yaml;
    type Error = Error;

    fn serialize_field<V>(&mut self, field: &'static str, v: &V) -> Result<()>
    where
        V: ?Sized + ser::Serialize,
    {
        self.hash.insert(to_yaml(field)?, to_yaml(v)?);
        Ok(())
    }

    fn end(self) -> Result<Yaml> {
        Ok(singleton_hash(to_yaml(self.name)?, Yaml::Hash(self.hash)))
    }
}

/// Serialize the given data structure as YAML into the IO stream.
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// return an error.
pub fn to_writer<W, T>(writer: W, value: &T) -> Result<()>
where
    W: io::Write,
    T: ?Sized + ser::Serialize,
{
    value.serialize(&mut Serializer::new(writer))
}

/// Serialize the given data structure as a YAML byte vector.
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// return an error.
pub fn to_vec<T>(value: &T) -> Result<Vec<u8>>
where
    T: ?Sized + ser::Serialize,
{
    let mut vec = Vec::with_capacity(128);
    to_writer(&mut vec, value)?;
    Ok(vec)
}

/// Serialize the given data structure as a String of YAML.
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// return an error.
pub fn to_string<T>(value: &T) -> Result<String>
where
    T: ?Sized + ser::Serialize,
{
    String::from_utf8(to_vec(value)?).map_err(error::string_utf8)
}

/// The yaml-rust library uses `fmt::Write` intead of `io::Write` so this is a
/// simple adapter.
struct FmtToIoWriter<W> {
    writer: W,
}

impl<W> fmt::Write for FmtToIoWriter<W>
where
    W: io::Write,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.writer.write_all(s.as_bytes()).is_err() {
            return Err(fmt::Error);
        }
        Ok(())
    }
}

fn to_yaml<T>(elem: T) -> Result<Yaml>
where
    T: ser::Serialize,
{
    elem.serialize(SerializerToYaml)
}

fn singleton_hash(k: Yaml, v: Yaml) -> Yaml {
    let mut hash = yaml::Hash::new();
    hash.insert(k, v);
    Yaml::Hash(hash)
}
