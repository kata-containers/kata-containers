//! Coerce a `Value` into some concrete types.
//!
//! These operations are cheap when the captured value is a simple primitive,
//! but may end up executing arbitrary caller code if the value is complex.
//! They will also attempt to downcast erased types into a primitive where possible.

use std::any::TypeId;
use std::fmt;

use super::{Erased, Inner, Primitive, Visitor};
use crate::kv::value::{Error, Value};

impl<'v> Value<'v> {
    /// Try get a `usize` from this value.
    ///
    /// This method is cheap for primitive types, but may call arbitrary
    /// serialization implementations for complex ones.
    pub fn to_usize(&self) -> Option<usize> {
        self.inner
            .cast()
            .into_primitive()
            .into_u64()
            .map(|v| v as usize)
    }

    /// Try get a `u8` from this value.
    ///
    /// This method is cheap for primitive types, but may call arbitrary
    /// serialization implementations for complex ones.
    pub fn to_u8(&self) -> Option<u8> {
        self.inner
            .cast()
            .into_primitive()
            .into_u64()
            .map(|v| v as u8)
    }

    /// Try get a `u16` from this value.
    ///
    /// This method is cheap for primitive types, but may call arbitrary
    /// serialization implementations for complex ones.
    pub fn to_u16(&self) -> Option<u16> {
        self.inner
            .cast()
            .into_primitive()
            .into_u64()
            .map(|v| v as u16)
    }

    /// Try get a `u32` from this value.
    ///
    /// This method is cheap for primitive types, but may call arbitrary
    /// serialization implementations for complex ones.
    pub fn to_u32(&self) -> Option<u32> {
        self.inner
            .cast()
            .into_primitive()
            .into_u64()
            .map(|v| v as u32)
    }

    /// Try get a `u64` from this value.
    ///
    /// This method is cheap for primitive types, but may call arbitrary
    /// serialization implementations for complex ones.
    pub fn to_u64(&self) -> Option<u64> {
        self.inner.cast().into_primitive().into_u64()
    }

    /// Try get a `isize` from this value.
    ///
    /// This method is cheap for primitive types, but may call arbitrary
    /// serialization implementations for complex ones.
    pub fn to_isize(&self) -> Option<isize> {
        self.inner
            .cast()
            .into_primitive()
            .into_i64()
            .map(|v| v as isize)
    }

    /// Try get a `i8` from this value.
    ///
    /// This method is cheap for primitive types, but may call arbitrary
    /// serialization implementations for complex ones.
    pub fn to_i8(&self) -> Option<i8> {
        self.inner
            .cast()
            .into_primitive()
            .into_i64()
            .map(|v| v as i8)
    }

    /// Try get a `i16` from this value.
    ///
    /// This method is cheap for primitive types, but may call arbitrary
    /// serialization implementations for complex ones.
    pub fn to_i16(&self) -> Option<i16> {
        self.inner
            .cast()
            .into_primitive()
            .into_i64()
            .map(|v| v as i16)
    }

    /// Try get a `i32` from this value.
    ///
    /// This method is cheap for primitive types, but may call arbitrary
    /// serialization implementations for complex ones.
    pub fn to_i32(&self) -> Option<i32> {
        self.inner
            .cast()
            .into_primitive()
            .into_i64()
            .map(|v| v as i32)
    }

    /// Try get a `i64` from this value.
    ///
    /// This method is cheap for primitive types, but may call arbitrary
    /// serialization implementations for complex ones.
    pub fn to_i64(&self) -> Option<i64> {
        self.inner.cast().into_primitive().into_i64()
    }

    /// Try get a `f32` from this value.
    ///
    /// This method is cheap for primitive types, but may call arbitrary
    /// serialization implementations for complex ones.
    pub fn to_f32(&self) -> Option<f32> {
        self.inner
            .cast()
            .into_primitive()
            .into_f64()
            .map(|v| v as f32)
    }

    /// Try get a `f64` from this value.
    ///
    /// This method is cheap for primitive types, but may call arbitrary
    /// serialization implementations for complex ones.
    pub fn to_f64(&self) -> Option<f64> {
        self.inner.cast().into_primitive().into_f64()
    }

    /// Try get a `bool` from this value.
    ///
    /// This method is cheap for primitive types, but may call arbitrary
    /// serialization implementations for complex ones.
    pub fn to_bool(&self) -> Option<bool> {
        self.inner.cast().into_primitive().into_bool()
    }

    /// Try get a `char` from this value.
    ///
    /// This method is cheap for primitive types, but may call arbitrary
    /// serialization implementations for complex ones.
    pub fn to_char(&self) -> Option<char> {
        self.inner.cast().into_primitive().into_char()
    }

