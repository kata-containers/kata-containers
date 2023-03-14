//! Affine points

use super::{FieldElement, ProjectivePoint, CURVE_EQUATION_A, CURVE_EQUATION_B, MODULUS};
use crate::{EncodedPoint, FieldBytes, NistP256, NonZeroScalar};
use core::ops::{Mul, Neg};
use elliptic_curve::{
    generic_array::arr,
    sec1::{self, FromEncodedPoint, ToEncodedPoint},
    subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption},
    weierstrass::DecompressPoint,
};

#[cfg(feature = "zeroize")]
use elliptic_curve::zeroize::Zeroize;

/// A point on the secp256r1 curve in affine coordinates.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
pub struct AffinePoint {
    pub(crate) x: FieldElement,
    pub(crate) y: FieldElement,
    pub(super) infinity: Choice,
}

impl AffinePoint {
    /// Returns the base point of P-256.
    pub fn generator() -> AffinePoint {
        // NIST P-256 basepoint in affine coordinates:
        // x = 6b17d1f2 e12c4247 f8bce6e5 63a440f2 77037d81 2deb33a0 f4a13945 d898c296
        // y = 4fe342e2 fe1a7f9b 8ee7eb4a 7c0f9e16 2bce3357 6b315ece cbb64068 37bf51f5
        AffinePoint {
            x: FieldElement::from_bytes(&arr![u8;
                0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63, 0xa4,
                0x40, 0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39, 0x45,
                0xd8, 0x98, 0xc2, 0x96
            ])
            .unwrap(),
            y: FieldElement::from_bytes(&arr![u8;
                0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b, 0x8e, 0xe7, 0xeb, 0x4a, 0x7c, 0x0f,
                0x9e, 0x16, 0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e, 0xce, 0xcb, 0xb6, 0x40, 0x68,
                0x37, 0xbf, 0x51, 0xf5
            ])
            .unwrap(),
            infinity: Choice::from(0),
        }
    }

    /// Returns the identity of the group: the point at infinity.
    pub fn identity() -> AffinePoint {
        Self {
            x: FieldElement::zero(),
            y: FieldElement::zero(),
            infinity: Choice::from(1),
        }
    }

    /// Is this point the identity point?
    pub fn is_identity(&self) -> Choice {
        self.infinity
    }
}

impl ConditionallySelectable for AffinePoint {
    fn conditional_select(a: &AffinePoint, b: &AffinePoint, choice: Choice) -> AffinePoint {
        AffinePoint {
            x: FieldElement::conditional_select(&a.x, &b.x, choice),
            y: FieldElement::conditional_select(&a.y, &b.y, choice),
            infinity: Choice::conditional_select(&a.infinity, &b.infinity, choice),
        }
    }
}

impl ConstantTimeEq for AffinePoint {
    fn ct_eq(&self, other: &AffinePoint) -> Choice {
        self.x.ct_eq(&other.x) & self.y.ct_eq(&other.y) & self.infinity.ct_eq(&other.infinity)
    }
}

impl Default for AffinePoint {
    fn default() -> Self {
        Self::identity()
    }
}

impl PartialEq for AffinePoint {
    fn eq(&self, other: &AffinePoint) -> bool {
        self.ct_eq(other).into()
    }
}

impl Eq for AffinePoint {}

impl DecompressPoint<NistP256> for AffinePoint {
    fn decompress(x_bytes: &FieldBytes, y_is_odd: Choice) -> CtOption<Self> {
        FieldElement::from_bytes(x_bytes).and_then(|x| {
            let alpha = x * &x * &x + &(CURVE_EQUATION_A * &x) + &CURVE_EQUATION_B;
            let beta = alpha.sqrt();

            beta.map(|beta| {
                let y = FieldElement::conditional_select(
                    &(MODULUS - &beta),
                    &beta,
                    beta.is_odd().ct_eq(&y_is_odd),
                );

                Self {
                    x,
                    y,
                    infinity: Choice::from(0),
                }
            })
        })
    }
}

impl FromEncodedPoint<NistP256> for AffinePoint {
    /// Attempts to parse the given [`EncodedPoint`] as an SEC1-encoded [`AffinePoint`].
    ///
    /// # Returns
    ///
    /// `None` value if `encoded_point` is not on the secp256r1 curve.
    fn from_encoded_point(encoded_point: &EncodedPoint) -> Option<Self> {
        match encoded_point.coordinates() {
            sec1::Coordinates::Identity => Some(Self::identity()),
            sec1::Coordinates::Compressed { x, y_is_odd } => {
                AffinePoint::decompress(x, Choice::from(y_is_odd as u8)).into()
            }
            sec1::Coordinates::Uncompressed { x, y } => {
                let x = FieldElement::from_bytes(x);
                let y = FieldElement::from_bytes(y);

                x.and_then(|x| {
                    y.and_then(|y| {
                        // Check that the point is on the curve
                        let lhs = y * &y;
                        let rhs = x * &x * &x + &(CURVE_EQUATION_A * &x) + &CURVE_EQUATION_B;
                        let point = AffinePoint {
                            x,
                            y,
                            infinity: Choice::from(0),
                        };
                        CtOption::new(point, lhs.ct_eq(&rhs))
                    })
                })
                .into()
            }
        }
    }
}

