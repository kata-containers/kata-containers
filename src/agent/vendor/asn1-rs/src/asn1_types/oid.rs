use crate::*;
use alloc::borrow::Cow;
#[cfg(not(feature = "std"))]
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::{
    convert::TryFrom, fmt, iter::FusedIterator, marker::PhantomData, ops::Shl, str::FromStr,
};

#[cfg(feature = "bigint")]
use num_bigint::BigUint;
use num_traits::Num;

/// An error for OID parsing functions.
#[derive(Debug)]
pub enum OidParseError {
    TooShort,
    /// Signalizes that the first or second component is too large.
    /// The first must be within the range 0 to 6 (inclusive).
    /// The second component must be less than 40.
    FirstComponentsTooLarge,
    ParseIntError,
}

/// Object ID (OID) representation which can be relative or non-relative.
/// An example for an OID in string representation is `"1.2.840.113549.1.1.5"`.
///
/// For non-relative OIDs restrictions apply to the first two components.
///
/// This library contains a procedural macro `oid` which can be used to
/// create oids. For example `oid!(1.2.44.233)` or `oid!(rel 44.233)`
/// for relative oids. See the [module documentation](index.html) for more information.
#[derive(Hash, PartialEq, Eq, Clone)]

pub struct Oid<'a> {
    asn1: Cow<'a, [u8]>,
    relative: bool,
}

impl<'a> TryFrom<Any<'a>> for Oid<'a> {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<Self> {
        TryFrom::try_from(&any)
    }
}

impl<'a, 'b> TryFrom<&'b Any<'a>> for Oid<'a> {
    type Error = Error;

    fn try_from(any: &'b Any<'a>) -> Result<Self> {
        // check that any.data.last().unwrap() >> 7 == 0u8
        let asn1 = Cow::Borrowed(any.data);
        Ok(Oid::new(asn1))
    }
}

impl<'a> CheckDerConstraints for Oid<'a> {
    fn check_constraints(any: &Any) -> Result<()> {
        any.header.assert_primitive()?;
        any.header.length.assert_definite()?;
        Ok(())
    }
}

impl DerAutoDerive for Oid<'_> {}

impl<'a> Tagged for Oid<'a> {
    const TAG: Tag = Tag::Oid;
}

#[cfg(feature = "std")]
impl ToDer for Oid<'_> {
    fn to_der_len(&self) -> Result<usize> {
        // OID/REL-OID tag will not change header size, so we don't care here
        let header = Header::new(
            Class::Universal,
            false,
            Self::TAG,
            Length::Definite(self.asn1.len()),
        );
        Ok(header.to_der_len()? + self.asn1.len())
    }

    fn write_der_header(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        let tag = if self.relative {
            Tag::RelativeOid
        } else {
            Tag::Oid
        };
        let header = Header::new(
            Class::Universal,
            false,
            tag,
            Length::Definite(self.asn1.len()),
        );
        header.write_der_header(writer).map_err(Into::into)
    }

    fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        writer.write(&self.asn1).map_err(Into::into)
    }
}

