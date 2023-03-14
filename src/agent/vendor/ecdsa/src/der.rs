//! Support for ECDSA signatures encoded as ASN.1 DER.

use crate::Error;
use core::{
    convert::{TryFrom, TryInto},
    fmt,
    ops::{Add, Range},
};
use der::Decodable;
use elliptic_curve::{
    consts::U9,
    generic_array::{typenum::NonZero, ArrayLength, GenericArray},
    weierstrass::Curve,
    Order,
};

#[cfg(feature = "alloc")]
use alloc::boxed::Box;

/// Maximum overhead of an ASN.1 DER-encoded ECDSA signature for a given curve:
/// 9-bytes.
///
/// Includes 3-byte ASN.1 DER header:
///
/// - 1-byte: ASN.1 `SEQUENCE` tag (0x30)
/// - 2-byte: length
///
/// ...followed by two ASN.1 `INTEGER` values, which each have a header whose
/// maximum length is the following:
///
/// - 1-byte: ASN.1 `INTEGER` tag (0x02)
/// - 1-byte: length
/// - 1-byte: zero to indicate value is positive (`INTEGER` is signed)
pub type MaxOverhead = U9;

/// Maximum size of an ASN.1 DER encoded signature for the given elliptic curve.
pub type MaxSize<C> =
    <<<C as elliptic_curve::Curve>::FieldSize as Add>::Output as Add<MaxOverhead>>::Output;

/// Byte array containing a serialized ASN.1 signature
type SignatureBytes<C> = GenericArray<u8, MaxSize<C>>;

/// Big integer type containing an `r` or `s` scalar
type RawScalar<'a, C> = der::BigUInt<'a, <C as elliptic_curve::Curve>::FieldSize>;

/// Error message to display if encoding fails
const ENCODING_ERR_MSG: &str = "DER encoding error";

/// ASN.1 DER-encoded signature.
///
/// Generic over the scalar size of the elliptic curve.
pub struct Signature<C>
where
    C: Curve + Order,
    C::FieldSize: Add + ArrayLength<u8> + NonZero,
    MaxSize<C>: ArrayLength<u8>,
    <C::FieldSize as Add>::Output: Add<MaxOverhead> + ArrayLength<u8>,
{
    /// ASN.1 DER-encoded signature data
    bytes: SignatureBytes<C>,

    /// Range of the `r` value within the signature
    r_range: Range<usize>,

    /// Range of the `s` value within the signature
    s_range: Range<usize>,
}

impl<C> signature::Signature for Signature<C>
where
    C: Curve + Order,
    C::FieldSize: Add + ArrayLength<u8> + NonZero,
    MaxSize<C>: ArrayLength<u8>,
    <C::FieldSize as Add>::Output: Add<MaxOverhead> + ArrayLength<u8>,
{
    /// Parse an ASN.1 DER-encoded ECDSA signature from a byte slice
    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        bytes.try_into()
    }
}

#[allow(clippy::len_without_is_empty)]
impl<C> Signature<C>
where
    C: Curve + Order,
    C::FieldSize: Add + ArrayLength<u8> + NonZero,
    MaxSize<C>: ArrayLength<u8>,
    <C::FieldSize as Add>::Output: Add<MaxOverhead> + ArrayLength<u8>,
{
    /// Get the length of the signature in bytes
    pub fn len(&self) -> usize {
        self.s_range.end
    }

    /// Borrow this signature as a byte slice
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes.as_slice()[..self.len()]
    }

    /// Serialize this signature as a boxed byte slice
    #[cfg(feature = "alloc")]
    pub fn to_bytes(&self) -> Box<[u8]> {
        self.as_bytes().to_vec().into_boxed_slice()
    }

    /// Create an ASN.1 DER encoded signature from big endian `r` and `s` scalars
    pub(crate) fn from_scalar_bytes(r: &[u8], s: &[u8]) -> Self {
        let r = RawScalar::<C>::new(r).expect(ENCODING_ERR_MSG);
        let s = RawScalar::<C>::new(s).expect(ENCODING_ERR_MSG);

        let mut bytes = SignatureBytes::<C>::default();
        let mut encoder = der::Encoder::new(&mut bytes);

        encoder.message(&[&r, &s]).expect(ENCODING_ERR_MSG);

        encoder
            .finish()
            .expect(ENCODING_ERR_MSG)
            .try_into()
            .expect(ENCODING_ERR_MSG)
    }

    /// Get the `r` component of the signature (leading zeros removed)
    pub(crate) fn r(&self) -> &[u8] {
        &self.bytes[self.r_range.clone()]
    }

    /// Get the `s` component of the signature (leading zeros removed)
    pub(crate) fn s(&self) -> &[u8] {
        &self.bytes[self.s_range.clone()]
    }
}

impl<C> AsRef<[u8]> for Signature<C>
where
    C: Curve + Order,
    C::FieldSize: Add + ArrayLength<u8> + NonZero,
    MaxSize<C>: ArrayLength<u8>,
    <C::FieldSize as Add>::Output: Add<MaxOverhead> + ArrayLength<u8>,
{
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<C> fmt::Debug for Signature<C>
where
    C: Curve + Order,
    C::FieldSize: Add + ArrayLength<u8> + NonZero,
    MaxSize<C>: ArrayLength<u8>,
    <C::FieldSize as Add>::Output: Add<MaxOverhead> + ArrayLength<u8>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("asn1::Signature")
            .field("r", &self.r())
            .field("s", &self.s())
            .finish()
    }
}

