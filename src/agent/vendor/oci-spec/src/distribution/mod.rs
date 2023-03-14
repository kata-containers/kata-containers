//! [OCI distribution spec](https://github.com/opencontainers/distribution-spec) types and definitions.
//!
//! The Open Container Initiative Distribution Specification (a.k.a. "OCI Distribution Spec")
//! defines an API protocol to facilitate and standardize the distribution of content.
//!
//! While OCI Image is the most prominent, the specification is designed to be agnostic of content
//! types. Concepts such as "manifests" and "digests", are currently defined in the [Open Container
//! Initiative Image Format Specification](https://github.com/opencontainers/image-spec) (a.k.a.
//! "OCI Image Spec").
//!
//! To support other artifact types, please see the [Open Container Initiative Artifact Authors
//! Guide](https://github.com/opencontainers/artifacts) (a.k.a. "OCI Artifacts").

mod error;
mod repository;
mod tag;
mod version;

pub use error::*;
pub use repository::*;
pub use tag::*;
pub use version::*;
