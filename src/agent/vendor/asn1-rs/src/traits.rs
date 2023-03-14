use crate::error::*;
use crate::{Any, Class, Explicit, Implicit, Tag, TaggedParser};
use core::convert::{TryFrom, TryInto};
#[cfg(feature = "std")]
use std::io::Write;

/// Phantom type representing a BER parser
#[doc(hidden)]
#[derive(Debug)]
pub enum BerParser {}

/// Phantom type representing a DER parser
#[doc(hidden)]
#[derive(Debug)]
pub enum DerParser {}

#[doc(hidden)]
pub trait ASN1Parser {}

impl ASN1Parser for BerParser {}
impl ASN1Parser for DerParser {}

pub trait Tagged {
    const TAG: Tag;
}

impl<T> Tagged for &'_ T
where
    T: Tagged,
{
    const TAG: Tag = T::TAG;
}

pub trait DynTagged {
    fn tag(&self) -> Tag;
}

impl<T> DynTagged for T
where
    T: Tagged,
{
    fn tag(&self) -> Tag {
        T::TAG
    }
}

/// Base trait for BER object parsers
///
/// Library authors should usually not directly implement this trait, but should prefer implementing the
/// [`TryFrom<Any>`] trait,
/// which offers greater flexibility and provides an equivalent `FromBer` implementation for free.
///
/// # Examples
///
/// ```
/// use asn1_rs::{Any, Result, Tag};
/// use std::convert::TryFrom;
///
/// // The type to be decoded
/// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// pub struct MyType(pub u32);
///
/// impl<'a> TryFrom<Any<'a>> for MyType {
///     type Error = asn1_rs::Error;
///
///     fn try_from(any: Any<'a>) -> Result<MyType> {
///         any.tag().assert_eq(Tag::Integer)?;
///         // for this fictive example, the type contains the number of characters
///         let n = any.data.len() as u32;
///         Ok(MyType(n))
///     }
/// }
///
/// // The above code provides a `FromBer` implementation for free.
///
/// // Example of parsing code:
/// use asn1_rs::FromBer;
///
/// let input = &[2, 1, 2];
/// // Objects can be parsed using `from_ber`, which returns the remaining bytes
/// // and the parsed object:
/// let (rem, my_type) = MyType::from_ber(input).expect("parsing failed");
/// ```
pub trait FromBer<'a, E = Error>: Sized {
    /// Attempt to parse input bytes into a BER object
    fn from_ber(bytes: &'a [u8]) -> ParseResult<Self, E>;
}

impl<'a, T, E> FromBer<'a, E> for T
where
    T: TryFrom<Any<'a>, Error = E>,
    E: From<Error>,
{
    fn from_ber(bytes: &'a [u8]) -> ParseResult<T, E> {
        let (i, any) = Any::from_ber(bytes).map_err(nom::Err::convert)?;
        let result = any.try_into().map_err(nom::Err::Error)?;
        Ok((i, result))
    }
}