impl ToEncodedPoint<NistP256> for AffinePoint {
    fn to_encoded_point(&self, compress: bool) -> EncodedPoint {
        EncodedPoint::conditional_select(
            &EncodedPoint::from_affine_coordinates(
                &self.x.to_bytes(),
                &self.y.to_bytes(),
                compress,
            ),
            &EncodedPoint::identity(),
            self.infinity,
        )
    }
}

impl From<AffinePoint> for EncodedPoint {
    /// Returns the SEC1 compressed encoding of this point.
    fn from(affine_point: AffinePoint) -> EncodedPoint {
        affine_point.to_encoded_point(false)
    }
}

impl Mul<NonZeroScalar> for AffinePoint {
    type Output = AffinePoint;

    fn mul(self, scalar: NonZeroScalar) -> Self {
        (ProjectivePoint::from(self) * scalar.as_ref()).to_affine()
    }
}

impl Neg for AffinePoint {
    type Output = AffinePoint;

    fn neg(self) -> Self::Output {
        AffinePoint {
            x: self.x,
            y: -self.y,
            infinity: self.infinity,
        }
    }
}

#[cfg(feature = "zeroize")]
impl Zeroize for AffinePoint {
    fn zeroize(&mut self) {
        self.x.zeroize();
        self.y.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::AffinePoint;
    use crate::EncodedPoint;
    use elliptic_curve::sec1::{FromEncodedPoint, ToEncodedPoint};
    use hex_literal::hex;

    const UNCOMPRESSED_BASEPOINT: &[u8] = &hex!(
        "046B17D1F2E12C4247F8BCE6E563A440F277037D812DEB33A0F4A13945D898C296
         4FE342E2FE1A7F9B8EE7EB4A7C0F9E162BCE33576B315ECECBB6406837BF51F5"
    );

    const COMPRESSED_BASEPOINT: &[u8] =
        &hex!("036B17D1F2E12C4247F8BCE6E563A440F277037D812DEB33A0F4A13945D898C296");

    #[test]
    fn uncompressed_round_trip() {
        let pubkey = EncodedPoint::from_bytes(UNCOMPRESSED_BASEPOINT).unwrap();
        let point = AffinePoint::from_encoded_point(&pubkey).unwrap();
        assert_eq!(point, AffinePoint::generator());

        let res: EncodedPoint = point.into();
        assert_eq!(res, pubkey);
    }

    #[test]
    fn compressed_round_trip() {
        let pubkey = EncodedPoint::from_bytes(COMPRESSED_BASEPOINT).unwrap();
        let point = AffinePoint::from_encoded_point(&pubkey).unwrap();
        assert_eq!(point, AffinePoint::generator());

        let res: EncodedPoint = point.to_encoded_point(true);
        assert_eq!(res, pubkey);
    }

    #[test]
    fn uncompressed_to_compressed() {
        let encoded = EncodedPoint::from_bytes(UNCOMPRESSED_BASEPOINT).unwrap();

        let res = AffinePoint::from_encoded_point(&encoded)
            .unwrap()
            .to_encoded_point(true);

        assert_eq!(res.as_bytes(), COMPRESSED_BASEPOINT);
    }

    #[test]
    fn compressed_to_uncompressed() {
        let encoded = EncodedPoint::from_bytes(COMPRESSED_BASEPOINT).unwrap();

        let res = AffinePoint::from_encoded_point(&encoded)
            .unwrap()
            .to_encoded_point(false);

        assert_eq!(res.as_bytes(), UNCOMPRESSED_BASEPOINT);
    }

    #[test]
    fn decompress() {
        let encoded = EncodedPoint::from_bytes(COMPRESSED_BASEPOINT).unwrap();
        let decompressed = encoded.decompress().unwrap();
        assert_eq!(decompressed.as_bytes(), UNCOMPRESSED_BASEPOINT);
    }

    #[test]
    fn affine_negation() {
        let basepoint = AffinePoint::generator();
        assert_eq!(-(-basepoint), basepoint);
    }
}
