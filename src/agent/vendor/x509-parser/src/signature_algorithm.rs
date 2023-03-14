use crate::error::X509Error;
use crate::x509::AlgorithmIdentifier;
use asn1_rs::{
    oid, Any, CheckDerConstraints, Class, DerAutoDerive, Error, FromDer, Oid, OptTaggedExplicit,
    OptTaggedParser, Tag,
};
use core::convert::TryFrom;
use oid_registry::*;

#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq)]
pub enum SignatureAlgorithm<'a> {
    RSA,
    RSASSA_PSS(Box<RsaSsaPssParams<'a>>),
    RSAAES_OAEP(Box<RsaAesOaepParams<'a>>),
    DSA,
    ECDSA,
    ED25519,
}

impl<'a, 'b> TryFrom<&'b AlgorithmIdentifier<'a>> for SignatureAlgorithm<'a> {
    type Error = X509Error;

    fn try_from(value: &'b AlgorithmIdentifier<'a>) -> Result<Self, Self::Error> {
        if value.algorithm.starts_with(&oid! {1.2.840.113549.1.1}) {
            // children of PKCS1 are all RSA
            // test if RSASSA-PSS
            if value.algorithm == OID_PKCS1_RSASSAPSS {
                let params = match value.parameters.as_ref() {
                    Some(any) => any,
                    None => return Err(X509Error::InvalidSignatureValue),
                };
                let params = RsaSsaPssParams::try_from(params)
                    .map_err(|_| X509Error::InvalidSignatureValue)?;
                Ok(SignatureAlgorithm::RSASSA_PSS(Box::new(params)))
            } else {
                // rfc3279#section-2.2.1: the parameters component of that type SHALL be
                // the ASN.1 type NULL
                // We could enforce presence of NULL, but that would make a strict parser
                // so it would best go to a verifier.
                Ok(SignatureAlgorithm::RSA)
            }
        } else if test_ecdsa_oid(&value.algorithm) {
            // parameter should be NULL - see above
            Ok(SignatureAlgorithm::ECDSA)
        } else if value.algorithm.starts_with(&oid! {1.2.840.10040.4}) {
            // parameter should be NULL - see above
            Ok(SignatureAlgorithm::DSA)
        } else if value.algorithm == OID_SIG_ED25519 {
            Ok(SignatureAlgorithm::ED25519)
        } else if value.algorithm == oid! {1.2.840.113549.1.1.7} {
            let params = match value.parameters.as_ref() {
                Some(any) => any,
                None => return Err(X509Error::InvalidSignatureValue),
            };
            let params =
                RsaAesOaepParams::try_from(params).map_err(|_| X509Error::InvalidSignatureValue)?;
            Ok(SignatureAlgorithm::RSAAES_OAEP(Box::new(params)))
        } else {
            if cfg!(debug_assertions) {
                // TODO: remove debug
                eprintln!("bad Signature AlgorithmIdentifier: {}", value.algorithm);
            }
            Err(X509Error::InvalidSignatureValue)
        }
    }
}

#[inline]
fn test_ecdsa_oid(oid: &Oid) -> bool {
    // test if oid is a child from {ansi-x962 signatures}
    oid.starts_with(&oid! {1.2.840.10045.4})
}

// RSASSA-PSS public keys [RFC4055](https://www.rfc-editor.org/rfc/rfc4055.html)

// RSASSA-PSS-params  ::=  SEQUENCE  {
//     hashAlgorithm      [0] HashAlgorithm DEFAULT
//                               sha1Identifier,
//     maskGenAlgorithm   [1] MaskGenAlgorithm DEFAULT
//                               mgf1SHA1Identifier,
//     saltLength         [2] INTEGER DEFAULT 20,
//     trailerField       [3] INTEGER DEFAULT 1  }
#[derive(Debug, PartialEq)]
pub struct RsaSsaPssParams<'a> {
    hash_alg: Option<AlgorithmIdentifier<'a>>,
    mask_gen_algorithm: Option<AlgorithmIdentifier<'a>>,
    salt_length: Option<u32>,
    trailer_field: Option<u32>,
}

impl<'a> RsaSsaPssParams<'a> {
    /// Get a reference to the rsa ssa pss params's hash algorithm.
    pub fn hash_algorithm(&self) -> Option<&AlgorithmIdentifier> {
        self.hash_alg.as_ref()
    }

