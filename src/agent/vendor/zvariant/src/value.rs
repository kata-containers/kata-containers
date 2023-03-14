use core::str;
use std::{convert::TryFrom, marker::PhantomData};

use serde::{
    de::{
        Deserialize, DeserializeSeed, Deserializer, Error, MapAccess, SeqAccess, Unexpected,
        Visitor,
    },
    ser::{Serialize, SerializeSeq, SerializeStruct, SerializeTupleStruct, Serializer},
};
use static_assertions::assert_impl_all;

#[cfg(feature = "gvariant")]
use crate::Maybe;
use crate::{
    signature_parser::SignatureParser, utils::*, Array, Basic, Dict, DynamicType, ObjectPath,
    OwnedValue, Signature, Str, Structure, StructureBuilder, Type,
};

#[cfg(unix)]
use crate::Fd;

/// A generic container, in the form of an enum that holds exactly one value of any of the other
/// types.
///
/// Note that this type corresponds to the `VARIANT` data type defined by the [D-Bus specification]
/// and as such, its encoding is not the same as that of the enclosed value.
///
/// # Examples
///
/// ```
/// use std::convert::TryFrom;
/// use zvariant::{from_slice, to_bytes, EncodingContext, Value};
///
/// // Create a Value from an i16
/// let v = Value::new(i16::max_value());
///
/// // Encode it
/// let ctxt = EncodingContext::<byteorder::LE>::new_dbus(0);
/// let encoding = to_bytes(ctxt, &v).unwrap();
///
/// // Decode it back
/// let v: Value = from_slice(&encoding, ctxt).unwrap();
///
/// // Check everything is as expected
/// assert_eq!(i16::try_from(&v).unwrap(), i16::max_value());
/// ```
///
/// Now let's try a more complicated example:
///
/// ```
/// use std::convert::TryFrom;
/// use zvariant::{from_slice, to_bytes, EncodingContext};
/// use zvariant::{Structure, Value, Str};
///
/// // Create a Value from a tuple this time
/// let v = Value::new((i16::max_value(), "hello", true));
///
/// // Same drill as previous example
/// let ctxt = EncodingContext::<byteorder::LE>::new_dbus(0);
/// let encoding = to_bytes(ctxt, &v).unwrap();
/// let v: Value = from_slice(&encoding, ctxt).unwrap();
///
/// // Check everything is as expected
/// let s = Structure::try_from(v).unwrap();
/// assert_eq!(
///     <(i16, Str, bool)>::try_from(s).unwrap(),
///     (i16::max_value(), Str::from("hello"), true),
/// );
/// ```
///
/// [D-Bus specification]: https://dbus.freedesktop.org/doc/dbus-specification.html#container-types
#[derive(Debug, Clone, PartialEq)]
pub enum Value<'a> {
    // Simple types
    U8(u8),
    Bool(bool),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F64(f64),
    Str(Str<'a>),
    Signature(Signature<'a>),
    ObjectPath(ObjectPath<'a>),
    Value(Box<Value<'a>>),

    // Container types
    Array(Array<'a>),
    Dict(Dict<'a, 'a>),
    Structure(Structure<'a>),
    #[cfg(feature = "gvariant")]
    Maybe(Maybe<'a>),

    #[cfg(unix)]
    Fd(Fd),
}

assert_impl_all!(Value<'_>: Send, Sync, Unpin);

macro_rules! serialize_value {
    ($self:ident $serializer:ident.$method:ident $($first_arg:expr)*) => {
        match $self {
            Value::U8(value) => $serializer.$method($($first_arg,)* value),
            Value::Bool(value) => $serializer.$method($($first_arg,)* value),
            Value::I16(value) => $serializer.$method($($first_arg,)* value),
            Value::U16(value) => $serializer.$method($($first_arg,)* value),
            Value::I32(value) => $serializer.$method($($first_arg,)* value),
            Value::U32(value) => $serializer.$method($($first_arg,)* value),
            Value::I64(value) => $serializer.$method($($first_arg,)* value),
            Value::U64(value) => $serializer.$method($($first_arg,)* value),
            Value::F64(value) => $serializer.$method($($first_arg,)* value),
            Value::Str(value) => $serializer.$method($($first_arg,)* value),
            Value::Signature(value) => $serializer.$method($($first_arg,)* value),
            Value::ObjectPath(value) => $serializer.$method($($first_arg,)* value),
            Value::Value(value) => $serializer.$method($($first_arg,)* value),

            // Container types
            Value::Array(value) => $serializer.$method($($first_arg,)* value),
            Value::Dict(value) => $serializer.$method($($first_arg,)* value),
            Value::Structure(value) => $serializer.$method($($first_arg,)* value),
            #[cfg(feature = "gvariant")]
            Value::Maybe(value) => $serializer.$method($($first_arg,)* value),

            #[cfg(unix)]
            Value::Fd(value) => $serializer.$method($($first_arg,)* value),
        }
    }
}

impl<'a> Value<'a> {
    /// Make a [`Value`] for a given value.
    ///
    /// In general, you can use [`Into`] trait on basic types, except
    /// when you explicitly need to wrap [`Value`] itself, in which
    /// case this constructor comes handy.
    ///
    /// # Examples
    ///
    /// ```
    /// use zvariant::Value;
    ///
    /// let s = Value::new("hello");
    /// let u: Value = 51.into();
    /// assert_ne!(s, u);
    /// ```
    ///
    /// [`Value`]: enum.Value.html
    /// [`Into`]: https://doc.rust-lang.org/std/convert/trait.Into.html
    pub fn new<T>(value: T) -> Self
    where
        T: Into<Self> + DynamicType,
    {
        // With specialization, we wouldn't have this
        if value.dynamic_signature() == VARIANT_SIGNATURE_STR {
            Self::Value(Box::new(value.into()))
        } else {
            value.into()
        }
    }

    /// Create an owned version of `self`.
    ///
    /// Ideally, we should implement [`std::borrow::ToOwned`] trait for `Value`, but that's
    /// implemented generically for us through `impl<T: Clone> ToOwned for T` and it's not what we
    /// need/want.
    pub fn to_owned(&self) -> OwnedValue {
        OwnedValue(match self {
            Value::U8(v) => Value::U8(*v),
            Value::Bool(v) => Value::Bool(*v),
            Value::I16(v) => Value::I16(*v),
            Value::U16(v) => Value::U16(*v),
            Value::I32(v) => Value::I32(*v),
            Value::U32(v) => Value::U32(*v),
            Value::I64(v) => Value::I64(*v),
            Value::U64(v) => Value::U64(*v),
            Value::F64(v) => Value::F64(*v),
            Value::Str(v) => Value::Str(v.to_owned()),
            Value::Signature(v) => Value::Signature(v.to_owned()),
            Value::ObjectPath(v) => Value::ObjectPath(v.to_owned()),
            Value::Value(v) => {
                let o = OwnedValue::from(&**v);
                Value::Value(Box::new(o.into_inner()))
            }

            Value::Array(v) => Value::Array(v.to_owned()),
            Value::Dict(v) => Value::Dict(v.to_owned()),
            Value::Structure(v) => Value::Structure(v.to_owned()),
            #[cfg(feature = "gvariant")]
            Value::Maybe(v) => Value::Maybe(v.to_owned()),
            #[cfg(unix)]
            Value::Fd(v) => Value::Fd(*v),
        })
    }

    /// Get the signature of the enclosed value.
    pub fn value_signature(&self) -> Signature<'_> {
        match self {
            Value::U8(_) => u8::signature(),
            Value::Bool(_) => bool::signature(),
            Value::I16(_) => i16::signature(),
            Value::U16(_) => u16::signature(),
            Value::I32(_) => i32::signature(),
            Value::U32(_) => u32::signature(),
            Value::I64(_) => i64::signature(),
            Value::U64(_) => u64::signature(),
            Value::F64(_) => f64::signature(),
            Value::Str(_) => <&str>::signature(),
            Value::Signature(_) => Signature::signature(),
            Value::ObjectPath(_) => ObjectPath::signature(),
            Value::Value(_) => Signature::from_static_str_unchecked("v"),

            // Container types
            Value::Array(value) => value.full_signature().clone(),
            Value::Dict(value) => value.full_signature().clone(),
            Value::Structure(value) => value.full_signature().clone(),
            #[cfg(feature = "gvariant")]
            Value::Maybe(value) => value.full_signature().clone(),

            #[cfg(unix)]
            Value::Fd(_) => Fd::signature(),
        }
    }

    pub(crate) fn serialize_value_as_struct_field<S>(
        &self,
        name: &'static str,
        serializer: &mut S,
    ) -> Result<(), S::Error>
    where
        S: SerializeStruct,
    {
        serialize_value!(self serializer.serialize_field name)
    }

    pub(crate) fn serialize_value_as_tuple_struct_field<S>(
        &self,
        serializer: &mut S,
    ) -> Result<(), S::Error>
    where
        S: SerializeTupleStruct,
    {
        serialize_value!(self serializer.serialize_field)
    }

    // Really crappy that we need to do this separately for struct and seq cases. :(
    pub(crate) fn serialize_value_as_seq_element<S>(
        &self,
        serializer: &mut S,
    ) -> Result<(), S::Error>
    where
        S: SerializeSeq,
    {
        serialize_value!(self serializer.serialize_element)
    }

    #[cfg(feature = "gvariant")]
    pub(crate) fn serialize_value_as_some<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_value!(self serializer.serialize_some)
    }

    /// Try to get the underlying type `T`.
    ///
    /// Note that [`TryFrom<Value>`] is implemented for various types, and it's usually best to use
    /// that instead. However, in generic code where you also want to unwrap [`Value::Value`],
    /// you should use this function (because [`TryFrom<Value>`] can not be implemented for `Value`
    /// itself as [`From<Value>`] is implicitly implemented for `Value`).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::convert::TryFrom;
    /// use zvariant::{Result, Value};
    ///
    /// fn value_vec_to_type_vec<'a, T>(values: Vec<Value<'a>>) -> Result<Vec<T>>
    /// where
    ///     T: TryFrom<Value<'a>>,
    /// {
    ///     let mut res = vec![];
    ///     for value in values.into_iter() {
    ///         res.push(value.downcast().unwrap());
    ///     }
    ///
    ///     Ok(res)
    /// }
    ///
    /// // Let's try u32 values first
    /// let v = vec![Value::U32(42), Value::U32(43)];
    /// let v = value_vec_to_type_vec::<u32>(v).unwrap();
    /// assert_eq!(v[0], 42);
    /// assert_eq!(v[1], 43);
    ///
    /// // Now try Value values
    /// let v = vec![Value::new(Value::U32(42)), Value::new(Value::U32(43))];
    /// let v = value_vec_to_type_vec::<Value>(v).unwrap();
    /// assert_eq!(v[0], Value::U32(42));
    /// assert_eq!(v[1], Value::U32(43));
    /// ```
    ///
    /// [`Value::Value`]: enum.Value.html#variant.Value
    /// [`TryFrom<Value>`]: https://doc.rust-lang.org/std/convert/trait.TryFrom.html
    /// [`From<Value>`]: https://doc.rust-lang.org/std/convert/trait.From.html
    pub fn downcast<T: ?Sized>(self) -> Option<T>
    where
        T: TryFrom<Value<'a>>,
    {
        if let Value::Value(v) = self {
            T::try_from(*v).ok()
        } else {
            T::try_from(self).ok()
        }
    }

    /// Try to get a reference to the underlying type `T`.
    ///
    /// Same as [`downcast`] except it doesn't consume `self` and get a reference to the underlying
    /// value.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::convert::TryFrom;
    /// use zvariant::{Result, Value};
    ///
    /// fn value_vec_to_type_vec<'a, T>(values: &'a Vec<Value<'a>>) -> Result<Vec<&'a T>>
    /// where
    ///     &'a T: TryFrom<&'a Value<'a>>,
    /// {
    ///     let mut res = vec![];
    ///     for value in values.into_iter() {
    ///         res.push(value.downcast_ref().unwrap());
    ///     }
    ///
    ///     Ok(res)
    /// }
    ///
    /// // Let's try u32 values first
    /// let v = vec![Value::U32(42), Value::U32(43)];
    /// let v = value_vec_to_type_vec::<u32>(&v).unwrap();
    /// assert_eq!(*v[0], 42);
    /// assert_eq!(*v[1], 43);
    ///
    /// // Now try Value values
    /// let v = vec![Value::new(Value::U32(42)), Value::new(Value::U32(43))];
    /// let v = value_vec_to_type_vec::<Value>(&v).unwrap();
    /// assert_eq!(*v[0], Value::U32(42));
    /// assert_eq!(*v[1], Value::U32(43));
    /// ```
    ///
    /// [`downcast`]: enum.Value.html#method.downcast
    pub fn downcast_ref<T>(&'a self) -> Option<&'a T>
    where
        T: ?Sized,
        &'a T: TryFrom<&'a Value<'a>>,
    {
        if let Value::Value(v) = self {
            <&T>::try_from(v).ok()
        } else {
            <&T>::try_from(self).ok()
        }
    }
}

impl<'a> Serialize for Value<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serializer implementation needs to ensure padding isn't added for Value.
        let mut structure = serializer.serialize_struct("zvariant::Value", 2)?;

        let signature = self.value_signature();
        structure.serialize_field("zvariant::Value::Signature", &signature)?;

        self.serialize_value_as_struct_field("zvariant::Value::Value", &mut structure)?;

        structure.end()
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for Value<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let visitor = ValueVisitor;

        deserializer.deserialize_any(visitor)
    }
}

// Note that the Visitor implementations don't check for validity of the
// signature. That's left to the Deserialize implementation of Signature
// itself.

struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value<'de>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a Value")
    }

    #[inline]
    fn visit_seq<V>(self, mut visitor: V) -> Result<Value<'de>, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let signature = visitor.next_element::<Signature<'_>>()?.ok_or_else(|| {
            Error::invalid_value(Unexpected::Other("nothing"), &"a Value signature")
        })?;
        let seed = ValueSeed::<Value<'_>> {
            signature,
            phantom: PhantomData,
        };

        visitor
            .next_element_seed(seed)?
            .ok_or_else(|| Error::invalid_value(Unexpected::Other("nothing"), &"a Value value"))
    }

    fn visit_map<V>(self, mut visitor: V) -> Result<Value<'de>, V::Error>
    where
        V: MapAccess<'de>,
    {
        let (_, signature) = visitor
            .next_entry::<&str, Signature<'_>>()?
            .ok_or_else(|| {
                Error::invalid_value(Unexpected::Other("nothing"), &"a Value signature")
            })?;
        let _ = visitor.next_key::<&str>()?;

        let seed = ValueSeed::<Value<'_>> {
            signature,
            phantom: PhantomData,
        };
        visitor.next_value_seed(seed)
    }
}

