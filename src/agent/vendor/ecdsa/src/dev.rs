//! Development-related functionality.

use crate::hazmat::FromDigest;
use core::convert::{TryFrom, TryInto};
use elliptic_curve::{
    consts::U32,
    dev::{MockCurve, Scalar, MODULUS},
    util::{adc64, sbb64},
};
use signature::digest::Digest;

impl FromDigest<MockCurve> for Scalar {
    fn from_digest<D>(digest: D) -> Self
    where
        D: Digest<OutputSize = U32>,
    {
        let bytes = digest.finalize();

        sub_inner(
            u64::from_be_bytes(bytes[24..32].try_into().unwrap()),
            u64::from_be_bytes(bytes[16..24].try_into().unwrap()),
            u64::from_be_bytes(bytes[8..16].try_into().unwrap()),
            u64::from_be_bytes(bytes[0..8].try_into().unwrap()),
            0,
            MODULUS[0],
            MODULUS[1],
            MODULUS[2],
            MODULUS[3],
            0,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn sub_inner(
    l0: u64,
    l1: u64,
    l2: u64,
    l3: u64,
    l4: u64,
    r0: u64,
    r1: u64,
    r2: u64,
    r3: u64,
    r4: u64,
) -> Scalar {
    let (w0, borrow) = sbb64(l0, r0, 0);
    let (w1, borrow) = sbb64(l1, r1, borrow);
    let (w2, borrow) = sbb64(l2, r2, borrow);
    let (w3, borrow) = sbb64(l3, r3, borrow);
    let (_, borrow) = sbb64(l4, r4, borrow);

    let (w0, carry) = adc64(w0, MODULUS[0] & borrow, 0);
    let (w1, carry) = adc64(w1, MODULUS[1] & borrow, carry);
    let (w2, carry) = adc64(w2, MODULUS[2] & borrow, carry);
    let (w3, _) = adc64(w3, MODULUS[3] & borrow, carry);

    Scalar::try_from([w0, w1, w2, w3]).unwrap()
}

// TODO(tarcieri): implement full set of tests from ECDSA2VS
// <https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Algorithm-Validation-Program/documents/dss2/ecdsa2vs.pdf>

/// ECDSA test vector
pub struct TestVector {
    /// Private scalar
    pub d: &'static [u8],

    /// Public key x-coordinate (`Qx`)
    pub q_x: &'static [u8],

    /// Public key y-coordinate (`Qy`)
    pub q_y: &'static [u8],

    /// Ephemeral scalar (a.k.a. nonce)
    pub k: &'static [u8],

    /// Message digest (prehashed)
    pub m: &'static [u8],

    /// Signature `r` component
    pub r: &'static [u8],

    /// Signature `s` component
    pub s: &'static [u8],
}

/// Define ECDSA signing test.
#[macro_export]
#[cfg_attr(docsrs, doc(cfg(feature = "dev")))]
macro_rules! new_signing_test {
    ($curve:path, $vectors:expr) => {
        use core::convert::TryInto;
        use $crate::{
            elliptic_curve::{
                ff::PrimeField, generic_array::GenericArray, ProjectiveArithmetic, Scalar,
            },
            hazmat::SignPrimitive,
        };

        #[test]
        fn ecdsa_signing() {
            for vector in $vectors {
                let d = Scalar::<$curve>::from_repr(GenericArray::clone_from_slice(vector.d))
                    .expect("invalid vector.d");

                let k = Scalar::<$curve>::from_repr(GenericArray::clone_from_slice(vector.k))
                    .expect("invalid vector.m");

                let z = Scalar::<$curve>::from_repr(GenericArray::clone_from_slice(vector.m))
                    .expect("invalid vector.z");

                let sig = d.try_sign_prehashed(&k, &z).unwrap();

                assert_eq!(vector.r, sig.r().to_bytes().as_slice());
                assert_eq!(vector.s, sig.s().to_bytes().as_slice());
            }
        }
    };
}

/// Define ECDSA verification test.
#[macro_export]
#[cfg_attr(docsrs, doc(cfg(feature = "dev")))]
macro_rules! new_verification_test {
    ($curve:path, $vectors:expr) => {
        use core::convert::TryInto;
        use $crate::{
            elliptic_curve::{
                ff::PrimeField, generic_array::GenericArray, sec1::EncodedPoint, AffinePoint,
                ProjectiveArithmetic, Scalar,
            },
            hazmat::VerifyPrimitive,
            Signature,
        };

        #[test]
        fn ecdsa_verify_success() {
            for vector in $vectors {
                let q_encoded = EncodedPoint::from_affine_coordinates(
                    GenericArray::from_slice(vector.q_x),
                    GenericArray::from_slice(vector.q_y),
                    false,
                );

                let q: AffinePoint<$curve> = q_encoded.decode().unwrap();

                let z = Scalar::<$curve>::from_repr(GenericArray::clone_from_slice(vector.m))
                    .expect("invalid vector.m");

                let sig = Signature::from_scalars(
                    GenericArray::clone_from_slice(vector.r),
                    GenericArray::clone_from_slice(vector.s),
                )
                .unwrap();

                let result = q.verify_prehashed(&z, &sig);
                assert!(result.is_ok());
            }
        }

        #[test]
        fn ecdsa_verify_invalid_s() {
            for vector in $vectors {
                let q_encoded = EncodedPoint::from_affine_coordinates(
                    GenericArray::from_slice(vector.q_x),
                    GenericArray::from_slice(vector.q_y),
                    false,
                );

                let q: AffinePoint<$curve> = q_encoded.decode().unwrap();

                let z = Scalar::<$curve>::from_repr(GenericArray::clone_from_slice(vector.m))
                    .expect("invalid vector.m");

                // Flip a bit in `s`
                let mut s_tweaked = GenericArray::clone_from_slice(vector.s);
                s_tweaked[0] ^= 1;

                let sig =
                    Signature::from_scalars(GenericArray::clone_from_slice(vector.r), s_tweaked)
                        .unwrap();

                let result = q.verify_prehashed(&z, &sig);
                assert!(result.is_err());
            }
        }

        // TODO(tarcieri): test invalid Q, invalid r, invalid m
    };
}

/// Define a Wycheproof verification test.
#[macro_export]
#[cfg_attr(docsrs, doc(cfg(feature = "dev")))]
macro_rules! new_wycheproof_test {
    ($name:ident, $test_name: expr, $curve:path) => {
        use $crate::{elliptic_curve::sec1::EncodedPoint, signature::Verifier, Signature};

        #[test]
        fn $name() {
            use blobby::Blob5Iterator;
            use elliptic_curve::generic_array::typenum::Unsigned;

            // Build a field element but allow for too-short input (left pad with zeros)
            // or too-long input (check excess leftmost bytes are zeros).
            fn element_from_padded_slice<C: elliptic_curve::Curve>(
                data: &[u8],
            ) -> elliptic_curve::FieldBytes<C> {
                let point_len = C::FieldSize::to_usize();
                if data.len() >= point_len {
                    let offset = data.len() - point_len;
                    for v in data.iter().take(offset) {
                        assert_eq!(*v, 0, "EcdsaVerifier: point too large");
                    }
                    elliptic_curve::FieldBytes::<C>::clone_from_slice(&data[offset..])
                } else {
                    // Provided slice is too short and needs to be padded with zeros
                    // on the left.  Build a combined exact iterator to do this.
                    let iter = core::iter::repeat(0)
                        .take(point_len - data.len())
                        .chain(data.iter().cloned());
                    elliptic_curve::FieldBytes::<C>::from_exact_iter(iter).unwrap()
                }
            }

            fn run_test(
                wx: &[u8],
                wy: &[u8],
                msg: &[u8],
                sig: &[u8],
                pass: bool,
            ) -> Option<&'static str> {
                let x = element_from_padded_slice::<$curve>(wx);
                let y = element_from_padded_slice::<$curve>(wy);
                let q_encoded: EncodedPoint<$curve> =
                    EncodedPoint::from_affine_coordinates(&x, &y, /* compress= */ false);
                let verifying_key = $crate::VerifyingKey::from_encoded_point(&q_encoded).unwrap();

                let sig = match Signature::from_der(sig) {
                    Ok(s) => s,
                    Err(_) if !pass => return None,
                    Err(_) => return Some("failed to parse signature ASN.1"),
                };

                match verifying_key.verify(msg, &sig) {
                    Ok(_) if pass => None,
                    Ok(_) => Some("signature verify failed"),
                    Err(_) if !pass => None,
                    Err(_) => Some("signature verify unexpectedly succeeded"),
                }
            }

            let data = include_bytes!(concat!("test_vectors/data/", $test_name, ".blb"));

            for (i, row) in Blob5Iterator::new(data).unwrap().enumerate() {
                let [wx, wy, msg, sig, status] = row.unwrap();
                let pass = match status[0] {
                    0 => false,
                    1 => true,
                    _ => panic!("invalid value for pass flag"),
                };
                if let Some(desc) = run_test(wx, wy, msg, sig, pass) {
                    panic!(
                        "\n\
                                 Failed test №{}: {}\n\
                                 wx:\t{:?}\n\
                                 wy:\t{:?}\n\
                                 msg:\t{:?}\n\
                                 sig:\t{:?}\n\
                                 pass:\t{}\n",
                        i, desc, wx, wy, msg, sig, pass,
                    );
                }
            }
        }
    };
}
