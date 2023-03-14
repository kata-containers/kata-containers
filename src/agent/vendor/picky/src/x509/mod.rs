//! X.509 certificates implementation based on [RFC5280](https://tools.ietf.org/html/rfc5280)
//!
//! ## Example
//!
//! ```rust
//! use picky::key::PrivateKey;
//! use picky::pem::{parse_pem, Pem};
//! use picky::{signature::SignatureAlgorithm, hash::HashAlgorithm};
//! use picky::x509::key_id_gen_method::{KeyIdGenError, KeyIdGenMethod};
//! use picky::x509::{certificate::CertType, csr::Csr};
//! use picky::x509::{name::DirectoryName, date::UtcDate};
//! use picky::x509::certificate::CertificateBuilder;
//!
//!# use std::error::Error;
//!# fn main() -> Result<(), Box<dyn Error>> {
//!# let root_key_pem_str =
//!#    "-----BEGIN PRIVATE KEY-----\n\
//!#     MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQCzl5R1QzQGznTe\n\
//!#     Jw1STzQz1ZOrbG58ZWdJJDOrjBB60PO+MgOIxDzn41p4W/OeqmzhxUPyzs9d3iAx\n\
//!#     RcF8gWkS8QvQXtHQtFlux61oE8GHvK9pqrEERQKjjTN/yculekauopAMQOcGDS1L\n\
//!#     t55z5oByunc/ZQYKHIZ82zfuw/VyKOdJ31/fXn2Djhnfq19YE8pgBDGxEPqCRv7S\n\
//!#     t/ThptUD+Xr3jwR2ycemq/OVLmsTETh8fYeGrcDIx2EsjL18ptPOg6rZly/UNN+w\n\
//!#     Sy8L5HlJqzDj03ytYTkgpT442TK3eGpxwX5uaHF7GuL6l9xOIDH+FuPKMt1huFWC\n\
//!#     Y+hHC8yXAgMBAAECggEATpCbM4lIs0mSOrUe0MkxbxOZ3D49P1IlEwwxRnSpqPDM\n\
//!#     EIJUpXojotYE9+5R7zFjHIltpkTu1oOb1bJMaKk8TDsDDg7M/t5G2X2k0IkD+tuC\n\
//!#     aVlwwwtlkiLtSpP+8M9DKJaZKaMi7bTKAfu757wIqSgFjsglEjtUYj6Mg3laYKvk\n\
//!#     EoR6NsOojR1tOhqGfPFJTH21ZBWDmFHhsyRHXyZOQ9I2u8pv5l33yasgmuZLZ2g3\n\
//!#     Hok6j0uU/fPpjSRTu6khVZV8+stjnFaHezTWGsz/CLqzC1EWJ1icFTb7UwafyWts\n\
//!#     EHgdMjqpu2qfqB+miFElhld4fdZIoBWTQwCdN7MnUQKBgQDnBIK6RUJYUjcX9Nei\n\
//!#     Qvf7Kl3sEY+QTQdI4na3puXjOk6eFqTUAbdsKVwU/yJyR3L+bQNTUmy3GdPlZkYt\n\
//!#     mx18fGpq9t8whDIFZG9jI5YX4/vEMOHa9zY9FI3QLpIgbifIBEIR76UVe/xakT+I\n\
//!#     bx8C1p/9msDuqVBJm7pZLIX3DQKBgQDHA2ftRkQVL0lOmvwFl56n2gbnnk0KAZ/8\n\
//!#     LyuN9FvadqzbpXQqfEUS5f1cfM4JSAXj2e9POBw2FWD1uFROIGPrPEAfSqInI6G9\n\
//!#     i/MSeBwlcvTXmWMphVGCnJwmCPlznT0Vizr5wIBImgUOpXZpxAqUQUvxOl4R4VYI\n\
//!#     gLW8iFSpMwKBgCAzacFrC/9hnlpRf4kXipdQ5XyVSgyUIBLRtjiNI/gTVYgFof4H\n\
//!#     KzkBXttyYKvLN8UtDsybbZnsGLQeGGQc/fQvJ2o3dQ6/LnW/9SK9gBteZOaI5cJu\n\
//!#     uPm0lrvQ8f9hO1xO86KqY7ll6dv56QAsdQchQXXJD2F06kMIWOY7JYU1AoGAJSKg\n\
//!#     kOjsqVtSfYV0A4MgSsfnQ+8JBxX4iXEv2mQ/g4tjg/TisU7RAM7DsS9krtyupK60\n\
//!#     9f5NXVYt6owDxzRKEMoEWRJvIYiHlLv5lnetINvLjjOECmpjJFEe3gYMriMoE84Q\n\
//!#     Kixeg62hxfLgHqpDIxjwF8pBZWq7yAhkYRK7YXsCgYAxqdZwpOYyntEBIQa3mOE8\n\
//!#     a2mthICUXKE30OkMJdrbP3qlFECWZcvKspPHzXWiy8cP9dS2D06ax/UBZmKGXUFv\n\
//!#     D48nECaQ2glgMPLyyd+tznXed9nZVYBlOFjUQ0fvfpsmhp975n1b/UxAg1jqvigK\n\
//!#     YHWKXfDNtZ9Ci6BMYVELsw==\n\
//!#     -----END PRIVATE KEY-----";
//!# let intermediate_key_pem_str = root_key_pem_str;
//!# let leaf_key_pem_str = root_key_pem_str;
//!#
//! // Generate a self-signed root certificate
//!
//! // Load private key
//! let root_key = PrivateKey::from_pem_str(root_key_pem_str)?;
//!
//! let root = CertificateBuilder::new()
//!     .validity(UtcDate::ymd(2020, 9, 28).unwrap(), UtcDate::ymd(2023, 9, 28).unwrap())
//!     .self_signed(DirectoryName::new_common_name("My Root CA"), &root_key)
//!     .ca(true)
//!     .signature_hash_type(SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_512))
//!     .key_id_gen_method(KeyIdGenMethod::SPKFullDER(HashAlgorithm::SHA2_384))
//!     .build()?;
//!
//! assert_eq!(root.ty(), CertType::Root);
//!
//! // Generate intermediate certificate signed by root CA
//!
//! let intermediate_key = PrivateKey::from_pem_str(intermediate_key_pem_str)?;
//!
//! let intermediate = CertificateBuilder::new()
//!     .validity(UtcDate::ymd(2020, 10, 15).unwrap(), UtcDate::ymd(2021, 10, 15).unwrap())
//!     .subject(DirectoryName::new_common_name("My Authority"), intermediate_key.to_public_key())
//!     .issuer_cert(&root, &root_key)
//!     .signature_hash_type(SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_224))
//!     .key_id_gen_method(KeyIdGenMethod::SPKValueHashedLeftmost160(HashAlgorithm::SHA1))
//!     .ca(true)
//!     .pathlen(0)
//!     .build()?;
//!
//! assert_eq!(intermediate.ty(), CertType::Intermediate);
//!
//! // Generate leaf certificate signed by intermediate authority
//!
//! let leaf_key = PrivateKey::from_pem_str(leaf_key_pem_str)?;
//!
//! // A CSR can be used
//! let csr = Csr::generate(
//!     DirectoryName::new_common_name("My Leaf"),
//!     &leaf_key,
//!     SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA1),
//! )?;
//!
//! let signed_leaf = CertificateBuilder::new()
//!     .validity(UtcDate::ymd(2020, 11, 1).unwrap(), UtcDate::ymd(2021, 1, 1).unwrap())
//!     .subject_from_csr(csr)
//!     .issuer_cert(&intermediate, &intermediate_key)
//!     .signature_hash_type(SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_384))
//!     .key_id_gen_method(KeyIdGenMethod::SPKFullDER(HashAlgorithm::SHA2_512))
//!     .build()?;
//!
//! assert_eq!(signed_leaf.ty(), CertType::Leaf);
//!
//! // Check leaf using CA chain
//!
//! let chain = [intermediate, root];
//!
//! signed_leaf
//!     .verifier()
//!     .chain(chain.iter())
//!     .exact_date(&UtcDate::ymd(2020, 12, 20).unwrap())
//!     .verify()?;
//!
//! // If `not_after` date is behind…
//!
//! let err = signed_leaf
//!     .verifier()
//!     .chain(chain.iter())
//!     .exact_date(&UtcDate::ymd(2021, 1, 2).unwrap())
//!     .verify()
//!     .err()
//!     .unwrap();
//!
//! assert_eq!(
//!     err.to_string(),
//!     "invalid certificate \'CN=My Leaf\': \
//!      certificate expired (not after: 2021-01-01 00:00:00, now: 2021-01-02 00:00:00)"
//! );
//!#
//!# Ok(())
//!# }
//! ```

#[cfg(feature = "pkcs7")]
pub mod pkcs7;

#[cfg(feature = "wincert")]
pub mod wincert;

pub mod certificate;
pub mod csr;
pub mod date;
pub mod key_id_gen_method;
pub mod name;

pub use certificate::Cert;
pub use csr::Csr;
pub use key_id_gen_method::KeyIdGenMethod;
pub use picky_asn1_x509::{DirectoryString, Extension, Extensions};

pub mod extension {
    pub use picky_asn1_x509::extension::*;
}

mod utils;
