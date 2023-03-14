//
// Copyright 2021 The Sigstore Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Structures and constants required to perform cryptographic operations.

use ring::signature;
use std::convert::TryFrom;

/// The default signature verification algorithm used by Sigstore.
/// Sigstore relies on NIST P-256
/// NIST P-256 is a Weierstrass curve specified in [FIPS 186-4: Digital Signature Standard (DSS)](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.186-4.pdf).
/// Also known as prime256v1 (ANSI X9.62) and secp256r1 (SECG)
pub static SIGSTORE_DEFAULT_SIGNATURE_VERIFICATION_ALGORITHM:
    &signature::EcdsaVerificationAlgorithm = &signature::ECDSA_P256_SHA256_ASN1;

/// Describes the signature digest algorithms supported.
/// The default one is sha256.
pub enum SignatureDigestAlgorithm {
    Sha256,
    Sha384,
    Sha512,
}

impl TryFrom<&str> for SignatureDigestAlgorithm {
    type Error = String;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value {
            "sha256" => Ok(Self::Sha256),
            "sha384" => Ok(Self::Sha384),
            "sha512" => Ok(Self::Sha512),
            unknown => Err(format!(
                "Unsupported signature digest algorithm: {}",
                unknown
            )),
        }
    }
}

impl Default for SignatureDigestAlgorithm {
    fn default() -> Self {
        Self::Sha256
    }
}

