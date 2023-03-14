//! Brings most relevant types and traits into scope for working with
//! certificates.
//!
//! Less often used types and traits that are more likely to lead to a
//! naming conflict are not brought into scope.
//!
//! Traits are brought into scope anonymously.
//!
//! ```
//! # #![allow(unused_imports)]
//! # use sequoia_openpgp as openpgp;
//! use openpgp::cert::prelude::*;
//! ```

#![allow(unused_imports)]
pub use crate::cert::{
    Cert,
    CertBuilder,
    CertParser,
    CertRevocationBuilder,
    CipherSuite,
    KeyBuilder,
    SubkeyBuilder,
    Preferences as _,
    SubkeyRevocationBuilder,
    UserAttributeRevocationBuilder,
    UserIDRevocationBuilder,
    ValidCert,
    amalgamation::ComponentAmalgamation,
    amalgamation::ComponentAmalgamationIter,
    amalgamation::UnknownComponentAmalgamation,
    amalgamation::UnknownComponentAmalgamationIter,
    amalgamation::UserAttributeAmalgamation,
    amalgamation::UserAttributeAmalgamationIter,
    amalgamation::UserIDAmalgamation,
    amalgamation::UserIDAmalgamationIter,
    amalgamation::ValidAmalgamation as _,
    amalgamation::ValidComponentAmalgamation,
    amalgamation::ValidComponentAmalgamationIter,
    amalgamation::ValidUserAttributeAmalgamation,
    amalgamation::ValidUserAttributeAmalgamationIter,
    amalgamation::ValidUserIDAmalgamation,
    amalgamation::ValidUserIDAmalgamationIter,
    amalgamation::ValidateAmalgamation as _,
    amalgamation::key::ErasedKeyAmalgamation,
    amalgamation::key::KeyAmalgamation,
    amalgamation::key::KeyAmalgamationIter,
    amalgamation::key::PrimaryKey as _,
    amalgamation::key::PrimaryKeyAmalgamation,
    amalgamation::key::SubordinateKeyAmalgamation,
    amalgamation::key::ValidErasedKeyAmalgamation,
    amalgamation::key::ValidKeyAmalgamation,
    amalgamation::key::ValidKeyAmalgamationIter,
    amalgamation::key::ValidPrimaryKeyAmalgamation,
    amalgamation::key::ValidSubordinateKeyAmalgamation,
    bundle::ComponentBundle,
    bundle::KeyBundle,
    bundle::PrimaryKeyBundle,
    bundle::SubkeyBundle,
    bundle::UnknownBundle,
    bundle::UserAttributeBundle,
    bundle::UserIDBundle,
};