pub(crate) struct SignatureSeed<'de> {
    pub signature: Signature<'de>,
}

impl<'de> SignatureSeed<'de> {
    #[inline]
    pub(crate) fn visit_array<V>(self, mut visitor: V) -> Result<Array<'de>, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let element_signature = self.signature.slice(1..);
        let mut array = Array::new_full_signature(self.signature.clone());

        while let Some(elem) = visitor.next_element_seed(ValueSeed::<Value<'_>> {
            signature: element_signature.clone(),
            phantom: PhantomData,
        })? {
            elem.value_signature();
            array.append(elem).map_err(Error::custom)?;
        }

        Ok(array)
    }

    #[inline]
    pub(crate) fn visit_struct<V>(self, mut visitor: V) -> Result<Structure<'de>, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let mut i = 1;
        let signature_end = self.signature.len() - 1;
        let mut builder = StructureBuilder::new();
        while i < signature_end {
            let fields_signature = self.signature.slice(i..signature_end);
            let parser = SignatureParser::new(fields_signature.clone());
            let len = parser.next_signature().map_err(Error::custom)?.len();
            let field_signature = fields_signature.slice(0..len);
            i += field_signature.len();

            if let Some(field) = visitor.next_element_seed(ValueSeed::<Value<'_>> {
                signature: field_signature,
                phantom: PhantomData,
            })? {
                builder = builder.append_field(field);
            }
        }
        Ok(builder.build_with_signature(self.signature))
    }
}