    /// Return the hash algorithm OID, or SHA1 if absent (RFC4055)
    pub fn hash_algorithm_oid(&self) -> &'a Oid {
        const SHA1: &Oid = &OID_HASH_SHA1;
        self.hash_alg
            .as_ref()
            .map(|alg| &alg.algorithm)
            .unwrap_or(SHA1)
    }

    /// Get a reference to the rsa ssa pss params's mask generation algorithm.
    pub fn mask_gen_algorithm_raw(&self) -> Option<&AlgorithmIdentifier> {
        self.mask_gen_algorithm.as_ref()
    }

    /// Get the rsa ssa pss params's mask generation algorithm.
    ///
    /// If the algorithm encoding is invalid, raise an error `InvalidAlgorithmIdentifier`
    pub fn mask_gen_algorithm(&self) -> Result<MaskGenAlgorithm, X509Error> {
        match self.mask_gen_algorithm.as_ref() {
            Some(alg) => {
                let (_, hash) = alg
                    .parameters()
                    .and_then(|any| Oid::from_der(any.data).ok())
                    .ok_or(X509Error::InvalidAlgorithmIdentifier)?;
                Ok(MaskGenAlgorithm::new(alg.algorithm.clone(), hash))
            }
            _ => {
                Ok(MaskGenAlgorithm::new(
                    oid! {1.2.840.113549.1.1.8}, // id-mgf1
                    OID_HASH_SHA1,
                ))
            }
        }
    }

    /// Return the salt length
    pub fn salt_length(&self) -> u32 {
        self.salt_length.unwrap_or(20)
    }

    /// Return the trailer field (value must be `1` according to RFC4055)
    pub fn trailer_field(&self) -> u32 {
        self.trailer_field.unwrap_or(1)
    }
}

impl<'a> TryFrom<Any<'a>> for RsaSsaPssParams<'a> {
    type Error = X509Error;

    fn try_from(value: Any<'a>) -> Result<Self, Self::Error> {
        Self::try_from(&value)
    }
}

impl<'a, 'b> TryFrom<&'b Any<'a>> for RsaSsaPssParams<'a> {
    type Error = X509Error;

    fn try_from(value: &'b Any<'a>) -> Result<Self, Self::Error> {
        value.tag().assert_eq(Tag::Sequence)?;
        let i = &value.data;
        // let (i, hash_alg) = OptTaggedExplicit::<_, X509Error, 0>::from_der(i)?;
        let (i, hash_alg) = OptTaggedParser::new(Class::ContextSpecific, Tag(0))
            .parse_der(i, |_, inner| AlgorithmIdentifier::from_der(inner))?;
        // let (i, mask_gen_algorithm) = OptTaggedExplicit::<_, Error, 1>::from_der(i)?;
        let (i, mask_gen_algorithm) = OptTaggedParser::new(Class::ContextSpecific, Tag(1))
            .parse_der(i, |_, inner| AlgorithmIdentifier::from_der(inner))?;
        let (i, salt_length) = OptTaggedExplicit::<_, Error, 2>::from_der(i)?;
        let (_, trailer_field) = OptTaggedExplicit::<_, Error, 3>::from_der(i)?;
        let params = RsaSsaPssParams {
            hash_alg,
            mask_gen_algorithm,
            salt_length: salt_length.map(|t| t.into_inner()),
            trailer_field: trailer_field.map(|t| t.into_inner()),
        };
        Ok(params)
    }
}

impl CheckDerConstraints for RsaSsaPssParams<'_> {
    fn check_constraints(any: &Any) -> asn1_rs::Result<()> {
        any.header.assert_constructed()?;
        Ok(())
    }
}

impl DerAutoDerive for RsaSsaPssParams<'_> {}

#[derive(Debug, PartialEq)]
pub struct MaskGenAlgorithm<'a, 'b> {
    pub mgf: Oid<'a>,
    pub hash: Oid<'b>,
}

impl<'a, 'b> MaskGenAlgorithm<'a, 'b> {
    pub const fn new(mgf: Oid<'a>, hash: Oid<'b>) -> Self {
        Self { mgf, hash }
    }
}

// RSAAES-OAEP public keys [RFC8017](https://www.rfc-editor.org/rfc/rfc8017.html)

// RSAES-OAEP-params  ::=  SEQUENCE  {
//     hashFunc          [0] AlgorithmIdentifier DEFAULT
//                              sha1Identifier,
//     maskGenFunc       [1] AlgorithmIdentifier DEFAULT
//                              mgf1SHA1Identifier,
//     pSourceFunc       [2] AlgorithmIdentifier DEFAULT
//                              pSpecifiedEmptyIdentifier  }
//
//  pSpecifiedEmptyIdentifier  AlgorithmIdentifier  ::=
//                       { id-pSpecified, nullOctetString }
//
//  nullOctetString  OCTET STRING (SIZE (0))  ::=  { ''H }
#[derive(Debug, PartialEq)]
pub struct RsaAesOaepParams<'a> {
    hash_alg: Option<AlgorithmIdentifier<'a>>,
    mask_gen_alg: Option<AlgorithmIdentifier<'a>>,
    p_source_alg: Option<AlgorithmIdentifier<'a>>,
}

