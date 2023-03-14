use std::{
    convert::TryInto,
    fmt::Display,
    ops::{Deref, DerefMut},
};

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::Type;

/// Type that uses a special value to be used as none.
///
/// See [`Optional`] documentation for the rationale for this trait's existence.
///
/// # Caveats
///
/// Since use of default values as none is typical, this trait is implemented for all types that
/// implement [`Default`] for convenience. Unfortunately, this means you can not implement this
/// trait manually for types that implement [`Default`].
pub trait NoneValue {
    type NoneType;

    /// The none-equivalent value.
    fn null_value() -> Self::NoneType;
}

impl<T> NoneValue for T
where
    T: Default,
{
    type NoneType = Self;

    fn null_value() -> Self {
        Default::default()
    }
}

/// An optional value.
///
/// Since D-Bus doesn't have the concept of nullability, it uses a special value (typically the
/// default value) as the null value. For example [this signal][ts] uses empty strings for null
/// values. Serde has built-in support for `Option` but unfortunately that doesn't work for us.
/// Hence the need for this type.
///
/// The serialization and deserialization of `Optional` relies on [`NoneValue`] implementation of
/// the underlying type.
///
/// [ts]: https://dbus.freedesktop.org/doc/dbus-specification.html#bus-messages-name-owner-changed
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Optional<T>(Option<T>);

impl<T> Type for Optional<T>
where
    T: Type,
{
    fn signature() -> crate::Signature<'static> {
        T::signature()
    }
}

impl<T> Serialize for Optional<T>
where
    T: Type + NoneValue + Serialize,
    <T as NoneValue>::NoneType: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match &self.0 {
            Some(value) => value.serialize(serializer),
            None => T::null_value().serialize(serializer),
        }
    }
}

impl<'de, T, E> Deserialize<'de> for Optional<T>
where
    T: Type + NoneValue + Deserialize<'de>,
    <T as NoneValue>::NoneType: Deserialize<'de> + TryInto<T, Error = E> + PartialEq,
    E: Display,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = <<T as NoneValue>::NoneType>::deserialize(deserializer)?;
        if value == T::null_value() {
            Ok(Optional(None))
        } else {
            Ok(Optional(Some(value.try_into().map_err(de::Error::custom)?)))
        }
    }
}

impl<T> From<Option<T>> for Optional<T> {
    fn from(value: Option<T>) -> Self {
        Optional(value)
    }
}

impl<T> From<Optional<T>> for Option<T> {
    fn from(value: Optional<T>) -> Self {
        value.0
    }
}

impl<T> Deref for Optional<T> {
    type Target = Option<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Optional<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