impl<'de, T> From<ValueSeed<'de, T>> for SignatureSeed<'de> {
    fn from(seed: ValueSeed<'de, T>) -> Self {
        SignatureSeed {
            signature: seed.signature,
        }
    }
}

struct ValueSeed<'de, T> {
    signature: Signature<'de>,
    phantom: PhantomData<T>,
}

impl<'de, T> ValueSeed<'de, T>
where
    T: Deserialize<'de>,
{
    #[inline]
    fn visit_array<V>(self, visitor: V) -> Result<Value<'de>, V::Error>
    where
        V: SeqAccess<'de>,
    {
        SignatureSeed::from(self)
            .visit_array(visitor)
            .map(Value::Array)
    }

    #[inline]
    fn visit_struct<V>(self, visitor: V) -> Result<Value<'de>, V::Error>
    where
        V: SeqAccess<'de>,
    {
        SignatureSeed::from(self)
            .visit_struct(visitor)
            .map(Value::Structure)
    }

    #[inline]
    fn visit_variant<V>(self, visitor: V) -> Result<Value<'de>, V::Error>
    where
        V: SeqAccess<'de>,
    {
        ValueVisitor
            .visit_seq(visitor)
            .map(|v| Value::Value(Box::new(v)))
    }
}

macro_rules! value_seed_basic_method {
    ($name:ident, $type:ty) => {
        #[inline]
        fn $name<E>(self, value: $type) -> Result<Value<'de>, E>
        where
            E: serde::de::Error,
        {
            Ok(value.into())
        }
    };
}