impl<'a> RsaAesOaepParams<'a> {
    pub const EMPTY: &'static AlgorithmIdentifier<'static> = &AlgorithmIdentifier::new(
        oid! {1.2.840.113549.1.1.9}, // id-pSpecified
        None,
    );

    /// Get a reference to the rsa aes oaep params's hash algorithm.
    pub fn hash_algorithm(&self) -> Option<&AlgorithmIdentifier> {
        self.hash_alg.as_ref()
    }

    /// Return the hash algorithm OID, or SHA1 if absent (RFC4055)
    pub fn hash_algorithm_oid(&self) -> &'a Oid {
        const SHA1: &Oid = &OID_HASH_SHA1;
        self.hash_alg
            .as_ref()
            .map(|alg| &alg.algorithm)
            .unwrap_or(SHA1)
    }

    /// Get a reference to the rsa ssa pss params's mask generation algorithm.
    pub fn mask_gen_algorithm_raw(&self) -> Option<&AlgorithmIdentifier> {
        self.mask_gen_alg.as_ref()
    }

    /// Get the rsa ssa pss params's mask generation algorithm.
    ///
    /// If the algorithm encoding is invalid, raise an error `InvalidAlgorithmIdentifier`
    pub fn mask_gen_algorithm(&self) -> Result<MaskGenAlgorithm, X509Error> {
        match self.mask_gen_alg.as_ref() {
            Some(alg) => {
                let (_, hash) = alg
                    .parameters()
                    .and_then(|any| Oid::from_der(any.data).ok())
                    .ok_or(X509Error::InvalidAlgorithmIdentifier)?;
                Ok(MaskGenAlgorithm::new(alg.algorithm.clone(), hash))
            }
            _ => {
                Ok(MaskGenAlgorithm::new(
                    oid! {1.2.840.113549.1.1.8}, // id-mgf1
                    OID_HASH_SHA1,
                ))
            }
        }
    }

    /// Return the pSourceFunc algorithm
    pub fn p_source_alg(&'a self) -> &'a AlgorithmIdentifier {
        self.p_source_alg.as_ref().unwrap_or(Self::EMPTY)
    }
}

impl<'a> TryFrom<Any<'a>> for RsaAesOaepParams<'a> {
    type Error = X509Error;

    fn try_from(value: Any<'a>) -> Result<Self, Self::Error> {
        Self::try_from(&value)
    }
}

//     hashFunc          [0] AlgorithmIdentifier DEFAULT
//                              sha1Identifier,
//     maskGenFunc       [1] AlgorithmIdentifier DEFAULT
//                              mgf1SHA1Identifier,
//     pSourceFunc       [2] AlgorithmIdentifier DEFAULT
//                              pSpecifiedEmptyIdentifier  }
impl<'a, 'b> TryFrom<&'b Any<'a>> for RsaAesOaepParams<'a> {
    type Error = X509Error;

    fn try_from(value: &'b Any<'a>) -> Result<Self, Self::Error> {
        value.tag().assert_eq(Tag::Sequence)?;
        let i = &value.data;
        // let (i, hash_alg) = OptTaggedExplicit::<_, X509Error, 0>::from_der(i)?;
        let (i, hash_alg) = OptTaggedParser::new(Class::ContextSpecific, Tag(0))
            .parse_der(i, |_, inner| AlgorithmIdentifier::from_der(inner))?;
        // let (i, mask_gen_algorithm) = OptTaggedExplicit::<_, Error, 1>::from_der(i)?;
        let (i, mask_gen_alg) = OptTaggedParser::new(Class::ContextSpecific, Tag(1))
            .parse_der(i, |_, inner| AlgorithmIdentifier::from_der(inner))?;
        let (_, p_source_alg) = OptTaggedParser::new(Class::ContextSpecific, Tag(2))
            .parse_der(i, |_, inner| AlgorithmIdentifier::from_der(inner))?;
        let params = RsaAesOaepParams {
            hash_alg,
            mask_gen_alg,
            p_source_alg,
        };
        Ok(params)
    }
}

impl CheckDerConstraints for RsaAesOaepParams<'_> {
    fn check_constraints(any: &Any) -> asn1_rs::Result<()> {
        any.header.assert_constructed()?;
        Ok(())
    }
}

impl DerAutoDerive for RsaAesOaepParams<'_> {}

// ECC subject public key information [RFC5480](https://datatracker.ietf.org/doc/rfc5480/)

// ECParameters ::= CHOICE {
//     namedCurve         OBJECT IDENTIFIER
//     -- implicitCurve   NULL
//     -- specifiedCurve  SpecifiedECDomain
//   }
//     -- implicitCurve and specifiedCurve MUST NOT be used in PKIX.
//     -- Details for SpecifiedECDomain can be found in [X9.62].
//     -- Any future additions to this CHOICE should be coordinated
//     -- with ANSI X9.
