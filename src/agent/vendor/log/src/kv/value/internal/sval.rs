//! Integration between `Value` and `sval`.
//!
//! This module allows any `Value` to implement the `sval::Value` trait,
//! and for any `sval::Value` to be captured as a `Value`.

extern crate sval;

use std::fmt;

use super::cast::Cast;
use super::{Erased, Inner, Primitive, Visitor};
use crate::kv;
use crate::kv::value::{Error, Slot};

impl<'v> kv::Value<'v> {
    /// Get a value from a structured type.
    pub fn from_sval<T>(value: &'v T) -> Self
    where
        T: sval::Value + 'static,
    {
        kv::Value {
            inner: Inner::Sval(unsafe { Erased::new_unchecked::<T>(value) }),
        }
    }
}

impl<'s, 'f> Slot<'s, 'f> {
    /// Fill the slot with a structured value.
    ///
    /// The given value doesn't need to satisfy any particular lifetime constraints.
    ///
    /// # Panics
    ///
    /// Calling more than a single `fill` method on this slot will panic.
    pub fn fill_sval<T>(&mut self, value: T) -> Result<(), Error>
    where
        T: sval::Value,
    {
        self.fill(|visitor| visitor.sval(&value))
    }
}

impl<'v> sval::Value for kv::Value<'v> {
    fn stream(&self, s: &mut sval::value::Stream) -> sval::value::Result {
        struct SvalVisitor<'a, 'b: 'a>(&'a mut sval::value::Stream<'b>);

        impl<'a, 'b: 'a, 'v> Visitor<'v> for SvalVisitor<'a, 'b> {
            fn debug(&mut self, v: &dyn fmt::Debug) -> Result<(), Error> {
                self.0
                    .fmt(format_args!("{:?}", v))
                    .map_err(Error::from_sval)
            }

            fn u64(&mut self, v: u64) -> Result<(), Error> {
                self.0.u64(v).map_err(Error::from_sval)
            }

            fn i64(&mut self, v: i64) -> Result<(), Error> {
                self.0.i64(v).map_err(Error::from_sval)
            }

            fn f64(&mut self, v: f64) -> Result<(), Error> {
                self.0.f64(v).map_err(Error::from_sval)
            }

            fn bool(&mut self, v: bool) -> Result<(), Error> {
                self.0.bool(v).map_err(Error::from_sval)
            }

            fn char(&mut self, v: char) -> Result<(), Error> {
                self.0.char(v).map_err(Error::from_sval)
            }

            fn str(&mut self, v: &str) -> Result<(), Error> {
                self.0.str(v).map_err(Error::from_sval)
            }

            fn none(&mut self) -> Result<(), Error> {
                self.0.none().map_err(Error::from_sval)
            }

            fn sval(&mut self, v: &dyn sval::Value) -> Result<(), Error> {
                self.0.any(v).map_err(Error::from_sval)
            }
        }

        self.visit(&mut SvalVisitor(s)).map_err(Error::into_sval)?;

        Ok(())
    }
}

pub(in kv::value) use self::sval::Value;

pub(super) fn fmt(f: &mut fmt::Formatter, v: &dyn sval::Value) -> Result<(), Error> {
    sval::fmt::debug(f, v)?;
    Ok(())
}

pub(super) fn cast<'v>(v: &dyn sval::Value) -> Cast<'v> {
    struct CastStream<'v>(Cast<'v>);

    impl<'v> sval::Stream for CastStream<'v> {
        fn u64(&mut self, v: u64) -> sval::stream::Result {
            self.0 = Cast::Primitive(Primitive::Unsigned(v));
            Ok(())
        }

        fn i64(&mut self, v: i64) -> sval::stream::Result {
            self.0 = Cast::Primitive(Primitive::Signed(v));
            Ok(())
        }

        fn f64(&mut self, v: f64) -> sval::stream::Result {
            self.0 = Cast::Primitive(Primitive::Float(v));
            Ok(())
        }

        fn char(&mut self, v: char) -> sval::stream::Result {
            self.0 = Cast::Primitive(Primitive::Char(v));
            Ok(())
        }

        fn bool(&mut self, v: bool) -> sval::stream::Result {
            self.0 = Cast::Primitive(Primitive::Bool(v));
            Ok(())
        }

        #[cfg(feature = "std")]
        fn str(&mut self, s: &str) -> sval::stream::Result {
            self.0 = Cast::String(s.into());
            Ok(())
        }
    }

    let mut cast = CastStream(Cast::Primitive(Primitive::None));
    let _ = sval::stream(&mut cast, v);

    cast.0
}

impl Error {
    fn from_sval(_: sval::value::Error) -> Self {
        Error::msg("`sval` serialization failed")
    }

    fn into_sval(self) -> sval::value::Error {
        sval::value::Error::msg("`sval` serialization failed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kv::value::test::Token;

    #[test]
    fn test_from_sval() {
        assert_eq!(kv::Value::from_sval(&42u64).to_token(), Token::Sval);
    }

    #[test]
    fn test_sval_structured() {
        let value = kv::Value::from(42u64);
        let expected = vec![sval::test::Token::Unsigned(42)];

        assert_eq!(sval::test::tokens(value), expected);
    }

    #[test]
    fn sval_cast() {
        assert_eq!(
            42u32,
            kv::Value::from_sval(&42u64)
                .to_u32()
                .expect("invalid value")
        );

        assert_eq!(
            "a string",
            kv::Value::from_sval(&"a string")
                .to_borrowed_str()
                .expect("invalid value")
        );

        #[cfg(feature = "std")]
        assert_eq!(
            "a string",
            kv::Value::from_sval(&"a string")
                .to_str()
                .expect("invalid value")
        );
    }

    #[test]
    fn sval_debug() {
        struct TestSval;

        impl sval::Value for TestSval {
            fn stream(&self, stream: &mut sval::value::Stream) -> sval::value::Result {
                stream.u64(42)
            }
        }

        assert_eq!(
            format!("{:04?}", 42u64),
            format!("{:04?}", kv::Value::from_sval(&TestSval)),
        );
    }
}