macro_rules! value_seed_str_method {
    ($name:ident, $type:ty, $constructor:ident) => {
        #[inline]
        fn $name<E>(self, value: $type) -> Result<Value<'de>, E>
        where
            E: serde::de::Error,
        {
            match self.signature.as_str() {
                <&str>::SIGNATURE_STR => Ok(Value::Str(Str::from(value))),
                Signature::SIGNATURE_STR => Ok(Value::Signature(Signature::$constructor(value))),
                ObjectPath::SIGNATURE_STR => Ok(Value::ObjectPath(ObjectPath::$constructor(value))),
                _ => {
                    let expected = format!(
                        "`{}`, `{}` or `{}`",
                        <&str>::SIGNATURE_STR,
                        Signature::SIGNATURE_STR,
                        ObjectPath::SIGNATURE_STR,
                    );
                    Err(Error::invalid_type(
                        Unexpected::Str(self.signature.as_str()),
                        &expected.as_str(),
                    ))
                }
            }
        }
    };
}

impl<'de, T> Visitor<'de> for ValueSeed<'de, T>
where
    T: Deserialize<'de>,
{
    type Value = Value<'de>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a Value value")
    }

    value_seed_basic_method!(visit_bool, bool);
    value_seed_basic_method!(visit_i16, i16);
    value_seed_basic_method!(visit_i64, i64);
    value_seed_basic_method!(visit_u8, u8);
    value_seed_basic_method!(visit_u16, u16);
    value_seed_basic_method!(visit_u32, u32);
    value_seed_basic_method!(visit_u64, u64);
    value_seed_basic_method!(visit_f64, f64);

    #[inline]
    fn visit_i32<E>(self, value: i32) -> Result<Value<'de>, E>
    where
        E: serde::de::Error,
    {
        let v = match self.signature.as_bytes().first().ok_or_else(|| {
            Error::invalid_value(
                Unexpected::Other("nothing"),
                &"i32 or fd signature character",
            )
        })? {
            #[cfg(unix)]
            b'h' => Fd::from(value).into(),
            _ => value.into(),
        };

        Ok(v)
    }

    #[inline]
    fn visit_str<E>(self, value: &str) -> Result<Value<'de>, E>
    where
        E: serde::de::Error,
    {
        self.visit_string(String::from(value))
    }

    value_seed_str_method!(visit_borrowed_str, &'de str, from_str_unchecked);

    #[inline]
    fn visit_seq<V>(self, visitor: V) -> Result<Value<'de>, V::Error>
    where
        V: SeqAccess<'de>,
    {
        match self.signature.as_bytes().first().ok_or_else(|| {
            Error::invalid_value(
                Unexpected::Other("nothing"),
                &"Array or Struct signature character",
            )
        })? {
            // For some reason rustc doesn't like us using ARRAY_SIGNATURE_CHAR const
            b'a' => self.visit_array(visitor),
            b'(' => self.visit_struct(visitor),
            b'v' => self.visit_variant(visitor),
            b => Err(Error::invalid_value(
                Unexpected::Char(*b as char),
                &"a Value signature",
            )),
        }
    }

    #[inline]
    fn visit_map<V>(self, mut visitor: V) -> Result<Value<'de>, V::Error>
    where
        V: MapAccess<'de>,
    {
        if self.signature.len() < 5 {
            return Err(serde::de::Error::invalid_length(
                self.signature.len(),
                &">= 5 characters in dict entry signature",
            ));
        }
        let key_signature = self.signature.slice(2..3);
        let signature_end = self.signature.len() - 1;
        let value_signature = self.signature.slice(3..signature_end);
        let mut dict = Dict::new_full_signature(self.signature.clone());

        while let Some((key, value)) = visitor.next_entry_seed(
            ValueSeed::<Value<'_>> {
                signature: key_signature.clone(),
                phantom: PhantomData,
            },
            ValueSeed::<Value<'_>> {
                signature: value_signature.clone(),
                phantom: PhantomData,
            },
        )? {
            dict.append(key, value).map_err(Error::custom)?;
        }

        Ok(Value::Dict(dict))
    }

    #[inline]
    #[cfg(feature = "gvariant")]
    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        let visitor = ValueSeed::<T> {
            signature: self.signature.slice(1..),
            phantom: PhantomData,
        };

        deserializer
            .deserialize_any(visitor)
            .map(|v| Value::Maybe(Maybe::just_full_signature(v, self.signature)))
    }

    #[cfg(not(feature = "gvariant"))]
    fn visit_some<D>(self, _deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        panic!("`Maybe` type is only supported for GVariant format but it's disabled");
    }

    #[cfg(feature = "gvariant")]
    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let value = Maybe::nothing_full_signature(self.signature);

        Ok(Value::Maybe(value))
    }

    #[cfg(not(feature = "gvariant"))]
    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        panic!("`Maybe` type is only supported for GVariant format but it's disabled");
    }
}

impl<'de, T> DeserializeSeed<'de> for ValueSeed<'de, T>
where
    T: Deserialize<'de>,
{
    type Value = Value<'de>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

impl<'a> Type for Value<'a> {
    fn signature() -> Signature<'static> {
        Signature::from_static_str_unchecked(VARIANT_SIGNATURE_STR)
    }
}