impl<C> TryFrom<&[u8]> for Signature<C>
where
    C: Curve + Order,
    C::FieldSize: Add + ArrayLength<u8> + NonZero,
    MaxSize<C>: ArrayLength<u8>,
    <C::FieldSize as Add>::Output: Add<MaxOverhead> + ArrayLength<u8>,
{
    type Error = Error;

    fn try_from(input: &[u8]) -> Result<Self, Error> {
        let (r, s) = der::Decoder::new(input)
            .sequence(|decoder| {
                let r = RawScalar::<C>::decode(decoder)?;
                let s = RawScalar::<C>::decode(decoder)?;
                Ok((r, s))
            })
            .map_err(|_| Error::new())?;

        let r_range = find_scalar_range(input, r.as_bytes())?;
        let s_range = find_scalar_range(input, s.as_bytes())?;

        if s_range.end != input.len() {
            return Err(Error::new());
        }

        let mut bytes = SignatureBytes::<C>::default();
        bytes[..s_range.end].copy_from_slice(input);

        Ok(Signature {
            bytes,
            r_range,
            s_range,
        })
    }
}

/// Locate the range within a slice at which a particular subslice is located
fn find_scalar_range(outer: &[u8], inner: &[u8]) -> Result<Range<usize>, Error> {
    let outer_start = outer.as_ptr() as usize;
    let inner_start = inner.as_ptr() as usize;
    let start = inner_start
        .checked_sub(outer_start)
        .ok_or_else(Error::new)?;
    let end = start.checked_add(inner.len()).ok_or_else(Error::new)?;
    Ok(Range { start, end })
}

#[cfg(all(feature = "digest", feature = "hazmat"))]
impl<C> signature::PrehashSignature for Signature<C>
where
    C: Curve + crate::hazmat::DigestPrimitive,
    C::FieldSize: Add + ArrayLength<u8> + NonZero,
    MaxSize<C>: ArrayLength<u8>,
    <C::FieldSize as Add>::Output: Add<MaxOverhead> + ArrayLength<u8>,
{
    type Digest = C::Digest;
}

#[cfg(all(test, feature = "arithmetic"))]
mod tests {
    use elliptic_curve::dev::MockCurve;
    use signature::Signature as _;

    type Signature = crate::Signature<MockCurve>;

    const EXAMPLE_SIGNATURE: [u8; 64] = [
        0xf3, 0xac, 0x80, 0x61, 0xb5, 0x14, 0x79, 0x5b, 0x88, 0x43, 0xe3, 0xd6, 0x62, 0x95, 0x27,
        0xed, 0x2a, 0xfd, 0x6b, 0x1f, 0x6a, 0x55, 0x5a, 0x7a, 0xca, 0xbb, 0x5e, 0x6f, 0x79, 0xc8,
        0xc2, 0xac, 0x8b, 0xf7, 0x78, 0x19, 0xca, 0x5, 0xa6, 0xb2, 0x78, 0x6c, 0x76, 0x26, 0x2b,
        0xf7, 0x37, 0x1c, 0xef, 0x97, 0xb2, 0x18, 0xe9, 0x6f, 0x17, 0x5a, 0x3c, 0xcd, 0xda, 0x2a,
        0xcc, 0x5, 0x89, 0x3,
    ];

    #[test]
    fn test_fixed_to_asn1_signature_roundtrip() {
        let signature1 = Signature::from_bytes(&EXAMPLE_SIGNATURE).unwrap();

        // Convert to ASN.1 DER and back
        let asn1_signature = signature1.to_der();
        let signature2 = Signature::from_der(asn1_signature.as_ref()).unwrap();

        assert_eq!(signature1, signature2);
    }

    #[test]
    fn test_asn1_too_short_signature() {
        assert!(Signature::from_der(&[]).is_err());
        assert!(Signature::from_der(&[der::Tag::Sequence as u8]).is_err());
        assert!(Signature::from_der(&[der::Tag::Sequence as u8, 0x00]).is_err());
        assert!(Signature::from_der(&[
            der::Tag::Sequence as u8,
            0x03,
            der::Tag::Integer as u8,
            0x01,
            0x01
        ])
        .is_err());
    }

    #[test]
    fn test_asn1_non_der_signature() {
        // A minimal 8-byte ASN.1 signature parses OK.
        assert!(Signature::from_der(&[
            der::Tag::Sequence as u8,
            0x06, // length of below
            der::Tag::Integer as u8,
            0x01, // length of value
            0x01, // value=1
            der::Tag::Integer as u8,
            0x01, // length of value
            0x01, // value=1
        ])
        .is_ok());

        // But length fields that are not minimally encoded should be rejected, as they are not
        // valid DER, cf.
        // https://github.com/google/wycheproof/blob/2196000605e4/testvectors/ecdsa_secp256k1_sha256_test.json#L57-L66
        assert!(Signature::from_der(&[
            der::Tag::Sequence as u8,
            0x81, // extended length: 1 length byte to come
            0x06, // length of below
            der::Tag::Integer as u8,
            0x01, // length of value
            0x01, // value=1
            der::Tag::Integer as u8,
            0x01, // length of value
            0x01, // value=1
        ])
        .is_err());
    }
}