fn encode_relative(ids: &'_ [u64]) -> impl Iterator<Item = u8> + '_ {
    ids.iter().flat_map(|id| {
        let bit_count = 64 - id.leading_zeros();
        let octets_needed = ((bit_count + 6) / 7).max(1);
        (0..octets_needed).map(move |i| {
            let flag = if i == octets_needed - 1 { 0 } else { 1 << 7 };
            ((id >> (7 * (octets_needed - 1 - i))) & 0b111_1111) as u8 | flag
        })
    })
}

impl<'a> Oid<'a> {
    /// Create an OID from the ASN.1 DER encoded form. See the [module documentation](index.html)
    /// for other ways to create oids.
    pub const fn new(asn1: Cow<'a, [u8]>) -> Oid {
        Oid {
            asn1,
            relative: false,
        }
    }

    /// Create a relative OID from the ASN.1 DER encoded form. See the [module documentation](index.html)
    /// for other ways to create relative oids.
    pub const fn new_relative(asn1: Cow<'a, [u8]>) -> Oid {
        Oid {
            asn1,
            relative: true,
        }
    }

    /// Build an OID from an array of object identifier components.
    /// This method allocates memory on the heap.
    pub fn from<'b>(s: &'b [u64]) -> core::result::Result<Oid<'static>, OidParseError> {
        if s.len() < 2 {
            if s.len() == 1 && s[0] == 0 {
                return Ok(Oid {
                    asn1: Cow::Borrowed(&[0]),
                    relative: false,
                });
            }
            return Err(OidParseError::TooShort);
        }
        if s[0] >= 7 || s[1] >= 40 {
            return Err(OidParseError::FirstComponentsTooLarge);
        }
        let asn1_encoded: Vec<u8> = [(s[0] * 40 + s[1]) as u8]
            .iter()
            .copied()
            .chain(encode_relative(&s[2..]))
            .collect();
        Ok(Oid {
            asn1: Cow::from(asn1_encoded),
            relative: false,
        })
    }

    /// Build a relative OID from an array of object identifier components.
    pub fn from_relative<'b>(s: &'b [u64]) -> core::result::Result<Oid<'static>, OidParseError> {
        if s.is_empty() {
            return Err(OidParseError::TooShort);
        }
        let asn1_encoded: Vec<u8> = encode_relative(s).collect();
        Ok(Oid {
            asn1: Cow::from(asn1_encoded),
            relative: true,
        })
    }

    /// Create a deep copy of the oid.
    ///
    /// This method allocates data on the heap. The returned oid
    /// can be used without keeping the ASN.1 representation around.
    ///
    /// Cloning the returned oid does again allocate data.
    pub fn to_owned(&self) -> Oid<'static> {
        Oid {
            asn1: Cow::from(self.asn1.to_vec()),
            relative: self.relative,
        }
    }

    /// Get the encoded oid without the header.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.asn1.as_ref()
    }

    /// Get the encoded oid without the header.
    #[deprecated(since = "0.2.0", note = "Use `as_bytes` instead")]
    #[inline]
    pub fn bytes(&self) -> &[u8] {
        self.as_bytes()
    }

    /// Get the bytes representation of the encoded oid
    pub fn into_cow(self) -> Cow<'a, [u8]> {
        self.asn1
    }

    /// Convert the OID to a string representation.
    /// The string contains the IDs separated by dots, for ex: "1.2.840.113549.1.1.5"
    #[cfg(feature = "bigint")]
    pub fn to_id_string(&self) -> String {
        let ints: Vec<String> = self.iter_bigint().map(|i| i.to_string()).collect();
        ints.join(".")
    }

    #[cfg(not(feature = "bigint"))]
    /// Convert the OID to a string representation.
    ///
    /// If every arc fits into a u64 a string like "1.2.840.113549.1.1.5"
    /// is returned, otherwise a hex representation.
    ///
    /// See also the "bigint" feature of this crate.
    pub fn to_id_string(&self) -> String {
        if let Some(arcs) = self.iter() {
            let ints: Vec<String> = arcs.map(|i| i.to_string()).collect();
            ints.join(".")
        } else {
            let mut ret = String::with_capacity(self.asn1.len() * 3);
            for (i, o) in self.asn1.iter().enumerate() {
                ret.push_str(&format!("{:02x}", o));
                if i + 1 != self.asn1.len() {
                    ret.push(' ');
                }
            }
            ret
        }
    }

    /// Return an iterator over the sub-identifiers (arcs).
    #[cfg(feature = "bigint")]
    pub fn iter_bigint(
        &'_ self,
    ) -> impl Iterator<Item = BigUint> + FusedIterator + ExactSizeIterator + '_ {
        SubIdentifierIterator {
            oid: self,
            pos: 0,
            first: false,
            n: PhantomData,
        }
    }

    /// Return an iterator over the sub-identifiers (arcs).
    /// Returns `None` if at least one arc does not fit into `u64`.
    pub fn iter(
        &'_ self,
    ) -> Option<impl Iterator<Item = u64> + FusedIterator + ExactSizeIterator + '_> {
        // Check that every arc fits into u64
        let bytes = if self.relative {
            &self.asn1
        } else if self.asn1.is_empty() {
            &[]
        } else {
            &self.asn1[1..]
        };
        let max_bits = bytes
            .iter()
            .fold((0usize, 0usize), |(max, cur), c| {
                let is_end = (c >> 7) == 0u8;
                if is_end {
                    (max.max(cur + 7), 0)
                } else {
                    (max, cur + 7)
                }
            })
            .0;
        if max_bits > 64 {
            return None;
        }

        Some(SubIdentifierIterator {
            oid: self,
            pos: 0,
            first: false,
            n: PhantomData,
        })
    }

    pub fn from_ber_relative(bytes: &'a [u8]) -> ParseResult<'a, Self> {
        let (rem, any) = Any::from_ber(bytes)?;
        any.header.assert_primitive()?;
        any.header.assert_tag(Tag::RelativeOid)?;
        let asn1 = Cow::Borrowed(any.data);
        Ok((rem, Oid::new_relative(asn1)))
    }

    pub fn from_der_relative(bytes: &'a [u8]) -> ParseResult<'a, Self> {
        let (rem, any) = Any::from_der(bytes)?;
        any.header.assert_tag(Tag::RelativeOid)?;
        Self::check_constraints(&any)?;
        let asn1 = Cow::Borrowed(any.data);
        Ok((rem, Oid::new_relative(asn1)))
    }

    /// Returns true if `needle` is a prefix of the OID.
    pub fn starts_with(&self, needle: &Oid) -> bool {
        self.asn1.len() >= needle.asn1.len() && self.asn1.starts_with(needle.as_bytes())
    }
}

trait Repr: Num + Shl<usize, Output = Self> + From<u8> {}
impl<N> Repr for N where N: Num + Shl<usize, Output = N> + From<u8> {}

struct SubIdentifierIterator<'a, N: Repr> {
    oid: &'a Oid<'a>,
    pos: usize,
    first: bool,
    n: PhantomData<&'a N>,
}

