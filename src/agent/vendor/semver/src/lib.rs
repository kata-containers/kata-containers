// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Semantic version parsing and comparison.
//!
//! Semantic versioning (see http://semver.org/) is a set of rules for
//! assigning version numbers.
//!
//! ## SemVer overview
//!
//! Given a version number MAJOR.MINOR.PATCH, increment the:
//!
//! 1. MAJOR version when you make incompatible API changes,
//! 2. MINOR version when you add functionality in a backwards-compatible
//!    manner, and
//! 3. PATCH version when you make backwards-compatible bug fixes.
//!
//! Additional labels for pre-release and build metadata are available as
//! extensions to the MAJOR.MINOR.PATCH format.
//!
//! Any references to 'the spec' in this documentation refer to [version 2.0 of
//! the SemVer spec](http://semver.org/spec/v2.0.0.html).
//!
//! ## SemVer and the Rust ecosystem
//!
//! Rust itself follows the SemVer specification, as does its standard
//! libraries. The two are not tied together.
//!
//! [Cargo](http://crates.io), Rust's package manager, uses SemVer to determine
//! which versions of packages you need installed.
//!
//! ## Versions
//!
//! At its simplest, the `semver` crate allows you to construct `Version`
//! objects using the `parse` method:
//!
//! ```{rust}
//! use semver::Version;
//!
//! assert!(Version::parse("1.2.3") == Ok(Version {
//!    major: 1,
//!    minor: 2,
//!    patch: 3,
//!    pre: vec!(),
//!    build: vec!(),
//! }));
//! ```
//!
//! If you have multiple `Version`s, you can use the usual comparison operators
//! to compare them:
//!
//! ```{rust}
//! use semver::Version;
//!
//! assert!(Version::parse("1.2.3-alpha") != Version::parse("1.2.3-beta"));
//! assert!(Version::parse("1.2.3-alpha2") >  Version::parse("1.2.0"));
//! ```
//!
//! If you explicitly need to modify a Version, SemVer also allows you to
//! increment the major, minor, and patch numbers in accordance with the spec.
//!
//! Please note that in order to do this, you must use a mutable Version:
//!
//! ```{rust}
//! use semver::Version;
//!
//! let mut bugfix_release = Version::parse("1.0.0").unwrap();
//! bugfix_release.increment_patch();
//!
//! assert_eq!(Ok(bugfix_release), Version::parse("1.0.1"));
//! ```
//!
//! When incrementing the minor version number, the patch number resets to zero
//! (in accordance with section 7 of the spec)
//!
//! ```{rust}
//! use semver::Version;
//!
//! let mut feature_release = Version::parse("1.4.6").unwrap();
//! feature_release.increment_minor();
//!
//! assert_eq!(Ok(feature_release), Version::parse("1.5.0"));
//! ```
//!
//! Similarly, when incrementing the major version number, the patch and minor
//! numbers reset to zero (in accordance with section 8 of the spec)
//!
//! ```{rust}
//! use semver::Version;
//!
//! let mut chrome_release = Version::parse("41.5.5377").unwrap();
//! chrome_release.increment_major();
//!
//! assert_eq!(Ok(chrome_release), Version::parse("42.0.0"));
//! ```
//!
//! ## Requirements
//!
//! The `semver` crate also provides the ability to compare requirements, which
//! are more complex comparisons.
//!
//! For example, creating a requirement that only matches versions greater than
//! or equal to 1.0.0:
//!
//! ```{rust}
//! # #![allow(unstable)]
//! use semver::Version;
//! use semver::VersionReq;
//!
//! let r = VersionReq::parse(">= 1.0.0").unwrap();
//! let v = Version::parse("1.0.0").unwrap();
//!
//! assert!(r.to_string() == ">= 1.0.0".to_string());
//! assert!(r.matches(&v))
//! ```
//!
//! It also allows parsing of `~x.y.z` and `^x.y.z` requirements as defined at
//! https://www.npmjs.org/doc/misc/semver.html
//!
//! **Tilde requirements** specify a minimal version with some updates:
//!
//! ```notrust
//! ~1.2.3 := >=1.2.3 <1.3.0
//! ~1.2   := >=1.2.0 <1.3.0
//! ~1     := >=1.0.0 <2.0.0
//! ```
//!
//! **Caret requirements** allow SemVer compatible updates to a specified
//! verion, `0.x` and `0.x+1` are not considered compatible, but `1.x` and
//! `1.x+1` are.
//!
//! `0.0.x` is not considered compatible with any other version.
//! Missing minor and patch versions are desugared to `0` but allow flexibility
//! for that value.
//!
//! ```notrust
//! ^1.2.3 := >=1.2.3 <2.0.0
//! ^0.2.3 := >=0.2.3 <0.3.0
//! ^0.0.3 := >=0.0.3 <0.0.4
//! ^0.0   := >=0.0.0 <0.1.0
//! ^0     := >=0.0.0 <1.0.0
//! ```
//!
//! **Wildcard requirements** allows parsing of version requirements of the
//! formats `*`, `x.*` and `x.y.*`.
//!
//! ```notrust
//! *     := >=0.0.0
//! 1.*   := >=1.0.0 <2.0.0
//! 1.2.* := >=1.2.0 <1.3.0
//! ```

#![doc(html_logo_url = "https://www.rust-lang.org/logos/rust-logo-128x128-blk-v2.png",
       html_favicon_url = "https://www.rust-lang.org/favicon.ico")]
#![deny(missing_docs)]
#![cfg_attr(test, deny(warnings))]

extern crate semver_parser;

// Serialization and deserialization support for version numbers
#[cfg(feature = "serde")]
extern crate serde;

// We take the common approach of keeping our own module system private, and
// just re-exporting the interface that we want.

pub use version::{Version, Identifier, SemVerError};
pub use version::Identifier::{Numeric, AlphaNumeric};
pub use version_req::{VersionReq, ReqParseError};

// SemVer-compliant versions.
mod version;

// advanced version comparisons
mod version_req;
