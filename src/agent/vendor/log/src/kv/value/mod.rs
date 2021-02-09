//! Structured values.

mod fill;
mod impls;
mod internal;

#[cfg(test)]
pub(in kv) mod test;

pub use self::fill::{Fill, Slot};
pub use kv::Error;

use self::internal::{Inner, Primitive, Visitor};

/// A type that can be converted into a [`Value`](struct.Value.html).
pub trait ToValue {
    /// Perform the conversion.
    fn to_value(&self) -> Value;
}

impl<'a, T> ToValue for &'a T
where
    T: ToValue + ?Sized,
{
    fn to_value(&self) -> Value {
        (**self).to_value()
    }
}

impl<'v> ToValue for Value<'v> {
    fn to_value(&self) -> Value {
        Value { inner: self.inner }
    }
}

/// A value in a structured key-value pair.
pub struct Value<'v> {
    inner: Inner<'v>,
}

impl<'v> Value<'v> {
    /// Get a value from an internal primitive.
    fn from_primitive<T>(value: T) -> Self
    where
        T: Into<Primitive<'v>>,
    {
        Value {
            inner: Inner::Primitive(value.into()),
        }
    }

    /// Visit the value using an internal visitor.
    fn visit<'a>(&'a self, visitor: &mut dyn Visitor<'a>) -> Result<(), Error> {
        self.inner.visit(visitor)
    }
}