impl<'a, N: Repr> Iterator for SubIdentifierIterator<'a, N> {
    type Item = N;

    fn next(&mut self) -> Option<Self::Item> {
        use num_traits::identities::Zero;

        if self.pos == self.oid.asn1.len() {
            return None;
        }
        if !self.oid.relative {
            if !self.first {
                debug_assert!(self.pos == 0);
                self.first = true;
                return Some((self.oid.asn1[0] / 40).into());
            } else if self.pos == 0 {
                self.pos += 1;
                if self.oid.asn1[0] == 0 && self.oid.asn1.len() == 1 {
                    return None;
                }
                return Some((self.oid.asn1[0] % 40).into());
            }
        }
        // decode objet sub-identifier according to the asn.1 standard
        let mut res = <N as Zero>::zero();
        for o in self.oid.asn1[self.pos..].iter() {
            self.pos += 1;
            res = (res << 7) + (o & 0b111_1111).into();
            let flag = o >> 7;
            if flag == 0u8 {
                break;
            }
        }
        Some(res)
    }
}

impl<'a, N: Repr> FusedIterator for SubIdentifierIterator<'a, N> {}

impl<'a, N: Repr> ExactSizeIterator for SubIdentifierIterator<'a, N> {
    fn len(&self) -> usize {
        if self.oid.relative {
            self.oid.asn1.iter().filter(|o| (*o >> 7) == 0u8).count()
        } else if self.oid.asn1.len() == 0 {
            0
        } else if self.oid.asn1.len() == 1 {
            if self.oid.asn1[0] == 0 {
                1
            } else {
                2
            }
        } else {
            2 + self.oid.asn1[2..]
                .iter()
                .filter(|o| (*o >> 7) == 0u8)
                .count()
        }
    }

    #[cfg(feature = "exact_size_is_empty")]
    fn is_empty(&self) -> bool {
        self.oid.asn1.is_empty()
    }
}

impl<'a> fmt::Display for Oid<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.relative {
            f.write_str("rel. ")?;
        }
        f.write_str(&self.to_id_string())
    }
}

impl<'a> fmt::Debug for Oid<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("OID(")?;
        <Oid as fmt::Display>::fmt(self, f)?;
        f.write_str(")")
    }
}

impl<'a> FromStr for Oid<'a> {
    type Err = OidParseError;

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        let v: core::result::Result<Vec<_>, _> = s.split('.').map(|c| c.parse::<u64>()).collect();
        v.map_err(|_| OidParseError::ParseIntError)
            .and_then(|v| Oid::from(&v))
    }
}

