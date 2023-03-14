use crate::{Class, Explicit, Implicit, TaggedValue};

/// A helper object to parse `[APPLICATION n] EXPLICIT T`
///
/// A helper object implementing [`FromBer`](crate::FromBer) and [`FromDer`](crate::FromDer), to
/// parse explicit application-tagged values.
///
/// # Examples
///
/// To parse a `[APPLICATION 0] EXPLICIT INTEGER` object:
///
/// ```rust
/// use asn1_rs::{ApplicationExplicit, Error, FromBer, Integer, TaggedValue};
///
/// let bytes = &[0x60, 0x03, 0x2, 0x1, 0x2];
///
/// // If tagged object is present (and has expected tag), parsing succeeds:
/// let (_, tagged) = ApplicationExplicit::<Integer, Error, 0>::from_ber(bytes).unwrap();
/// assert_eq!(tagged, TaggedValue::explicit(Integer::from(2)));
/// ```
pub type ApplicationExplicit<T, E, const TAG: u32> =
    TaggedValue<T, E, Explicit, { Class::APPLICATION }, TAG>;

/// A helper object to parse `[APPLICATION n] IMPLICIT T`
///
/// A helper object implementing [`FromBer`](crate::FromBer) and [`FromDer`](crate::FromDer), to
/// parse explicit application-tagged values.
///
/// # Examples
///
/// To parse a `[APPLICATION 0] IMPLICIT INTEGER` object:
///
/// ```rust
/// use asn1_rs::{ApplicationImplicit, Error, FromBer, Integer, TaggedValue};
///
/// let bytes = &[0x60, 0x1, 0x2];
///
/// let (_, tagged) = ApplicationImplicit::<Integer, Error, 0>::from_ber(bytes).unwrap();
/// assert_eq!(tagged, TaggedValue::implicit(Integer::from(2_u8)));
/// ```
pub type ApplicationImplicit<T, E, const TAG: u32> =
    TaggedValue<T, E, Implicit, { Class::APPLICATION }, TAG>;