/// A signature produced by a private key
pub enum Signature<'a> {
    /// Raw signature. There's no need to process the contents
    Raw(&'a [u8]),
    /// A base64 encoded signature
    Base64Encoded(&'a [u8]),
}

pub(crate) mod certificate;
pub(crate) mod certificate_pool;

pub mod verification_key;
pub use verification_key::CosignVerificationKey;

#[cfg(test)]
pub(crate) mod tests {
    use chrono::{DateTime, Duration, Utc};
    use openssl::asn1::{Asn1Integer, Asn1Time};
    use openssl::bn::{BigNum, MsbOption};
    use openssl::conf::{Conf, ConfMethod};
    use openssl::ec::{EcGroup, EcKey};
    use openssl::hash::MessageDigest;
    use openssl::nid::Nid;
    use openssl::pkey;
    use openssl::x509::extension::{
        AuthorityKeyIdentifier, BasicConstraints, ExtendedKeyUsage, KeyUsage,
        SubjectAlternativeName, SubjectKeyIdentifier,
    };
    use openssl::x509::{X509Extension, X509NameBuilder, X509};

    pub(crate) const PUBLIC_KEY: &str = r#"-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAENptdY/l3nB0yqkXLBWkZWQwo6+cu
OSWS1X9vPavpiQOoTTGC0xX57OojUadxF1cdQmrsiReWg2Wn4FneJfa8xw==
-----END PUBLIC KEY-----"#;

    pub(crate) struct CertData {
        pub cert: X509,
        pub private_key: EcKey<pkey::Private>,
    }

    pub(crate) struct CertGenerationOptions {
        pub digital_signature_key_usage: bool,
        pub code_signing_extended_key_usage: bool,
        pub subject_email: Option<String>,
        pub subject_url: Option<String>,
        //TODO: remove macro once https://github.com/sfackler/rust-openssl/issues/1411
        //is fixed
        #[allow(dead_code)]
        pub subject_issuer: Option<String>,
        pub not_before: DateTime<chrono::Utc>,
        pub not_after: DateTime<chrono::Utc>,
    }

    impl Default for CertGenerationOptions {
        fn default() -> Self {
            let not_before = Utc::now().checked_sub_signed(Duration::days(1)).unwrap();
            let not_after = Utc::now().checked_add_signed(Duration::days(1)).unwrap();

            CertGenerationOptions {
                digital_signature_key_usage: true,
                code_signing_extended_key_usage: true,
                subject_email: Some(String::from("tests@sigstore-rs.dev")),
                subject_issuer: Some(String::from("https://sigstore.dev/oauth")),
                subject_url: None,
                not_before,
                not_after,
            }
        }
    }

    pub(crate) fn generate_certificate(
        issuer: Option<&CertData>,
        settings: CertGenerationOptions,
    ) -> anyhow::Result<CertData> {
        // Sigstore relies on NIST P-256
        // NIST P-256 is a Weierstrass curve specified in FIPS 186-4: Digital Signature Standard (DSS):
        // https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.186-4.pdf
        // Also known as prime256v1 (ANSI X9.62) and secp256r1 (SECG)
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).expect("Cannot create EcGroup");
        let private_key = EcKey::generate(&group).expect("Cannot create private key");
        let public_key = private_key.public_key();

        let ec_pub_key =
            EcKey::from_public_key(&group, &public_key).expect("Cannot create ec pub key");
        let pkey = pkey::PKey::from_ec_key(ec_pub_key).expect("Cannot create pkey");

        let mut x509_name_builder = X509NameBuilder::new()?;
        x509_name_builder.append_entry_by_text("O", "tests")?;
        x509_name_builder.append_entry_by_text("CN", "sigstore.test")?;
        let x509_name = x509_name_builder.build();

        let mut x509_builder = openssl::x509::X509::builder()?;
        x509_builder.set_subject_name(&x509_name)?;
        x509_builder
            .set_pubkey(&pkey)
            .expect("Cannot set public key");

        // set serial number
        let mut big = BigNum::new().expect("Cannot create BigNum");
        big.rand(152, MsbOption::MAYBE_ZERO, true)?;
        let serial_number = Asn1Integer::from_bn(&big)?;
        x509_builder.set_serial_number(&serial_number)?;

        // set version 3
        x509_builder.set_version(2)?;

        // x509 v3 extensions
        let conf = Conf::new(ConfMethod::default())?;
        let x509v3_context = match issuer {
            Some(issuer_data) => x509_builder.x509v3_context(Some(&issuer_data.cert), Some(&conf)),
            None => x509_builder.x509v3_context(None, Some(&conf)),
        };

        let mut extensions: Vec<X509Extension> = Vec::new();

        let x509_extension_subject_key_identifier =
            SubjectKeyIdentifier::new().build(&x509v3_context)?;
        extensions.push(x509_extension_subject_key_identifier);

        // CA usage
        if issuer.is_none() {
            // CA usage
            let x509_basic_constraint_ca =
                BasicConstraints::new().critical().ca().pathlen(1).build()?;
            extensions.push(x509_basic_constraint_ca);
        } else {
            let x509_basic_constraint_ca = BasicConstraints::new().critical().build()?;
            extensions.push(x509_basic_constraint_ca);
        }

        // set key usage
        if issuer.is_some() {
            if settings.digital_signature_key_usage {
                let key_usage = KeyUsage::new().critical().digital_signature().build()?;
                extensions.push(key_usage);
            }

            if settings.code_signing_extended_key_usage {
                let extended_key_usage = ExtendedKeyUsage::new().code_signing().build()?;
                extensions.push(extended_key_usage);
            }
        } else {
            let key_usage = KeyUsage::new()
                .critical()
                .crl_sign()
                .key_cert_sign()
                .build()?;
            extensions.push(key_usage);
        }

        // extensions that diverge, based on whether we're creating the CA or
        // a certificate issued by it
        if issuer.is_none() {
        } else {
            let x509_extension_authority_key_identifier = AuthorityKeyIdentifier::new()
                .keyid(true)
                .build(&x509v3_context)?;
            extensions.push(x509_extension_authority_key_identifier);

            if settings.subject_email.is_some() && settings.subject_url.is_some() {
                panic!(
                    "cosign doesn't generate certificates with a SAN that has both email and url"
                );
            }
            if let Some(email) = settings.subject_email {
                let x509_extension_san = SubjectAlternativeName::new()
                    .critical()
                    .email(&email)
                    .build(&x509v3_context)?;

                extensions.push(x509_extension_san);
            };
            if let Some(url) = settings.subject_url {
                let x509_extension_san = SubjectAlternativeName::new()
                    .critical()
                    .uri(&url)
                    .build(&x509v3_context)?;

                extensions.push(x509_extension_san);
            }
            //
            // TODO: uncomment once https://github.com/sfackler/rust-openssl/issues/1411
            // is fixed. This would allow to test also the parsing of the custom fields
            // added to certificate extensions
            //if let Some(subject_issuer) = settings.subject_issuer {
            //    let sigstore_issuer_asn1_obj = Asn1Object::from_str("1.3.6.1.4.1.57264.1.1")?; //&SIGSTORE_ISSUER_OID.to_string())?;

            //    let value = format!("ASN1:UTF8String:{}", subject_issuer);

            //    let sigstore_subject_issuer_extension = X509Extension::new_nid(
            //        None,
            //        Some(&x509v3_context),
            //        sigstore_issuer_asn1_obj.nid(),
            //        //&subject_issuer,
            //        &value,
            //    )?;

            //    extensions.push(sigstore_subject_issuer_extension);
            //}
        }

        for ext in extensions {
            x509_builder.append_extension(ext)?;
        }

        // setup validity
        let not_before = Asn1Time::from_unix(settings.not_before.timestamp())?;
        let not_after = Asn1Time::from_unix(settings.not_after.timestamp())?;
        x509_builder.set_not_after(&not_after)?;
        x509_builder.set_not_before(&not_before)?;

        // set issuer
        if let Some(issuer_data) = issuer {
            let issuer_name = issuer_data.cert.subject_name();
            x509_builder.set_issuer_name(&issuer_name)?;
        } else {
            // self signed cert
            x509_builder.set_issuer_name(&x509_name)?;
        }

        // sign the cert
        let issuer_key = match issuer {
            Some(issuer_data) => issuer_data.private_key.clone(),
            None => private_key.clone(),
        };
        let issuer_pkey = pkey::PKey::from_ec_key(issuer_key).expect("Cannot create signer pkey");
        x509_builder
            .sign(&issuer_pkey, MessageDigest::sha256())
            .expect("Cannot sign certificate");

        let x509 = x509_builder.build();

        Ok(CertData {
            cert: x509,
            private_key,
        })
    }
}
