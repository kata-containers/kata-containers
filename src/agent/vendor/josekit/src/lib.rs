//! # josekit
//!
//! `josekit` is a JOSE (Javascript Object Signing and Encryption: JWT, JWS, JWE, JWA, JWK) library.

pub mod jwe;
pub mod jwk;
pub mod jws;
pub mod jwt;
pub mod util;

mod jose_error;
mod jose_header;

pub use crate::jose_error::JoseError;
pub use crate::jose_header::JoseHeader;

pub use serde_json::{Map, Number, Value};

#[cfg(doctest)]
use doc_comment::doctest;

#[cfg(doctest)]
doctest!("../README.md");