    /// Try get a `str` from this value.
    ///
    /// This method is cheap for primitive types. It won't allocate an owned
    /// `String` if the value is a complex type.
    pub fn to_borrowed_str(&self) -> Option<&str> {
        self.inner.cast().into_primitive().into_borrowed_str()
    }
}

impl<'v> Inner<'v> {
    /// Cast the inner value to another type.
    fn cast(self) -> Cast<'v> {
        struct CastVisitor<'v>(Cast<'v>);

        impl<'v> Visitor<'v> for CastVisitor<'v> {
            fn debug(&mut self, _: &dyn fmt::Debug) -> Result<(), Error> {
                Ok(())
            }

            fn u64(&mut self, v: u64) -> Result<(), Error> {
                self.0 = Cast::Primitive(Primitive::Unsigned(v));
                Ok(())
            }

            fn i64(&mut self, v: i64) -> Result<(), Error> {
                self.0 = Cast::Primitive(Primitive::Signed(v));
                Ok(())
            }

            fn f64(&mut self, v: f64) -> Result<(), Error> {
                self.0 = Cast::Primitive(Primitive::Float(v));
                Ok(())
            }

            fn bool(&mut self, v: bool) -> Result<(), Error> {
                self.0 = Cast::Primitive(Primitive::Bool(v));
                Ok(())
            }

            fn char(&mut self, v: char) -> Result<(), Error> {
                self.0 = Cast::Primitive(Primitive::Char(v));
                Ok(())
            }

            fn borrowed_str(&mut self, v: &'v str) -> Result<(), Error> {
                self.0 = Cast::Primitive(Primitive::Str(v));
                Ok(())
            }

            #[cfg(not(feature = "std"))]
            fn str(&mut self, _: &str) -> Result<(), Error> {
                Ok(())
            }

            #[cfg(feature = "std")]
            fn str(&mut self, v: &str) -> Result<(), Error> {
                self.0 = Cast::String(v.into());
                Ok(())
            }

            fn none(&mut self) -> Result<(), Error> {
                self.0 = Cast::Primitive(Primitive::None);
                Ok(())
            }

            #[cfg(feature = "kv_unstable_sval")]
            fn sval(&mut self, v: &dyn super::sval::Value) -> Result<(), Error> {
                self.0 = super::sval::cast(v);
                Ok(())
            }
        }

        // Try downcast an erased value first
        // It also lets us avoid the Visitor infrastructure for simple primitives
        let primitive = match self {
            Inner::Primitive(value) => Some(value),
            Inner::Fill(value) => value.downcast_primitive(),
            Inner::Debug(value) => value.downcast_primitive(),
            Inner::Display(value) => value.downcast_primitive(),

            #[cfg(feature = "sval")]
            Inner::Sval(value) => value.downcast_primitive(),
        };

        primitive.map(Cast::Primitive).unwrap_or_else(|| {
            // If the erased value isn't a primitive then we visit it
            let mut cast = CastVisitor(Cast::Primitive(Primitive::None));
            let _ = self.visit(&mut cast);
            cast.0
        })
    }
}

pub(super) enum Cast<'v> {
    Primitive(Primitive<'v>),
    #[cfg(feature = "std")]
    String(String),
}

impl<'v> Cast<'v> {
    fn into_primitive(self) -> Primitive<'v> {
        match self {
            Cast::Primitive(value) => value,
            #[cfg(feature = "std")]
            _ => Primitive::None,
        }
    }
}

impl<'v> Primitive<'v> {
    fn into_borrowed_str(self) -> Option<&'v str> {
        if let Primitive::Str(value) = self {
            Some(value)
        } else {
            None
        }
    }

    fn into_u64(self) -> Option<u64> {
        match self {
            Primitive::Unsigned(value) => Some(value),
            Primitive::Signed(value) => Some(value as u64),
            Primitive::Float(value) => Some(value as u64),
            _ => None,
        }
    }

    fn into_i64(self) -> Option<i64> {
        match self {
            Primitive::Signed(value) => Some(value),
            Primitive::Unsigned(value) => Some(value as i64),
            Primitive::Float(value) => Some(value as i64),
            _ => None,
        }
    }

    fn into_f64(self) -> Option<f64> {
        match self {
            Primitive::Float(value) => Some(value),
            Primitive::Unsigned(value) => Some(value as f64),
            Primitive::Signed(value) => Some(value as f64),
            _ => None,
        }
    }

    fn into_char(self) -> Option<char> {
        if let Primitive::Char(value) = self {
            Some(value)
        } else {
            None
        }
    }

    fn into_bool(self) -> Option<bool> {
        if let Primitive::Bool(value) = self {
            Some(value)
        } else {
            None
        }
    }
}

