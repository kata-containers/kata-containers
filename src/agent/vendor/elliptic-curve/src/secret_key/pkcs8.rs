//! PKCS#8 encoding/decoding support

use super::{SecretKey, SecretValue};
use crate::{
    sec1::{self, UncompressedPointSize, UntaggedPointSize, ValidatePublicKey},
    weierstrass, AlgorithmParameters, FieldBytes, ALGORITHM_OID,
};
use core::ops::Add;
use generic_array::{typenum::U1, ArrayLength};
use pkcs8::{der, FromPrivateKey};
use zeroize::Zeroize;

// Imports for the `ToPrivateKey` impl
// TODO(tarcieri): use weak activation of `pkcs8/alloc` for gating `ToPrivateKey` impl
#[cfg(all(feature = "arithmetic", feature = "pem"))]
use {
    crate::{
        ff::PrimeField,
        scalar::Scalar,
        sec1::{FromEncodedPoint, ToEncodedPoint},
        AffinePoint, ProjectiveArithmetic, ProjectivePoint,
    },
    alloc::vec::Vec,
    core::{convert::TryInto, fmt::Debug, iter},
    pkcs8::{der::Encodable, ToPrivateKey},
    zeroize::Zeroizing,
};

// Imports for actual PEM support
#[cfg(feature = "pem")]
use {
    crate::{error::Error, Result},
    core::str::FromStr,
};

/// Version
const VERSION: u8 = 1;

/// Encoding error message
#[cfg(all(feature = "arithmetic", feature = "pem"))]
const ENCODING_ERROR_MSG: &str = "DER encoding error";

#[cfg_attr(docsrs, doc(cfg(feature = "pkcs8")))]
impl<C> FromPrivateKey for SecretKey<C>
where
    C: weierstrass::Curve + AlgorithmParameters + ValidatePublicKey + SecretValue,
    C::Secret: Clone + Zeroize,
    FieldBytes<C>: From<C::Secret>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    fn from_pkcs8_private_key_info(
        private_key_info: pkcs8::PrivateKeyInfo<'_>,
    ) -> pkcs8::Result<Self> {
        if private_key_info.algorithm.oid != ALGORITHM_OID
            || private_key_info.algorithm.parameters_oid()? != C::OID
        {
            return Err(pkcs8::Error::Decode);
        }

        let mut decoder = der::Decoder::new(private_key_info.private_key);

        let result = decoder.sequence(|decoder| {
            if decoder.uint8()? != VERSION {
                return Err(der::ErrorKind::Value {
                    tag: der::Tag::Integer,
                }
                .into());
            }

            let secret_key = Self::from_bytes(decoder.octet_string()?).map_err(|_| {
                der::Error::from(der::ErrorKind::Value {
                    tag: der::Tag::Sequence,
                })
            })?;

            let public_key_field = decoder.any()?;

            public_key_field
                .tag()
                .assert_eq(der::Tag::ContextSpecific1)?;

            let public_key_bytes = der::Decoder::new(public_key_field.as_bytes()).bit_string()?;

            if let Ok(pk) = sec1::EncodedPoint::<C>::from_bytes(public_key_bytes.as_ref()) {
                if C::validate_public_key(&secret_key, &pk).is_ok() {
                    return Ok(secret_key);
                }
            }

            Err(der::ErrorKind::Value {
                tag: der::Tag::BitString,
            }
            .into())
        })?;

        Ok(decoder.finish(result)?)
    }
}

// TODO(tarcieri): use weak activation of `pkcs8/alloc` for this when possible
// It doesn't strictly depend on `pkcs8/pem` but we can't easily activate `pkcs8/alloc`
// without adding a separate crate feature just for this functionality.
#[cfg(all(feature = "arithmetic", feature = "pem"))]
#[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl<C> ToPrivateKey for SecretKey<C>
where
    C: weierstrass::Curve + AlgorithmParameters + ProjectiveArithmetic,
    AffinePoint<C>: Copy + Clone + Debug + Default + FromEncodedPoint<C> + ToEncodedPoint<C>,
    ProjectivePoint<C>: From<AffinePoint<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>> + Zeroize,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    fn to_pkcs8_der(&self) -> pkcs8::PrivateKeyDocument {
        // TODO(tarcieri): wrap `secret_key_bytes` in `Zeroizing`
        let mut secret_key_bytes = self.to_bytes();
        let secret_key_field = der::OctetString::new(&secret_key_bytes).expect(ENCODING_ERROR_MSG);

        let public_key_body = self.public_key().to_encoded_point(false);
        let public_key_bytes = der::BitString::new(public_key_body.as_ref())
            .and_then(|bit_string| bit_string.to_vec())
            .expect("DER encoding error");

        let public_key_field =
            der::Any::new(der::Tag::ContextSpecific1, &public_key_bytes).expect(ENCODING_ERROR_MSG);

        let der_message_fields: &[&dyn Encodable] =
            &[&VERSION, &secret_key_field, &public_key_field];

        let encoded_len = der::message::encoded_len(der_message_fields)
            .and_then(TryInto::try_into)
            .expect(ENCODING_ERROR_MSG);

        let mut der_message = Zeroizing::new(Vec::new());
        der_message.reserve(encoded_len);
        der_message.extend(iter::repeat(0).take(encoded_len));

        let mut encoder = der::Encoder::new(&mut der_message);
        encoder
            .message(der_message_fields)
            .expect(ENCODING_ERROR_MSG);

        encoder.finish().expect(ENCODING_ERROR_MSG);
        secret_key_bytes.zeroize();

        pkcs8::PrivateKeyInfo {
            algorithm: C::algorithm_identifier(),
            private_key: &der_message,
        }
        .to_der()
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl<C> FromStr for SecretKey<C>
where
    C: weierstrass::Curve + AlgorithmParameters + ValidatePublicKey + SecretValue,
    C::Secret: Clone + Zeroize,
    FieldBytes<C>: From<C::Secret>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::from_pkcs8_pem(s).map_err(|_| Error)
    }
}
