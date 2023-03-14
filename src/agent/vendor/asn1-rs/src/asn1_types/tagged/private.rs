use crate::{Class, Explicit, Implicit, TaggedValue};

/// A helper object to parse `[PRIVATE n] EXPLICIT T`
///
/// A helper object implementing [`FromBer`](crate::FromBer) and [`FromDer`](crate::FromDer), to
/// parse explicit private-tagged values.
///
/// # Examples
///
/// To parse a `[PRIVATE 0] EXPLICIT INTEGER` object:
///
/// ```rust
/// use asn1_rs::{Error, FromBer, Integer, PrivateExplicit, TaggedValue};
///
/// let bytes = &[0xe0, 0x03, 0x2, 0x1, 0x2];
///
/// // If tagged object is present (and has expected tag), parsing succeeds:
/// let (_, tagged) = PrivateExplicit::<Integer, Error, 0>::from_ber(bytes).unwrap();
/// assert_eq!(tagged, TaggedValue::explicit(Integer::from(2)));
/// ```
pub type PrivateExplicit<T, E, const TAG: u32> =
    TaggedValue<T, E, Explicit, { Class::PRIVATE }, TAG>;

/// A helper object to parse `[PRIVATE n] IMPLICIT T`
///
/// A helper object implementing [`FromBer`](crate::FromBer) and [`FromDer`](crate::FromDer), to
/// parse implicit private-tagged values.
///
/// # Examples
///
/// To parse a `[PRIVATE 0] IMPLICIT INTEGER` object:
///
/// ```rust
/// use asn1_rs::{Error, FromBer, Integer, PrivateImplicit, TaggedValue};
///
/// let bytes = &[0xe0, 0x1, 0x2];
///
/// let (_, tagged) = PrivateImplicit::<Integer, Error, 0>::from_ber(bytes).unwrap();
/// assert_eq!(tagged, TaggedValue::implicit(Integer::from(2_u8)));
/// ```
pub type PrivateImplicit<T, E, const TAG: u32> =
    TaggedValue<T, E, Implicit, { Class::PRIVATE }, TAG>;
