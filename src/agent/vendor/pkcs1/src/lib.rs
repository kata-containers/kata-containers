#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_root_url = "https://docs.rs/pkcs1/0.3.3"
)]
#![forbid(unsafe_code, clippy::unwrap_used)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod error;
mod private_key;
mod public_key;
mod traits;
mod version;

pub use der::{
    self,
    asn1::{ObjectIdentifier, UIntBytes},
};

pub use self::{
    error::{Error, Result},
    private_key::RsaPrivateKey,
    public_key::RsaPublicKey,
    traits::{DecodeRsaPrivateKey, DecodeRsaPublicKey},
    version::Version,
};

#[cfg(feature = "alloc")]
pub use crate::{
    private_key::{
        document::RsaPrivateKeyDocument, other_prime_info::OtherPrimeInfo, OtherPrimeInfos,
    },
    public_key::document::RsaPublicKeyDocument,
    traits::{EncodeRsaPrivateKey, EncodeRsaPublicKey},
};

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
pub use der::pem::{self, LineEnding};

/// `rsaEncryption` Object Identifier (OID)
#[cfg(feature = "pkcs8")]
#[cfg_attr(docsrs, doc(cfg(feature = "pkcs8")))]
pub const ALGORITHM_OID: ObjectIdentifier = ObjectIdentifier::new("1.2.840.113549.1.1.1");

/// `AlgorithmIdentifier` for RSA.
#[cfg(feature = "pkcs8")]
#[cfg_attr(docsrs, doc(cfg(feature = "pkcs8")))]
pub const ALGORITHM_ID: pkcs8::AlgorithmIdentifier<'static> = pkcs8::AlgorithmIdentifier {
    oid: ALGORITHM_OID,
    parameters: Some(der::asn1::Any::NULL),
};