/// Helper macro to declare integers at compile-time
///
/// Since the DER encoded oids are not very readable we provide a
/// procedural macro `oid!`. The macro can be used the following ways:
///
/// - `oid!(1.4.42.23)`: Create a const expression for the corresponding `Oid<'static>`
/// - `oid!(rel 42.23)`: Create a const expression for the corresponding relative `Oid<'static>`
/// - `oid!(raw 1.4.42.23)`/`oid!(raw rel 42.23)`: Obtain the DER encoded form as a byte array.
///
/// # Comparing oids
///
/// Comparing a parsed oid to a static oid is probably the most common
/// thing done with oids in your code. The `oid!` macro can be used in expression positions for
/// this purpose. For example
/// ```
/// use asn1_rs::{oid, Oid};
///
/// # let some_oid: Oid<'static> = oid!(1.2.456);
/// const SOME_STATIC_OID: Oid<'static> = oid!(1.2.456);
/// assert_eq!(some_oid, SOME_STATIC_OID)
/// ```
/// To get a relative Oid use `oid!(rel 1.2)`.
///
/// Because of limitations for procedural macros ([rust issue](https://github.com/rust-lang/rust/issues/54727))
/// and constants used in patterns ([rust issue](https://github.com/rust-lang/rust/issues/31434))
/// the `oid` macro can not directly be used in patterns, also not through constants.
/// You can do this, though:
/// ```
/// # use asn1_rs::{oid, Oid};
/// # let some_oid: Oid<'static> = oid!(1.2.456);
/// const SOME_OID: Oid<'static> = oid!(1.2.456);
/// if some_oid == SOME_OID || some_oid == oid!(1.2.456) {
///     println!("match");
/// }
///
/// // Alternatively, compare the DER encoded form directly:
/// const SOME_OID_RAW: &[u8] = &oid!(raw 1.2.456);
/// match some_oid.as_bytes() {
///     SOME_OID_RAW => println!("match"),
///     _ => panic!("no match"),
/// }
/// ```
/// *Attention*, be aware that the latter version might not handle the case of a relative oid correctly. An
/// extra check might be necessary.
#[macro_export]
macro_rules! oid {
    (raw $items:expr) => {
        $crate::exports::asn1_rs_impl::encode_oid!($items)
    };
    (rel $items:expr) => {
        $crate::Oid::new_relative($crate::exports::borrow::Cow::Borrowed(
            &$crate::exports::asn1_rs_impl::encode_oid!(rel $items),
        ))
    };
    ($items:expr) => {
        $crate::Oid::new($crate::exports::borrow::Cow::Borrowed(
            &$crate::oid!(raw $items),
        ))
    };
}

#[cfg(test)]
mod tests {
    use crate::{FromDer, Oid, ToDer};
    use hex_literal::hex;

    #[test]
    fn declare_oid() {
        let oid = super::oid! {1.2.840.113549.1};
        assert_eq!(oid.to_string(), "1.2.840.113549.1");
    }

    const OID_RSA_ENCRYPTION: &[u8] = &oid! {raw 1.2.840.113549.1.1.1};
    const OID_EC_PUBLIC_KEY: &[u8] = &oid! {raw 1.2.840.10045.2.1};
    #[allow(clippy::match_like_matches_macro)]
    fn compare_oid(oid: &Oid) -> bool {
        match oid.as_bytes() {
            OID_RSA_ENCRYPTION => true,
            OID_EC_PUBLIC_KEY => true,
            _ => false,
        }
    }

    #[test]
    fn test_compare_oid() {
        let oid = Oid::from(&[1, 2, 840, 113_549, 1, 1, 1]).unwrap();
        assert_eq!(oid, oid! {1.2.840.113549.1.1.1});
        let oid = Oid::from(&[1, 2, 840, 113_549, 1, 1, 1]).unwrap();
        assert!(compare_oid(&oid));
    }

    #[test]
    fn oid_to_der() {
        let oid = super::oid! {1.2.840.113549.1};
        assert_eq!(oid.to_der_len(), Ok(9));
        let v = oid.to_der_vec().expect("could not serialize");
        assert_eq!(&v, &hex! {"06 07 2a 86 48 86 f7 0d 01"});
        let (_, oid2) = Oid::from_der(&v).expect("could not re-parse");
        assert_eq!(&oid, &oid2);
    }

    #[test]
    fn oid_starts_with() {
        const OID_RSA_ENCRYPTION: Oid = oid! {1.2.840.113549.1.1.1};
        const OID_EC_PUBLIC_KEY: Oid = oid! {1.2.840.10045.2.1};
        let oid = super::oid! {1.2.840.113549.1};
        assert!(OID_RSA_ENCRYPTION.starts_with(&oid));
        assert!(!OID_EC_PUBLIC_KEY.starts_with(&oid));
    }
}
