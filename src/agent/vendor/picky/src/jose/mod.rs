//! JOSE framework subset implementation.
//!
//! A Json Web Token (JWT) comes in two flavors, roughly:
//! - Json Web Encryption (JWE), used to transfer data securely
//! - Json Web Signature (JWS), used to assert one's identity
//!
//! Common part is known as the "JOSE header".
//!
//! JSON Web Key (JWK) are used to represent cryptographic keys using JSON.

pub mod jwe;
pub mod jwk;
pub mod jws;
pub mod jwt;