impl<'v, T: ?Sized + 'static> Erased<'v, T> {
    // NOTE: This function is a perfect candidate for memoization
    // The outcome could be stored in a `Cell<Primitive>`
    fn downcast_primitive(self) -> Option<Primitive<'v>> {
        macro_rules! type_ids {
            ($($value:ident : $ty:ty => $cast:expr,)*) => {{
                struct TypeIds;

                impl TypeIds {
                    fn downcast_primitive<'v, T: ?Sized>(&self, value: Erased<'v, T>) -> Option<Primitive<'v>> {
                        $(
                            if TypeId::of::<$ty>() == value.type_id {
                                let $value = unsafe { value.downcast_unchecked::<$ty>() };
                                return Some(Primitive::from($cast));
                            }
                        )*

                        None
                    }
                }

                TypeIds
            }};
        }

        let type_ids = type_ids![
            value: usize => *value as u64,
            value: u8 => *value as u64,
            value: u16 => *value as u64,
            value: u32 => *value as u64,
            value: u64 => *value,

            value: isize => *value as i64,
            value: i8 => *value as i64,
            value: i16 => *value as i64,
            value: i32 => *value as i64,
            value: i64 => *value,

            value: f32 => *value as f64,
            value: f64 => *value,

            value: char => *value,
            value: bool => *value,

            value: &str => *value,
        ];

        type_ids.downcast_primitive(self)
    }
}

#[cfg(feature = "std")]
mod std_support {
    use super::*;

    use std::borrow::Cow;

    impl<'v> Value<'v> {
        /// Try get a `usize` from this value.
        ///
        /// This method is cheap for primitive types, but may call arbitrary
        /// serialization implementations for complex ones. If the serialization
        /// implementation produces a short lived string it will be allocated.
        pub fn to_str(&self) -> Option<Cow<str>> {
            self.inner.cast().into_str()
        }
    }

    impl<'v> Cast<'v> {
        pub(super) fn into_str(self) -> Option<Cow<'v, str>> {
            match self {
                Cast::Primitive(Primitive::Str(value)) => Some(value.into()),
                Cast::String(value) => Some(value.into()),
                _ => None,
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use crate::kv::ToValue;

        #[test]
        fn primitive_cast() {
            assert_eq!(
                "a string",
                "a string"
                    .to_owned()
                    .to_value()
                    .to_borrowed_str()
                    .expect("invalid value")
            );
            assert_eq!(
                "a string",
                &*"a string".to_value().to_str().expect("invalid value")
            );
            assert_eq!(
                "a string",
                &*"a string"
                    .to_owned()
                    .to_value()
                    .to_str()
                    .expect("invalid value")
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::kv::ToValue;

    #[test]
    fn primitive_cast() {
        assert_eq!(
            "a string",
            "a string"
                .to_value()
                .to_borrowed_str()
                .expect("invalid value")
        );
        assert_eq!(
            "a string",
            Some("a string")
                .to_value()
                .to_borrowed_str()
                .expect("invalid value")
        );

        assert_eq!(1u8, 1u64.to_value().to_u8().expect("invalid value"));
        assert_eq!(1u16, 1u64.to_value().to_u16().expect("invalid value"));
        assert_eq!(1u32, 1u64.to_value().to_u32().expect("invalid value"));
        assert_eq!(1u64, 1u64.to_value().to_u64().expect("invalid value"));
        assert_eq!(1usize, 1u64.to_value().to_usize().expect("invalid value"));

        assert_eq!(-1i8, -1i64.to_value().to_i8().expect("invalid value"));
        assert_eq!(-1i16, -1i64.to_value().to_i16().expect("invalid value"));
        assert_eq!(-1i32, -1i64.to_value().to_i32().expect("invalid value"));
        assert_eq!(-1i64, -1i64.to_value().to_i64().expect("invalid value"));
        assert_eq!(-1isize, -1i64.to_value().to_isize().expect("invalid value"));

        assert!(1f32.to_value().to_f32().is_some(), "invalid value");
        assert!(1f64.to_value().to_f64().is_some(), "invalid value");

        assert_eq!(1u32, 1i64.to_value().to_u32().expect("invalid value"));
        assert_eq!(1i32, 1u64.to_value().to_i32().expect("invalid value"));
        assert!(1f32.to_value().to_i32().is_some(), "invalid value");

        assert_eq!('a', 'a'.to_value().to_char().expect("invalid value"));
        assert_eq!(true, true.to_value().to_bool().expect("invalid value"));
    }
}
