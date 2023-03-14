use serde::{Deserialize, Deserializer, Serialize};
use static_assertions::assert_impl_all;
use std::{collections::HashMap, convert::TryFrom, hash::BuildHasher};

use crate::{
    Array, Dict, ObjectPath, OwnedObjectPath, OwnedSignature, Signature, Str, Structure, Type,
    Value,
};

#[cfg(unix)]
use crate::Fd;

#[cfg(feature = "gvariant")]
use crate::Maybe;

// FIXME: Replace with a generic impl<T: TryFrom<Value>> TryFrom<OwnedValue> for T?
// https://gitlab.freedesktop.org/dbus/zbus/-/issues/138

/// Owned [`Value`](enum.Value.html)
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct OwnedValue(pub(crate) Value<'static>);

assert_impl_all!(OwnedValue: Send, Sync, Unpin);

impl OwnedValue {
    pub(crate) fn into_inner(self) -> Value<'static> {
        self.0
    }

    pub(crate) fn inner(&self) -> &Value<'_> {
        &self.0
    }
}

macro_rules! ov_try_from {
    ($to:ty) => {
        impl TryFrom<OwnedValue> for $to {
            type Error = crate::Error;

            fn try_from(v: OwnedValue) -> Result<Self, Self::Error> {
                <$to>::try_from(v.0)
            }
        }
    };
}

macro_rules! ov_try_from_ref {
    ($to:ty) => {
        impl<'a> TryFrom<&'a OwnedValue> for $to {
            type Error = crate::Error;

            fn try_from(v: &'a OwnedValue) -> Result<Self, Self::Error> {
                <$to>::try_from(&v.0)
            }
        }
    };
}

