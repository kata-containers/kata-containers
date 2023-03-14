use std::{collections::HashMap, hash::BuildHasher};

#[cfg(feature = "gvariant")]
use crate::Maybe;
use crate::{Array, Dict, ObjectPath, Signature, Str, Structure, Type, Value};

#[cfg(unix)]
use crate::Fd;

//
// Conversions from encodable types to `Value`

macro_rules! into_value {
    ($from:ty, $kind:ident) => {
        impl<'a> From<$from> for Value<'a> {
            fn from(v: $from) -> Self {
                Value::$kind(v.into())
            }
        }

        impl<'a> From<&'a $from> for Value<'a> {
            fn from(v: &'a $from) -> Self {
                Value::from(v.clone())
            }
        }
    };
}

into_value!(u8, U8);
into_value!(i8, I16);
into_value!(bool, Bool);
into_value!(u16, U16);
into_value!(i16, I16);
into_value!(u32, U32);
into_value!(i32, I32);
into_value!(u64, U64);
into_value!(i64, I64);
into_value!(f32, F64);
into_value!(f64, F64);
#[cfg(unix)]
into_value!(Fd, Fd);

into_value!(&'a str, Str);
into_value!(Str<'a>, Str);
into_value!(Signature<'a>, Signature);
into_value!(ObjectPath<'a>, ObjectPath);
into_value!(Array<'a>, Array);
into_value!(Dict<'a, 'a>, Dict);
#[cfg(feature = "gvariant")]
into_value!(Maybe<'a>, Maybe);

impl From<String> for Value<'static> {
    fn from(v: String) -> Self {
        Value::Str(crate::Str::from(v))
    }
}

impl<'v, 's: 'v, T> From<T> for Value<'v>
where
    T: Into<Structure<'s>>,
{
    fn from(v: T) -> Value<'v> {
        Value::Structure(v.into())
    }
}

impl<'v, V> From<&'v [V]> for Value<'v>
where
    &'v [V]: Into<Array<'v>>,
{
    fn from(v: &'v [V]) -> Value<'v> {
        Value::Array(v.into())
    }
}

impl<'v, V> From<Vec<V>> for Value<'v>
where
    Vec<V>: Into<Array<'v>>,
{
    fn from(v: Vec<V>) -> Value<'v> {
        Value::Array(v.into())
    }
}

impl<'v, V> From<&'v Vec<V>> for Value<'v>
where
    &'v Vec<V>: Into<Array<'v>>,
{
    fn from(v: &'v Vec<V>) -> Value<'v> {
        Value::Array(v.into())
    }
}

impl<'a, 'k, 'v, K, V, H> From<HashMap<K, V, H>> for Value<'a>
where
    'k: 'a,
    'v: 'a,
    K: Type + Into<Value<'k>> + std::hash::Hash + std::cmp::Eq,
    V: Type + Into<Value<'v>>,
    H: BuildHasher + Default,
{
    fn from(value: HashMap<K, V, H>) -> Self {
        Self::Dict(value.into())
    }
}

impl<'v> From<&'v String> for Value<'v> {
    fn from(v: &'v String) -> Value<'v> {
        Value::Str(v.into())
    }
}

#[cfg(feature = "gvariant")]
impl<'v, V> From<Option<V>> for Value<'v>
where
    Option<V>: Into<Maybe<'v>>,
{
    fn from(v: Option<V>) -> Value<'v> {
        Value::Maybe(v.into())
    }
}
