//! PKCS#1 RSA Private Keys.

#[cfg(feature = "alloc")]
pub(crate) mod document;
#[cfg(feature = "alloc")]
pub(crate) mod other_prime_info;

use crate::{Error, Result, RsaPublicKey, Version};
use core::fmt;
use der::{asn1::UIntBytes, Decodable, Decoder, Encodable, Sequence, Tag};

#[cfg(feature = "alloc")]
use {self::other_prime_info::OtherPrimeInfo, crate::RsaPrivateKeyDocument, alloc::vec::Vec};

#[cfg(feature = "pem")]
use {
    crate::{EncodeRsaPrivateKey, LineEnding},
    alloc::string::String,
    zeroize::Zeroizing,
};

/// PKCS#1 RSA Private Keys as defined in [RFC 8017 Appendix 1.2].
///
/// ASN.1 structure containing a serialized RSA private key:
///
/// ```text
/// RSAPrivateKey ::= SEQUENCE {
///     version           Version,
///     modulus           INTEGER,  -- n
///     publicExponent    INTEGER,  -- e
///     privateExponent   INTEGER,  -- d
///     prime1            INTEGER,  -- p
///     prime2            INTEGER,  -- q
///     exponent1         INTEGER,  -- d mod (p-1)
///     exponent2         INTEGER,  -- d mod (q-1)
///     coefficient       INTEGER,  -- (inverse of q) mod p
///     otherPrimeInfos   OtherPrimeInfos OPTIONAL
/// }
/// ```
///
/// Note: the `version` field is selected automatically based on the absence or
/// presence of the `other_prime_infos` field.
///
/// [RFC 8017 Appendix 1.2]: https://datatracker.ietf.org/doc/html/rfc8017#appendix-A.1.2
#[derive(Clone)]
pub struct RsaPrivateKey<'a> {
    /// `n`: RSA modulus.
    pub modulus: UIntBytes<'a>,

    /// `e`: RSA public exponent.
    pub public_exponent: UIntBytes<'a>,

    /// `d`: RSA private exponent.
    pub private_exponent: UIntBytes<'a>,

    /// `p`: first prime factor of `n`.
    pub prime1: UIntBytes<'a>,

    /// `q`: Second prime factor of `n`.
    pub prime2: UIntBytes<'a>,

    /// First exponent: `d mod (p-1)`.
    pub exponent1: UIntBytes<'a>,

    /// Second exponent: `d mod (q-1)`.
    pub exponent2: UIntBytes<'a>,

    /// CRT coefficient: `(inverse of q) mod p`.
    pub coefficient: UIntBytes<'a>,

    /// Additional primes `r_3`, ..., `r_u`, in order, if this is a multi-prime
    /// RSA key (i.e. `version` is `multi`).
    pub other_prime_infos: Option<OtherPrimeInfos<'a>>,
}

impl<'a> RsaPrivateKey<'a> {
    /// Get the public key that corresponds to this [`RsaPrivateKey`].
    pub fn public_key(&self) -> RsaPublicKey<'a> {
        RsaPublicKey {
            modulus: self.modulus,
            public_exponent: self.public_exponent,
        }
    }

    /// Get the [`Version`] for this key.
    ///
    /// Determined by the presence or absence of the
    /// [`RsaPrivateKey::other_prime_infos`] field.
    pub fn version(&self) -> Version {
        if self.other_prime_infos.is_some() {
            Version::Multi
        } else {
            Version::TwoPrime
        }
    }

    /// Encode this [`RsaPrivateKey`] as ASN.1 DER.
    #[cfg(feature = "alloc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub fn to_der(&self) -> Result<RsaPrivateKeyDocument> {
        self.try_into()
    }

    /// Encode this [`RsaPrivateKey`] as PEM-encoded ASN.1 DER using the given
    /// [`LineEnding`].
    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    pub fn to_pem(&self, line_ending: LineEnding) -> Result<Zeroizing<String>> {
        self.to_der()?.to_pkcs1_pem(line_ending)
    }
}

impl<'a> Decodable<'a> for RsaPrivateKey<'a> {
    fn decode(decoder: &mut Decoder<'a>) -> der::Result<Self> {
        decoder.sequence(|decoder| {
            let version = Version::decode(decoder)?;

            let result = Self {
                modulus: decoder.decode()?,
                public_exponent: decoder.decode()?,
                private_exponent: decoder.decode()?,
                prime1: decoder.decode()?,
                prime2: decoder.decode()?,
                exponent1: decoder.decode()?,
                exponent2: decoder.decode()?,
                coefficient: decoder.decode()?,
                other_prime_infos: decoder.decode()?,
            };

            // Ensure version is set correctly for two-prime vs multi-prime key.
            if version.is_multi() != result.other_prime_infos.is_some() {
                return Err(decoder.error(der::ErrorKind::Value { tag: Tag::Integer }));
            }

            Ok(result)
        })
    }
}

impl<'a> Sequence<'a> for RsaPrivateKey<'a> {
    fn fields<F, T>(&self, f: F) -> der::Result<T>
    where
        F: FnOnce(&[&dyn Encodable]) -> der::Result<T>,
    {
        f(&[
            &self.version(),
            &self.modulus,
            &self.public_exponent,
            &self.private_exponent,
            &self.prime1,
            &self.prime2,
            &self.exponent1,
            &self.exponent2,
            &self.coefficient,
            #[cfg(feature = "alloc")]
            &self.other_prime_infos,
        ])
    }
}

impl<'a> From<RsaPrivateKey<'a>> for RsaPublicKey<'a> {
    fn from(private_key: RsaPrivateKey<'a>) -> RsaPublicKey<'a> {
        private_key.public_key()
    }
}

impl<'a> From<&RsaPrivateKey<'a>> for RsaPublicKey<'a> {
    fn from(private_key: &RsaPrivateKey<'a>) -> RsaPublicKey<'a> {
        private_key.public_key()
    }
}

impl<'a> TryFrom<&'a [u8]> for RsaPrivateKey<'a> {
    type Error = Error;

    fn try_from(bytes: &'a [u8]) -> Result<Self> {
        Ok(Self::from_der(bytes)?)
    }
}

impl<'a> fmt::Debug for RsaPrivateKey<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RsaPrivateKey")
            .field("version", &self.version())
            .field("modulus", &self.modulus)
            .field("public_exponent", &self.public_exponent)
            .finish_non_exhaustive()
    }
}

/// Placeholder struct for `OtherPrimeInfos` in the no-`alloc` case.
#[cfg(not(feature = "alloc"))]
#[derive(Clone)]
#[non_exhaustive]
pub struct OtherPrimeInfos<'a> {
    _lifetime: core::marker::PhantomData<&'a ()>,
}

#[cfg(not(feature = "alloc"))]
impl<'a> Decodable<'a> for OtherPrimeInfos<'a> {
    fn decode(decoder: &mut Decoder<'a>) -> der::Result<Self> {
        // Placeholder decoder that always returns an error.
        // Use `Tag::Integer` to signal an unsupported version.
        Err(decoder.error(der::ErrorKind::Value { tag: Tag::Integer }))
    }
}

#[cfg(not(feature = "alloc"))]
impl<'a> der::FixedTag for OtherPrimeInfos<'a> {
    const TAG: Tag = Tag::Sequence;
}

/// Additional RSA prime info in a multi-prime RSA key.
#[cfg(feature = "alloc")]
pub type OtherPrimeInfos<'a> = Vec<OtherPrimeInfo<'a>>;