/// Base trait for DER object parsers
///
/// Library authors should usually not directly implement this trait, but should prefer implementing the
/// [`TryFrom<Any>`] + [`CheckDerConstraints`] traits,
/// which offers greater flexibility and provides an equivalent `FromDer` implementation for free
/// (in fact, it provides both [`FromBer`] and `FromDer`).
///
/// Note: if you already implemented [`TryFrom<Any>`] and [`CheckDerConstraints`],
/// you can get a free [`FromDer`] implementation by implementing the
/// [`DerAutoDerive`] trait. This is not automatic, so it is also possible to manually
/// implement [`FromDer`] if preferred.
///
/// # Examples
///
/// ```
/// use asn1_rs::{Any, CheckDerConstraints, DerAutoDerive, Result, Tag};
/// use std::convert::TryFrom;
///
/// // The type to be decoded
/// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// pub struct MyType(pub u32);
///
/// impl<'a> TryFrom<Any<'a>> for MyType {
///     type Error = asn1_rs::Error;
///
///     fn try_from(any: Any<'a>) -> Result<MyType> {
///         any.tag().assert_eq(Tag::Integer)?;
///         // for this fictive example, the type contains the number of characters
///         let n = any.data.len() as u32;
///         Ok(MyType(n))
///     }
/// }
///
/// impl CheckDerConstraints for MyType {
///     fn check_constraints(any: &Any) -> Result<()> {
///         any.header.assert_primitive()?;
///         Ok(())
///     }
/// }
///
/// impl DerAutoDerive for MyType {}
///
/// // The above code provides a `FromDer` implementation for free.
///
/// // Example of parsing code:
/// use asn1_rs::FromDer;
///
/// let input = &[2, 1, 2];
/// // Objects can be parsed using `from_der`, which returns the remaining bytes
/// // and the parsed object:
/// let (rem, my_type) = MyType::from_der(input).expect("parsing failed");
/// ```
pub trait FromDer<'a, E = Error>: Sized {
    /// Attempt to parse input bytes into a DER object (enforcing constraints)
    fn from_der(bytes: &'a [u8]) -> ParseResult<Self, E>;
}

/// Trait to automatically derive `FromDer`
///
/// This trait is only a marker to control if a DER parser should be automatically derived. It is
/// empty.
///
/// This trait is used in combination with others:
/// after implementing [`TryFrom<Any>`] and [`CheckDerConstraints`] for a type,
/// a free [`FromDer`] implementation is provided by implementing the
/// [`DerAutoDerive`] trait. This is the most common case.
///
/// However, this is not automatic so it is also possible to manually
/// implement [`FromDer`] if preferred.
/// Manual implementation is generally only needed for generic containers (for ex. `Vec<T>`),
/// because the default implementation adds a constraint on `T` to implement also `TryFrom<Any>`
/// and `CheckDerConstraints`. This is problematic when `T` only provides `FromDer`, and can be
/// solved by providing a manual implementation of [`FromDer`].
pub trait DerAutoDerive {}

impl<'a, T, E> FromDer<'a, E> for T
where
    T: TryFrom<Any<'a>, Error = E>,
    T: CheckDerConstraints,
    T: DerAutoDerive,
    E: From<Error>,
{
    fn from_der(bytes: &'a [u8]) -> ParseResult<T, E> {
        // Note: Any::from_der checks than length is definite
        let (i, any) = Any::from_der(bytes).map_err(nom::Err::convert)?;
        <T as CheckDerConstraints>::check_constraints(&any)
            .map_err(|e| nom::Err::Error(e.into()))?;
        let result = any.try_into().map_err(nom::Err::Error)?;
        Ok((i, result))
    }
}

/// Verification of DER constraints
pub trait CheckDerConstraints {
    fn check_constraints(any: &Any) -> Result<()>;
}

