//! The internal `Value` serialization API.
//!
//! This implementation isn't intended to be public. It may need to change
//! for optimizations or to support new external serialization frameworks.

use std::any::TypeId;

use super::{Error, Fill, Slot};

pub(super) mod cast;
pub(super) mod fmt;
#[cfg(feature = "kv_unstable_sval")]
pub(super) mod sval;

/// A container for a structured value for a specific kind of visitor.
#[derive(Clone, Copy)]
pub(super) enum Inner<'v> {
    /// A simple primitive value that can be copied without allocating.
    Primitive(Primitive<'v>),
    /// A value that can be filled.
    Fill(Erased<'v, dyn Fill + 'static>),
    /// A debuggable value.
    Debug(Erased<'v, dyn fmt::Debug + 'static>),
    /// A displayable value.
    Display(Erased<'v, dyn fmt::Display + 'static>),

    #[cfg(feature = "kv_unstable_sval")]
    /// A structured value from `sval`.
    Sval(Erased<'v, dyn sval::Value + 'static>),
}

impl<'v> Inner<'v> {
    pub(super) fn visit(self, visitor: &mut dyn Visitor<'v>) -> Result<(), Error> {
        match self {
            Inner::Primitive(value) => value.visit(visitor),
            Inner::Fill(value) => value.get().fill(&mut Slot::new(visitor)),
            Inner::Debug(value) => visitor.debug(value.get()),
            Inner::Display(value) => visitor.display(value.get()),

            #[cfg(feature = "kv_unstable_sval")]
            Inner::Sval(value) => visitor.sval(value.get()),
        }
    }
}

/// The internal serialization contract.
pub(super) trait Visitor<'v> {
    fn debug(&mut self, v: &dyn fmt::Debug) -> Result<(), Error>;
    fn display(&mut self, v: &dyn fmt::Display) -> Result<(), Error> {
        self.debug(&format_args!("{}", v))
    }

    fn u64(&mut self, v: u64) -> Result<(), Error>;
    fn i64(&mut self, v: i64) -> Result<(), Error>;
    fn f64(&mut self, v: f64) -> Result<(), Error>;
    fn bool(&mut self, v: bool) -> Result<(), Error>;
    fn char(&mut self, v: char) -> Result<(), Error>;

    fn str(&mut self, v: &str) -> Result<(), Error>;
    fn borrowed_str(&mut self, v: &'v str) -> Result<(), Error> {
        self.str(v)
    }

    fn none(&mut self) -> Result<(), Error>;

    #[cfg(feature = "kv_unstable_sval")]
    fn sval(&mut self, v: &dyn sval::Value) -> Result<(), Error>;
}

/// A captured primitive value.
///
/// These values are common and cheap to copy around.
#[derive(Clone, Copy)]
pub(super) enum Primitive<'v> {
    Signed(i64),
    Unsigned(u64),
    Float(f64),
    Bool(bool),
    Char(char),
    Str(&'v str),
    Fmt(fmt::Arguments<'v>),
    None,
}

impl<'v> Primitive<'v> {
    fn visit(self, visitor: &mut dyn Visitor<'v>) -> Result<(), Error> {
        match self {
            Primitive::Signed(value) => visitor.i64(value),
            Primitive::Unsigned(value) => visitor.u64(value),
            Primitive::Float(value) => visitor.f64(value),
            Primitive::Bool(value) => visitor.bool(value),
            Primitive::Char(value) => visitor.char(value),
            Primitive::Str(value) => visitor.borrowed_str(value),
            Primitive::Fmt(value) => visitor.debug(&value),
            Primitive::None => visitor.none(),
        }
    }
}

impl<'v> From<u64> for Primitive<'v> {
    fn from(v: u64) -> Self {
        Primitive::Unsigned(v)
    }
}

impl<'v> From<i64> for Primitive<'v> {
    fn from(v: i64) -> Self {
        Primitive::Signed(v)
    }
}

impl<'v> From<f64> for Primitive<'v> {
    fn from(v: f64) -> Self {
        Primitive::Float(v)
    }
}

impl<'v> From<bool> for Primitive<'v> {
    fn from(v: bool) -> Self {
        Primitive::Bool(v)
    }
}

impl<'v> From<char> for Primitive<'v> {
    fn from(v: char) -> Self {
        Primitive::Char(v)
    }
}

impl<'v> From<&'v str> for Primitive<'v> {
    fn from(v: &'v str) -> Self {
        Primitive::Str(v)
    }
}

impl<'v> From<fmt::Arguments<'v>> for Primitive<'v> {
    fn from(v: fmt::Arguments<'v>) -> Self {
        Primitive::Fmt(v)
    }
}

/// A downcastable dynamic type.
pub(super) struct Erased<'v, T: ?Sized> {
    type_id: TypeId,
    inner: &'v T,
}

impl<'v, T: ?Sized> Clone for Erased<'v, T> {
    fn clone(&self) -> Self {
        Erased {
            type_id: self.type_id,
            inner: self.inner,
        }
    }
}

impl<'v, T: ?Sized> Copy for Erased<'v, T> {}

impl<'v, T: ?Sized> Erased<'v, T> {
    // SAFETY: `U: Unsize<T>` and the underlying value `T` must not change
    // We could add a safe variant of this method with the `Unsize` trait
    pub(super) unsafe fn new_unchecked<U>(inner: &'v T) -> Self
    where
        U: 'static,
        T: 'static,
    {
        Erased {
            type_id: TypeId::of::<U>(),
            inner,
        }
    }

    pub(super) fn get(self) -> &'v T {
        self.inner
    }

    // SAFETY: The underlying type of `T` is `U`
    pub(super) unsafe fn downcast_unchecked<U>(self) -> &'v U {
        &*(self.inner as *const T as *const U)
    }
}
