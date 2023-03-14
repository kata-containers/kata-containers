#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt;

/// Key used for metric `AttributeSet`s and trace `Span` attributes.
#[cfg_attr(feature = "serialize", derive(Deserialize, Serialize))]
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key(Cow<'static, str>);

impl Key {
    /// Create a new `Key`.
    pub fn new<S: Into<Cow<'static, str>>>(value: S) -> Self {
        Key(value.into())
    }

    /// Create a new const `Key`.
    pub const fn from_static_str(value: &'static str) -> Self {
        Key(Cow::Borrowed(value))
    }

    /// Create a `KeyValue` pair for `bool` values.
    pub fn bool<T: Into<bool>>(self, value: T) -> KeyValue {
        KeyValue {
            key: self,
            value: Value::Bool(value.into()),
        }
    }

    /// Create a `KeyValue` pair for `i64` values.
    pub fn i64(self, value: i64) -> KeyValue {
        KeyValue {
            key: self,
            value: Value::I64(value),
        }
    }

    /// Create a `KeyValue` pair for `f64` values.
    pub fn f64(self, value: f64) -> KeyValue {
        KeyValue {
            key: self,
            value: Value::F64(value),
        }
    }

    /// Create a `KeyValue` pair for `String` values.
    pub fn string<T: Into<Cow<'static, str>>>(self, value: T) -> KeyValue {
        KeyValue {
            key: self,
            value: Value::String(value.into()),
        }
    }

    /// Create a `KeyValue` pair for arrays.
    pub fn array<T: Into<Array>>(self, value: T) -> KeyValue {
        KeyValue {
            key: self,
            value: Value::Array(value.into()),
        }
    }

    /// Returns a reference to the underlying key name
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl From<&'static str> for Key {
    /// Convert a `&str` to a `Key`.
    fn from(key_str: &'static str) -> Self {
        Key(Cow::from(key_str))
    }
}

impl From<String> for Key {
    /// Convert a `String` to a `Key`.
    fn from(string: String) -> Self {
        Key(Cow::from(string))
    }
}

impl From<Key> for String {
    /// Converts `Key` instances into `String`.
    fn from(key: Key) -> Self {
        key.0.into_owned()
    }
}

impl fmt::Display for Key {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(fmt)
    }
}

/// Array of homogeneous values
#[cfg_attr(feature = "serialize", derive(Deserialize, Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum Array {
    /// Array of bools
    Bool(Vec<bool>),
    /// Array of integers
    I64(Vec<i64>),
    /// Array of floats
    F64(Vec<f64>),
    /// Array of strings
    String(Vec<Cow<'static, str>>),
}

impl fmt::Display for Array {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Array::Bool(values) => display_array_str(values, fmt),
            Array::I64(values) => display_array_str(values, fmt),
            Array::F64(values) => display_array_str(values, fmt),
            Array::String(values) => {
                write!(fmt, "[")?;
                for (i, t) in values.iter().enumerate() {
                    if i > 0 {
                        write!(fmt, ",")?;
                    }
                    write!(fmt, "{:?}", t)?;
                }
                write!(fmt, "]")
            }
        }
    }
}

fn display_array_str<T: fmt::Display>(slice: &[T], fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(fmt, "[")?;
    for (i, t) in slice.iter().enumerate() {
        if i > 0 {
            write!(fmt, ",")?;
        }
        write!(fmt, "{}", t)?;
    }
    write!(fmt, "]")
}

macro_rules! into_array {
    ($(($t:ty, $val:expr),)+) => {
        $(
            impl From<$t> for Array {
                fn from(t: $t) -> Self {
                    $val(t)
                }
            }
        )+
    }
}

into_array!(
    (Vec<bool>, Array::Bool),
    (Vec<i64>, Array::I64),
    (Vec<f64>, Array::F64),
    (Vec<Cow<'static, str>>, Array::String),
);

/// Value types for use in `KeyValue` pairs.
#[cfg_attr(feature = "serialize", derive(Deserialize, Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// bool values
    Bool(bool),
    /// i64 values
    I64(i64),
    /// f64 values
    F64(f64),
    /// String values
    String(Cow<'static, str>),
    /// Array of homogeneous values
    Array(Array),
}

impl Value {
    /// String representation of the `Value`
    ///
    /// This will allocate iff the underlying value is not a `String`.
    pub fn as_str(&self) -> Cow<'_, str> {
        match self {
            Value::Bool(v) => format!("{}", v).into(),
            Value::I64(v) => format!("{}", v).into(),
            Value::F64(v) => format!("{}", v).into(),
            Value::String(v) => Cow::Borrowed(v.as_ref()),
            Value::Array(v) => format!("{}", v).into(),
        }
    }
}

macro_rules! from_values {
   (
        $(
            ($t:ty, $val:expr);
        )+
    ) => {
        $(
            impl From<$t> for Value {
                fn from(t: $t) -> Self {
                    $val(t)
                }
            }
        )+
    }
}

from_values!(
    (bool, Value::Bool);
    (i64, Value::I64);
    (f64, Value::F64);
    (Cow<'static, str>, Value::String);
);

impl From<&'static str> for Value {
    /// Convenience method for creating a `Value` from a `&'static str`.
    fn from(s: &'static str) -> Self {
        Value::String(s.into())
    }
}

impl From<String> for Value {
    /// Convenience method for creating a `Value` from a `String`.
    fn from(s: String) -> Self {
        Value::String(s.into())
    }
}

impl fmt::Display for Value {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bool(v) => fmt.write_fmt(format_args!("{}", v)),
            Value::I64(v) => fmt.write_fmt(format_args!("{}", v)),
            Value::F64(v) => fmt.write_fmt(format_args!("{}", v)),
            Value::String(v) => fmt.write_fmt(format_args!("{}", v)),
            Value::Array(v) => fmt.write_fmt(format_args!("{}", v)),
        }
    }
}

/// `KeyValue` pairs are used by `AttributeSet`s and `Span` attributes.
#[cfg_attr(feature = "serialize", derive(Deserialize, Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct KeyValue {
    /// Dimension or event key
    pub key: Key,
    /// Dimension or event value
    pub value: Value,
}

impl KeyValue {
    /// Create a new `KeyValue` pair.
    pub fn new<K, V>(key: K, value: V) -> Self
    where
        K: Into<Key>,
        V: Into<Value>,
    {
        KeyValue {
            key: key.into(),
            value: value.into(),
        }
    }
}