/// Common trait for all objects that can be encoded using the DER representation
///
/// # Examples
///
/// Objects from this crate can be encoded as DER:
///
/// ```
/// use asn1_rs::{Integer, ToDer};
///
/// let int = Integer::from(4u32);
/// let mut writer = Vec::new();
/// let sz = int.write_der(&mut writer).expect("serialization failed");
///
/// assert_eq!(&writer, &[0x02, 0x01, 0x04]);
/// # assert_eq!(sz, 3);
/// ```
///
/// Many of the primitive types can also directly be encoded as DER:
///
/// ```
/// use asn1_rs::ToDer;
///
/// let mut writer = Vec::new();
/// let sz = 4.write_der(&mut writer).expect("serialization failed");
///
/// assert_eq!(&writer, &[0x02, 0x01, 0x04]);
/// # assert_eq!(sz, 3);
/// ```
#[cfg(feature = "std")]
pub trait ToDer
where
    Self: DynTagged,
{
    /// Get the length of the object, when encoded
    ///
    // Since we are using DER, length cannot be Indefinite, so we can use `usize`.
    // XXX can this function fail?
    fn to_der_len(&self) -> Result<usize>;

    /// Write the DER encoded representation to a newly allocated `Vec<u8>`.
    fn to_der_vec(&self) -> SerializeResult<Vec<u8>> {
        let mut v = Vec::new();
        let _ = self.write_der(&mut v)?;
        Ok(v)
    }

    /// Similar to using `to_vec`, but uses provided values without changes.
    /// This can generate an invalid encoding for a DER object.
    fn to_der_vec_raw(&self) -> SerializeResult<Vec<u8>> {
        let mut v = Vec::new();
        let _ = self.write_der_raw(&mut v)?;
        Ok(v)
    }

    /// Attempt to write the DER encoded representation (header and content) into this writer.
    ///
    /// # Examples
    ///
    /// ```
    /// use asn1_rs::{Integer, ToDer};
    ///
    /// let int = Integer::from(4u32);
    /// let mut writer = Vec::new();
    /// let sz = int.write_der(&mut writer).expect("serialization failed");
    ///
    /// assert_eq!(&writer, &[0x02, 0x01, 0x04]);
    /// # assert_eq!(sz, 3);
    /// ```
    fn write_der(&self, writer: &mut dyn Write) -> SerializeResult<usize> {
        let sz = self.write_der_header(writer)?;
        let sz = sz + self.write_der_content(writer)?;
        Ok(sz)
    }

    /// Attempt to write the DER header to this writer.
    fn write_der_header(&self, writer: &mut dyn Write) -> SerializeResult<usize>;

    /// Attempt to write the DER content (all except header) to this writer.
    fn write_der_content(&self, writer: &mut dyn Write) -> SerializeResult<usize>;

    /// Similar to using `to_der`, but uses provided values without changes.
    /// This can generate an invalid encoding for a DER object.
    fn write_der_raw(&self, writer: &mut dyn Write) -> SerializeResult<usize> {
        self.write_der(writer)
    }
}

#[cfg(feature = "std")]
impl<'a, T> ToDer for &'a T
where
    T: ToDer,
    &'a T: DynTagged,
{
    fn to_der_len(&self) -> Result<usize> {
        (*self).to_der_len()
    }

    fn write_der_header(&self, writer: &mut dyn Write) -> SerializeResult<usize> {
        (*self).write_der_header(writer)
    }

    fn write_der_content(&self, writer: &mut dyn Write) -> SerializeResult<usize> {
        (*self).write_der_content(writer)
    }
}

/// Helper trait for creating tagged EXPLICIT values
///
/// # Examples
///
/// ```
/// use asn1_rs::{AsTaggedExplicit, Class, Error, TaggedParser};
///
/// // create a `[1] EXPLICIT INTEGER` value
/// let tagged: TaggedParser<_, _, Error> = 4u32.explicit(Class::ContextSpecific, 1);
/// ```
pub trait AsTaggedExplicit<'a, E = Error>: Sized {
    fn explicit(self, class: Class, tag: u32) -> TaggedParser<'a, Explicit, Self, E> {
        TaggedParser::new_explicit(class, tag, self)
    }
}

impl<'a, T, E> AsTaggedExplicit<'a, E> for T where T: Sized + 'a {}

/// Helper trait for creating tagged IMPLICIT values
///
/// # Examples
///
/// ```
/// use asn1_rs::{AsTaggedImplicit, Class, Error, TaggedParser};
///
/// // create a `[1] IMPLICIT INTEGER` value, not constructed
/// let tagged: TaggedParser<_, _, Error> = 4u32.implicit(Class::ContextSpecific, false, 1);
/// ```
pub trait AsTaggedImplicit<'a, E = Error>: Sized {
    fn implicit(
        self,
        class: Class,
        constructed: bool,
        tag: u32,
    ) -> TaggedParser<'a, Implicit, Self, E> {
        TaggedParser::new_implicit(class, constructed, tag, self)
    }
}

impl<'a, T, E> AsTaggedImplicit<'a, E> for T where T: Sized + 'a {}

pub trait ToStatic {
    type Owned: 'static;
    fn to_static(&self) -> Self::Owned;
}