ov_try_from!(u8);
ov_try_from!(bool);
ov_try_from!(i16);
ov_try_from!(u16);
ov_try_from!(i32);
ov_try_from!(u32);
ov_try_from!(i64);
ov_try_from!(u64);
ov_try_from!(f64);
ov_try_from!(String);
ov_try_from!(Signature<'static>);
ov_try_from!(OwnedSignature);
ov_try_from!(ObjectPath<'static>);
ov_try_from!(OwnedObjectPath);
ov_try_from!(Array<'static>);
ov_try_from!(Dict<'static, 'static>);
#[cfg(feature = "gvariant")]
ov_try_from!(Maybe<'static>);
ov_try_from!(Str<'static>);
ov_try_from!(Structure<'static>);
#[cfg(unix)]
ov_try_from!(Fd);

ov_try_from_ref!(u8);
ov_try_from_ref!(bool);
ov_try_from_ref!(i16);
ov_try_from_ref!(u16);
ov_try_from_ref!(i32);
ov_try_from_ref!(u32);
ov_try_from_ref!(i64);
ov_try_from_ref!(u64);
ov_try_from_ref!(f64);
ov_try_from_ref!(&'a str);
ov_try_from_ref!(&'a Signature<'a>);
ov_try_from_ref!(&'a ObjectPath<'a>);
ov_try_from_ref!(&'a Array<'a>);
ov_try_from_ref!(&'a Dict<'a, 'a>);
ov_try_from_ref!(&'a Str<'a>);
ov_try_from_ref!(&'a Structure<'a>);
#[cfg(feature = "gvariant")]
ov_try_from_ref!(&'a Maybe<'a>);
#[cfg(unix)]
ov_try_from_ref!(Fd);

impl<'a, T> TryFrom<OwnedValue> for Vec<T>
where
    T: TryFrom<Value<'a>>,
    T::Error: Into<crate::Error>,
{
    type Error = crate::Error;

    fn try_from(value: OwnedValue) -> Result<Self, Self::Error> {
        if let Value::Array(v) = value.0 {
            Self::try_from(v)
        } else {
            Err(crate::Error::IncorrectType)
        }
    }
}

#[cfg(feature = "enumflags2")]
impl<'a, F> TryFrom<OwnedValue> for enumflags2::BitFlags<F>
where
    F: enumflags2::BitFlag,
    F::Numeric: TryFrom<Value<'a>, Error = crate::Error>,
{
    type Error = crate::Error;

    fn try_from(value: OwnedValue) -> Result<Self, Self::Error> {
        Self::try_from(value.0)
    }
}

impl<'k, 'v, K, V, H> TryFrom<OwnedValue> for HashMap<K, V, H>
where
    K: crate::Basic + TryFrom<Value<'k>> + std::hash::Hash + std::cmp::Eq,
    V: TryFrom<Value<'v>>,
    H: BuildHasher + Default,
    K::Error: Into<crate::Error>,
    V::Error: Into<crate::Error>,
{
    type Error = crate::Error;

    fn try_from(value: OwnedValue) -> Result<Self, Self::Error> {
        if let Value::Dict(v) = value.0 {
            Self::try_from(v)
        } else {
            Err(crate::Error::IncorrectType)
        }
    }
}

impl<K, V, H> From<HashMap<K, V, H>> for OwnedValue
where
    K: Type + Into<Value<'static>> + std::hash::Hash + std::cmp::Eq,
    V: Type + Into<Value<'static>>,
    H: BuildHasher + Default,
{
    fn from(value: HashMap<K, V, H>) -> Self {
        Self(value.into())
    }
}

// tuple conversions in `structure` module for avoiding code-duplication.

impl<'a> From<Value<'a>> for OwnedValue {
    fn from(v: Value<'a>) -> Self {
        // TODO: add into_owned, avoiding copy if already owned..
        v.to_owned()
    }
}

impl<'a> From<&Value<'a>> for OwnedValue {
    fn from(v: &Value<'a>) -> Self {
        v.to_owned()
    }
}

macro_rules! to_value {
    ($from:ty) => {
        impl<'a> From<$from> for OwnedValue {
            fn from(v: $from) -> Self {
                OwnedValue::from(<Value<'a>>::from(v))
            }
        }
    };
}

to_value!(u8);
to_value!(bool);
to_value!(i16);
to_value!(u16);
to_value!(i32);
to_value!(u32);
to_value!(i64);
to_value!(u64);
to_value!(f64);
to_value!(Array<'a>);
to_value!(Dict<'a, 'a>);
#[cfg(feature = "gvariant")]
to_value!(Maybe<'a>);
to_value!(Str<'a>);
to_value!(Signature<'a>);
to_value!(Structure<'a>);
to_value!(ObjectPath<'a>);
#[cfg(unix)]
to_value!(Fd);

impl From<OwnedValue> for Value<'static> {
    fn from(v: OwnedValue) -> Value<'static> {
        v.into_inner()
    }
}

impl<'o> From<&'o OwnedValue> for Value<'o> {
    fn from(v: &'o OwnedValue) -> Value<'o> {
        v.inner().clone()
    }
}

impl std::ops::Deref for OwnedValue {
    type Target = Value<'static>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'de> Deserialize<'de> for OwnedValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Value::deserialize(deserializer)?.into())
    }
}

#[cfg(test)]
mod tests {
    use byteorder::LE;
    use std::{collections::HashMap, convert::TryFrom, error::Error, result::Result};

    use crate::{from_slice, to_bytes, EncodingContext, OwnedValue, Value};

    #[cfg(feature = "enumflags2")]
    #[test]
    fn bitflags() -> Result<(), Box<dyn Error>> {
        #[repr(u32)]
        #[enumflags2::bitflags]
        #[derive(Copy, Clone, Debug)]
        pub enum Flaggy {
            One = 0x1,
            Two = 0x2,
        }

        let v = Value::from(0x2u32);
        let ov: OwnedValue = v.into();
        assert_eq!(<enumflags2::BitFlags<Flaggy>>::try_from(ov)?, Flaggy::Two);
        Ok(())
    }

    #[test]
    fn from_value() -> Result<(), Box<dyn Error>> {
        let v = Value::from("hi!");
        let ov: OwnedValue = v.into();
        assert_eq!(<&str>::try_from(&ov)?, "hi!");
        Ok(())
    }

    #[test]
    fn serde() -> Result<(), Box<dyn Error>> {
        let ec = EncodingContext::<LE>::new_dbus(0);
        let ov: OwnedValue = Value::from("hi!").into();
        let ser = to_bytes(ec, &ov)?;
        let de: Value<'_> = from_slice(&ser, ec)?;
        assert_eq!(<&str>::try_from(&de)?, "hi!");
        Ok(())
    }

    #[test]
    fn map_conversion() -> Result<(), Box<dyn Error>> {
        let mut map = HashMap::<String, String>::new();
        map.insert("one".to_string(), "1".to_string());
        map.insert("two".to_string(), "2".to_string());
        let value = OwnedValue::from(map.clone());
        // Now convert back
        let map2 = <HashMap<String, String>>::try_from(value)?;
        assert_eq!(map, map2);

        Ok(())
    }
}
