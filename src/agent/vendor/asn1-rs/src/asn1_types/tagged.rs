use crate::{Class, Error, Tag, Tagged};
use core::marker::PhantomData;

mod application;
mod builder;
mod explicit;
mod helpers;
mod implicit;
mod optional;
mod parser;
mod private;

pub use application::*;
pub use builder::*;
pub use explicit::*;
pub use helpers::*;
pub use implicit::*;
pub use optional::*;
pub use parser::*;
pub use private::*;

pub(crate) const CONTEXT_SPECIFIC: u8 = Class::ContextSpecific as u8;

/// A type parameter for `IMPLICIT` tagged values.
#[derive(Debug, PartialEq, Eq)]
pub enum Implicit {}

/// A type parameter for `EXPLICIT` tagged values.
#[derive(Debug, PartialEq, Eq)]
pub enum Explicit {}

/// A type parameter for tagged values either [`Explicit`] or [`Implicit`].
pub trait TagKind {}

impl TagKind for Implicit {}
impl TagKind for Explicit {}

/// Helper object for creating `FromBer`/`FromDer` types for TAGGED OPTIONAL types
///
/// When parsing `ContextSpecific` (the most common class), see [`TaggedExplicit`] and
/// [`TaggedImplicit`] alias types.
///
/// # Notes
///
/// `CLASS` must be between 0 and 4. See [`Class`] for possible values for the `CLASS` parameter.
/// Constants from this class can be used, but they must be wrapped in braces due to
/// [Rust syntax for generics](https://doc.rust-lang.org/reference/items/generics.html)
/// (see example below).
///
/// # Examples
///
/// To parse a `[APPLICATION 0] EXPLICIT INTEGER` object:
///
/// ```rust
/// use asn1_rs::{Class, Error, Explicit, FromBer, Integer, TaggedValue};
///
/// let bytes = &[0x60, 0x03, 0x2, 0x1, 0x2];
///
/// // If tagged object is present (and has expected tag), parsing succeeds:
/// let (_, tagged) =
///     TaggedValue::<Integer, Error, Explicit, {Class::APPLICATION}, 0>::from_ber(bytes)
///         .unwrap();
/// assert_eq!(tagged, TaggedValue::explicit(Integer::from(2)));
/// ```
#[derive(Debug, PartialEq)]
pub struct TaggedValue<T, E, TagKind, const CLASS: u8, const TAG: u32> {
    pub(crate) inner: T,

    tag_kind: PhantomData<TagKind>,
    _e: PhantomData<E>,
}

impl<T, E, TagKind, const CLASS: u8, const TAG: u32> TaggedValue<T, E, TagKind, CLASS, TAG> {
    /// Consumes the `TaggedParser`, returning the wrapped value.
    #[inline]
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Return the (outer) tag of this object
    pub const fn tag(&self) -> Tag {
        Self::TAG
    }

    /// Return the (outer) class of this object
    #[inline]
    pub const fn class(&self) -> u8 {
        CLASS
    }
}

impl<T, E, const CLASS: u8, const TAG: u32> TaggedValue<T, E, Explicit, CLASS, TAG> {
    /// Constructs a new `EXPLICIT TaggedParser` with the provided value
    #[inline]
    pub const fn explicit(inner: T) -> Self {
        TaggedValue {
            inner,
            tag_kind: PhantomData,
            _e: PhantomData,
        }
    }
}

impl<T, E, const CLASS: u8, const TAG: u32> TaggedValue<T, E, Implicit, CLASS, TAG> {
    /// Constructs a new `IMPLICIT TaggedParser` with the provided value
    #[inline]
    pub const fn implicit(inner: T) -> Self {
        TaggedValue {
            inner,
            tag_kind: PhantomData,
            _e: PhantomData,
        }
    }
}

impl<T, E, TagKind, const CLASS: u8, const TAG: u32> AsRef<T>
    for TaggedValue<T, E, TagKind, CLASS, TAG>
{
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<T, E, TagKind, const CLASS: u8, const TAG: u32> Tagged
    for TaggedValue<T, E, TagKind, CLASS, TAG>
{
    const TAG: Tag = Tag(TAG);
}
