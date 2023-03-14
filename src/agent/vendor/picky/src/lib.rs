//! [![Crates.io](https://img.shields.io/crates/v/picky.svg)](https://crates.io/crates/picky)
//! [![docs.rs](https://docs.rs/picky/badge.svg)](https://docs.rs/picky)
//! ![Crates.io](https://img.shields.io/crates/l/picky)
//! # picky
//!
//! Portable X.509, PKI, JOSE and HTTP signature implementation.

#[cfg(feature = "http_signature")]
pub mod http;

#[cfg(feature = "jose")]
pub mod jose;

#[cfg(feature = "x509")]
pub mod x509;

#[cfg(feature = "ssh")]
pub mod ssh;

pub mod hash;
pub mod key;
pub mod pem;
pub mod signature;

pub use picky_asn1_x509::{oids, AlgorithmIdentifier};

#[cfg(test)]
mod test_files {
    pub const RSA_2048_PK_1: &str = include_str!("../../test_assets/private_keys/rsa-2048-pk_1.key");
    pub const RSA_2048_PK_7: &str = include_str!("../../test_assets/private_keys/rsa-2048-pk_7.key");
    pub const RSA_4096_PK_3: &str = include_str!("../../test_assets/private_keys/rsa-4096-pk_3.key");

    cfg_if::cfg_if! { if  #[cfg(feature = "pkcs7")]  {
        pub const PKCS7: &str = include_str!("../../test_assets/pkcs7.p7b");
    }}

    cfg_if::cfg_if! { if #[cfg(feature = "ctl")] {
        pub const CERTIFICATE_TRUST_LIST: &[u8] = include_bytes!("../../test_assets/authroot.stl");
    }}

    cfg_if::cfg_if! { if #[cfg(feature = "x509")] {
        pub const RSA_2048_PK_2: &str =
            include_str!("../../test_assets/private_keys/rsa-2048-pk_2.key");
        pub const RSA_2048_PK_3: &str =
            include_str!("../../test_assets/private_keys/rsa-2048-pk_3.key");
        pub const RSA_2048_PK_4: &str =
            include_str!("../../test_assets/private_keys/rsa-2048-pk_4.key");

        pub const INTERMEDIATE_CA: &str = include_str!("../../test_assets/intermediate_ca.crt");
        pub const ROOT_CA: &str = include_str!("../../test_assets/root_ca.crt");

        pub const PSDIAG_ROOT: &str = include_str!("../../test_assets/authenticode-psdiagnostics/1_psdiag_root.pem");
        pub const PSDIAG_INTER: &str = include_str!("../../test_assets/authenticode-psdiagnostics/2_psdiag_inter.pem");
        pub const PSDIAG_LEAF: &str = include_str!("../../test_assets/authenticode-psdiagnostics/3_psdiag_leaf.pem");
    }}

    cfg_if::cfg_if! { if #[cfg(feature = "jose")] {
        pub const JOSE_JWT_SIG_EXAMPLE: &str =
            include_str!("../../test_assets/jose/jwt_sig_example.txt");
        pub const JOSE_JWT_SIG_WITH_EXP: &str =
            include_str!("../../test_assets/jose/jwt_sig_with_exp.txt");
        pub const JOSE_JWK_SET: &str =
            include_str!("../../test_assets/jose/jwk_set.json");
    }}
}
