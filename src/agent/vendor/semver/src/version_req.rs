// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::error::Error;
use std::fmt;
use std::result;
use std::str;

use Version;
use version::Identifier;
use semver_parser;

#[cfg(feature = "serde")]
use serde::ser::{Serialize, Serializer};
#[cfg(feature = "serde")]
use serde::de::{self, Deserialize, Deserializer, Visitor};

use self::Op::{Ex, Gt, GtEq, Lt, LtEq, Tilde, Compatible, Wildcard};
use self::WildcardVersion::{Major, Minor, Patch};
use self::ReqParseError::*;

/// A `VersionReq` is a struct containing a list of predicates that can apply to ranges of version
/// numbers. Matching operations can then be done with the `VersionReq` against a particular
/// version to see if it satisfies some or all of the constraints.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct VersionReq {
    predicates: Vec<Predicate>,
}

impl From<semver_parser::range::VersionReq> for VersionReq {
    fn from(other: semver_parser::range::VersionReq) -> VersionReq {
        VersionReq { predicates: other.predicates.into_iter().map(From::from).collect() }
    }
}

#[cfg(feature = "serde")]
impl Serialize for VersionReq {
    fn serialize<S>(&self, serializer: S) -> result::Result<S::Ok, S::Error>
        where S: Serializer
    {
        // Serialize VersionReq as a string.
        serializer.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for VersionReq {
    fn deserialize<D>(deserializer: D) -> result::Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        struct VersionReqVisitor;

        /// Deserialize `VersionReq` from a string.
        impl<'de> Visitor<'de> for VersionReqVisitor {
            type Value = VersionReq;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a SemVer version requirement as a string")
            }

            fn visit_str<E>(self, v: &str) -> result::Result<Self::Value, E>
                where E: de::Error
            {
                VersionReq::parse(v).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_str(VersionReqVisitor)
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum WildcardVersion {
    Major,
    Minor,
    Patch,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum Op {
    Ex, // Exact
    Gt, // Greater than
    GtEq, // Greater than or equal to
    Lt, // Less than
    LtEq, // Less than or equal to
    Tilde, // e.g. ~1.0.0
    Compatible, // compatible by definition of semver, indicated by ^
    Wildcard(WildcardVersion), // x.y.*, x.*, *
}

impl From<semver_parser::range::Op> for Op {
    fn from(other: semver_parser::range::Op) -> Op {
        use semver_parser::range;
        match other {
            range::Op::Ex => Op::Ex,
            range::Op::Gt => Op::Gt,
            range::Op::GtEq => Op::GtEq,
            range::Op::Lt => Op::Lt,
            range::Op::LtEq => Op::LtEq,
            range::Op::Tilde => Op::Tilde,
            range::Op::Compatible => Op::Compatible,
            range::Op::Wildcard(version) => {
                match version {
                    range::WildcardVersion::Major => Op::Wildcard(WildcardVersion::Major),
                    range::WildcardVersion::Minor => Op::Wildcard(WildcardVersion::Minor),
                    range::WildcardVersion::Patch => Op::Wildcard(WildcardVersion::Patch),
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
struct Predicate {
    op: Op,
    major: u64,
    minor: Option<u64>,
    patch: Option<u64>,
    pre: Vec<Identifier>,
}

impl From<semver_parser::range::Predicate> for Predicate {
    fn from(other: semver_parser::range::Predicate) -> Predicate {
        Predicate {
            op: From::from(other.op),
            major: other.major,
            minor: other.minor,
            patch: other.patch,
            pre: other.pre.into_iter().map(From::from).collect(),
        }
    }
}

/// A `ReqParseError` is returned from methods which parse a string into a `VersionReq`. Each
/// enumeration is one of the possible errors that can occur.
#[derive(Clone, Debug, PartialEq)]
pub enum ReqParseError {
    /// The given version requirement is invalid.
    InvalidVersionRequirement,
    /// You have already provided an operation, such as `=`, `~`, or `^`. Only use one.
    OpAlreadySet,
    /// The sigil you have written is not correct.
    InvalidSigil,
    /// All components of a version must be numeric.
    VersionComponentsMustBeNumeric,
    /// There was an error parsing an identifier.
    InvalidIdentifier,
    /// At least a major version is required.
    MajorVersionRequired,
    /// An unimplemented version requirement.
    UnimplementedVersionRequirement,
    /// This form of requirement is deprecated.
    DeprecatedVersionRequirement(VersionReq),
}

impl fmt::Display for ReqParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.description().fmt(f)
    }
}

impl Error for ReqParseError {
    fn description(&self) -> &str {
        match self {
            &InvalidVersionRequirement => "the given version requirement is invalid",
            &OpAlreadySet => {
                "you have already provided an operation, such as =, ~, or ^; only use one"
            },
            &InvalidSigil => "the sigil you have written is not correct",
            &VersionComponentsMustBeNumeric => "version components must be numeric",
            &InvalidIdentifier => "invalid identifier",
            &MajorVersionRequired => "at least a major version number is required",
            &UnimplementedVersionRequirement => {
                "the given version requirement is not implemented, yet"
            },
            &DeprecatedVersionRequirement(_) => "This requirement is deprecated",
        }
    }
}

impl From<String> for ReqParseError {
    fn from(other: String) -> ReqParseError {
        match &*other {
            "Null is not a valid VersionReq" => ReqParseError::InvalidVersionRequirement,
            "VersionReq did not parse properly." => ReqParseError::OpAlreadySet,
            _ => ReqParseError::InvalidVersionRequirement,
        }
    }
}

impl VersionReq {
    /// `any()` is a factory method which creates a `VersionReq` with no constraints. In other
    /// words, any version will match against it.
    ///
    /// # Examples
    ///
    /// ```
    /// use semver::VersionReq;
    ///
    /// let anything = VersionReq::any();
    /// ```
    pub fn any() -> VersionReq {
        VersionReq { predicates: vec![] }
    }

    /// `parse()` is the main constructor of a `VersionReq`. It takes a string like `"^1.2.3"`
    /// and turns it into a `VersionReq` that matches that particular constraint.
    ///
    /// A `Result` is returned which contains a `ReqParseError` if there was a problem parsing the
    /// `VersionReq`.
    ///
    /// # Examples
    ///
    /// ```
    /// use semver::VersionReq;
    ///
    /// let version = VersionReq::parse("=1.2.3");
    /// let version = VersionReq::parse(">1.2.3");
    /// let version = VersionReq::parse("<1.2.3");
    /// let version = VersionReq::parse("~1.2.3");
    /// let version = VersionReq::parse("^1.2.3");
    /// let version = VersionReq::parse("1.2.3"); // synonym for ^1.2.3
    /// let version = VersionReq::parse("<=1.2.3");
    /// let version = VersionReq::parse(">=1.2.3");
    /// ```
    ///
    /// This example demonstrates error handling, and will panic.
    ///
    /// ```should-panic
    /// use semver::VersionReq;
    ///
    /// let version = match VersionReq::parse("not a version") {
    ///     Ok(version) => version,
    ///     Err(e) => panic!("There was a problem parsing: {}", e),
    /// }
    /// ```
    pub fn parse(input: &str) -> Result<VersionReq, ReqParseError> {
        let res = semver_parser::range::parse(input);

        if let Ok(v) = res {
            return Ok(From::from(v));
        }

        return match VersionReq::parse_deprecated(input) {
            Some(v) => {
                Err(ReqParseError::DeprecatedVersionRequirement(v))
            }
            None => Err(From::from(res.err().unwrap())),
        }
    }

    fn parse_deprecated(version: &str) -> Option<VersionReq> {
        return match version {
            ".*" => Some(VersionReq::any()),
            "0.1.0." => Some(VersionReq::parse("0.1.0").unwrap()),
            "0.3.1.3" => Some(VersionReq::parse("0.3.13").unwrap()),
            "0.2*" => Some(VersionReq::parse("0.2.*").unwrap()),
            "*.0" => Some(VersionReq::any()),
            _ => None,
        }
    }

    /// `exact()` is a factory method which creates a `VersionReq` with one exact constraint.
    ///
    /// # Examples
    ///
    /// ```
    /// use semver::VersionReq;
    /// use semver::Version;
    ///
    /// let version = Version { major: 1, minor: 1, patch: 1, pre: vec![], build: vec![] };
    /// let exact = VersionReq::exact(&version);
    /// ```
    pub fn exact(version: &Version) -> VersionReq {
        VersionReq { predicates: vec![Predicate::exact(version)] }
    }

    /// `matches()` matches a given `Version` against this `VersionReq`.
    ///
    /// # Examples
    ///
    /// ```
    /// use semver::VersionReq;
    /// use semver::Version;
    ///
    /// let version = Version { major: 1, minor: 1, patch: 1, pre: vec![], build: vec![] };
    /// let exact = VersionReq::exact(&version);
    ///
    /// assert!(exact.matches(&version));
    /// ```
    pub fn matches(&self, version: &Version) -> bool {
        // no predicates means anything matches
        if self.predicates.is_empty() {
            return true;
        }

        self.predicates.iter().all(|p| p.matches(version)) &&
        self.predicates.iter().any(|p| p.pre_tag_is_compatible(version))
    }
}

impl str::FromStr for VersionReq {
    type Err = ReqParseError;

    fn from_str(s: &str) -> Result<VersionReq, ReqParseError> {
        VersionReq::parse(s)
    }
}

impl Predicate {
    fn exact(version: &Version) -> Predicate {
        Predicate {
            op: Ex,
            major: version.major,
            minor: Some(version.minor),
            patch: Some(version.patch),
            pre: version.pre.clone(),
        }
    }

    /// `matches()` takes a `Version` and determines if it matches this particular `Predicate`.
    pub fn matches(&self, ver: &Version) -> bool {
        match self.op {
            Ex => self.is_exact(ver),
            Gt => self.is_greater(ver),
            GtEq => self.is_exact(ver) || self.is_greater(ver),
            Lt => !self.is_exact(ver) && !self.is_greater(ver),
            LtEq => !self.is_greater(ver),
            Tilde => self.matches_tilde(ver),
            Compatible => self.is_compatible(ver),
            Wildcard(_) => self.matches_wildcard(ver),
        }
    }

    fn is_exact(&self, ver: &Version) -> bool {
        if self.major != ver.major {
            return false;
        }

        match self.minor {
            Some(minor) => {
                if minor != ver.minor {
                    return false;
                }
            }
            None => return true,
        }

        match self.patch {
            Some(patch) => {
                if patch != ver.patch {
                    return false;
                }
            }
            None => return true,
        }

        if self.pre != ver.pre {
            return false;
        }

        true
    }

    // https://docs.npmjs.com/misc/semver#prerelease-tags
    fn pre_tag_is_compatible(&self, ver: &Version) -> bool {
        // If a version has a prerelease tag (for example, 1.2.3-alpha.3) then it will
        // only be
        // allowed to satisfy comparator sets if at least one comparator with the same
        // [major,
        // minor, patch] tuple also has a prerelease tag.
        !ver.is_prerelease() ||
        (self.major == ver.major && self.minor == Some(ver.minor) &&
         self.patch == Some(ver.patch) && !self.pre.is_empty())
    }

    fn is_greater(&self, ver: &Version) -> bool {
        if self.major != ver.major {
            return ver.major > self.major;
        }

        match self.minor {
            Some(minor) => {
                if minor != ver.minor {
                    return ver.minor > minor;
                }
            }
            None => return false,
        }

        match self.patch {
            Some(patch) => {
                if patch != ver.patch {
                    return ver.patch > patch;
                }
            }
            None => return false,
        }

        if !self.pre.is_empty() {
            return ver.pre.is_empty() || ver.pre > self.pre;
        }

        false
    }

    // see https://www.npmjs.org/doc/misc/semver.html for behavior
    fn matches_tilde(&self, ver: &Version) -> bool {
        let minor = match self.minor {
            Some(n) => n,
            None => return self.major == ver.major,
        };

        match self.patch {
            Some(patch) => {
                self.major == ver.major && minor == ver.minor &&
                (ver.patch > patch || (ver.patch == patch && self.pre_is_compatible(ver)))
            }
            None => self.major == ver.major && minor == ver.minor,
        }
    }

    // see https://www.npmjs.org/doc/misc/semver.html for behavior
    fn is_compatible(&self, ver: &Version) -> bool {
        if self.major != ver.major {
            return false;
        }

        let minor = match self.minor {
            Some(n) => n,
            None => return self.major == ver.major,
        };

        match self.patch {
            Some(patch) => {
                if self.major == 0 {
                    if minor == 0 {
                        ver.minor == minor && ver.patch == patch && self.pre_is_compatible(ver)
                    } else {
                        ver.minor == minor &&
                        (ver.patch > patch || (ver.patch == patch && self.pre_is_compatible(ver)))
                    }
                } else {
                    ver.minor > minor ||
                    (ver.minor == minor &&
                     (ver.patch > patch || (ver.patch == patch && self.pre_is_compatible(ver))))
                }
            }
            None => {
                if self.major == 0 {
                    ver.minor == minor
                } else {
                    ver.minor >= minor
                }
            }
        }
    }

    fn pre_is_compatible(&self, ver: &Version) -> bool {
        ver.pre.is_empty() || ver.pre >= self.pre
    }

    // see https://www.npmjs.org/doc/misc/semver.html for behavior
    fn matches_wildcard(&self, ver: &Version) -> bool {
        match self.op {
            Wildcard(Major) => true,
            Wildcard(Minor) => self.major == ver.major,
            Wildcard(Patch) => {
                match self.minor {
                    Some(minor) => self.major == ver.major && minor == ver.minor,
                    None => {
                        // minor and patch version astericks mean match on major
                        self.major == ver.major
                    }
                }
            }
            _ => false,  // unreachable
        }
    }
}

impl fmt::Display for VersionReq {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if self.predicates.is_empty() {
            try!(write!(fmt, "*"));
        } else {
            for (i, ref pred) in self.predicates.iter().enumerate() {
                if i == 0 {
                    try!(write!(fmt, "{}", pred));
                } else {
                    try!(write!(fmt, ", {}", pred));
                }
            }
        }

        Ok(())
    }
}

impl fmt::Display for Predicate {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self.op {
            Wildcard(Major) => try!(write!(fmt, "*")),
            Wildcard(Minor) => try!(write!(fmt, "{}.*", self.major)),
            Wildcard(Patch) => {
                if let Some(minor) = self.minor {
                    try!(write!(fmt, "{}.{}.*", self.major, minor))
                } else {
                    try!(write!(fmt, "{}.*.*", self.major))
                }
            }
            _ => {
                try!(write!(fmt, "{}{}", self.op, self.major));

                match self.minor {
                    Some(v) => try!(write!(fmt, ".{}", v)),
                    None => (),
                }

                match self.patch {
                    Some(v) => try!(write!(fmt, ".{}", v)),
                    None => (),
                }

                if !self.pre.is_empty() {
                    try!(write!(fmt, "-"));
                    for (i, x) in self.pre.iter().enumerate() {
                        if i != 0 {
                            try!(write!(fmt, "."))
                        }
                        try!(write!(fmt, "{}", x));
                    }
                }
            }
        }

        Ok(())
    }
}

impl fmt::Display for Op {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Ex => try!(write!(fmt, "= ")),
            Gt => try!(write!(fmt, "> ")),
            GtEq => try!(write!(fmt, ">= ")),
            Lt => try!(write!(fmt, "< ")),
            LtEq => try!(write!(fmt, "<= ")),
            Tilde => try!(write!(fmt, "~")),
            Compatible => try!(write!(fmt, "^")),
            // gets handled specially in Predicate::fmt
            Wildcard(_) => try!(write!(fmt, "")),
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::{VersionReq, Op};
    use super::super::version::Version;
    use std::hash::{Hash, Hasher};

    fn req(s: &str) -> VersionReq {
        VersionReq::parse(s).unwrap()
    }

    fn version(s: &str) -> Version {
        match Version::parse(s) {
            Ok(v) => v,
            Err(e) => panic!("`{}` is not a valid version. Reason: {:?}", s, e),
        }
    }

    fn assert_match(req: &VersionReq, vers: &[&str]) {
        for ver in vers.iter() {
            assert!(req.matches(&version(*ver)), "did not match {}", ver);
        }
    }

    fn assert_not_match(req: &VersionReq, vers: &[&str]) {
        for ver in vers.iter() {
            assert!(!req.matches(&version(*ver)), "matched {}", ver);
        }
    }

    fn calculate_hash<T: Hash>(t: T) -> u64 {
        use std::collections::hash_map::DefaultHasher;

        let mut s = DefaultHasher::new();
        t.hash(&mut s);
        s.finish()
    }

    #[test]
    fn test_parsing_default() {
        let r = req("1.0.0");

        assert_eq!(r.to_string(), "^1.0.0".to_string());

        assert_match(&r, &["1.0.0", "1.0.1"]);
        assert_not_match(&r, &["0.9.9", "0.10.0", "0.1.0"]);
    }

    #[test]
    fn test_parsing_exact() {
        let r = req("=1.0.0");

        assert!(r.to_string() == "= 1.0.0".to_string());
        assert_eq!(r.to_string(), "= 1.0.0".to_string());

        assert_match(&r, &["1.0.0"]);
        assert_not_match(&r, &["1.0.1", "0.9.9", "0.10.0", "0.1.0", "1.0.0-pre"]);

        let r = req("=0.9.0");

        assert_eq!(r.to_string(), "= 0.9.0".to_string());

        assert_match(&r, &["0.9.0"]);
        assert_not_match(&r, &["0.9.1", "1.9.0", "0.0.9"]);

        let r = req("=0.1.0-beta2.a");

        assert_eq!(r.to_string(), "= 0.1.0-beta2.a".to_string());

        assert_match(&r, &["0.1.0-beta2.a"]);
        assert_not_match(&r, &["0.9.1", "0.1.0", "0.1.1-beta2.a", "0.1.0-beta2"]);
    }

    #[test]
    fn test_parse_metadata_see_issue_88_see_issue_88() {
        for op in &[Op::Compatible, Op::Ex, Op::Gt, Op::GtEq, Op::Lt, Op::LtEq, Op::Tilde] {
            req(&format!("{} 1.2.3+meta", op));
        }
    }

    #[test]
    pub fn test_parsing_greater_than() {
        let r = req(">= 1.0.0");

        assert_eq!(r.to_string(), ">= 1.0.0".to_string());

        assert_match(&r, &["1.0.0", "2.0.0"]);
        assert_not_match(&r, &["0.1.0", "0.0.1", "1.0.0-pre", "2.0.0-pre"]);

        let r = req(">= 2.1.0-alpha2");

        assert_match(&r, &["2.1.0-alpha2", "2.1.0-alpha3", "2.1.0", "3.0.0"]);
        assert_not_match(&r,
                         &["2.0.0", "2.1.0-alpha1", "2.0.0-alpha2", "3.0.0-alpha2"]);
    }

    #[test]
    pub fn test_parsing_less_than() {
        let r = req("< 1.0.0");

        assert_eq!(r.to_string(), "< 1.0.0".to_string());

        assert_match(&r, &["0.1.0", "0.0.1"]);
        assert_not_match(&r, &["1.0.0", "1.0.0-beta", "1.0.1", "0.9.9-alpha"]);

        let r = req("<= 2.1.0-alpha2");

        assert_match(&r, &["2.1.0-alpha2", "2.1.0-alpha1", "2.0.0", "1.0.0"]);
        assert_not_match(&r,
                         &["2.1.0", "2.2.0-alpha1", "2.0.0-alpha2", "1.0.0-alpha2"]);
    }

    #[test]
    pub fn test_multiple() {
        let r = req("> 0.0.9, <= 2.5.3");
        assert_eq!(r.to_string(), "> 0.0.9, <= 2.5.3".to_string());
        assert_match(&r, &["0.0.10", "1.0.0", "2.5.3"]);
        assert_not_match(&r, &["0.0.8", "2.5.4"]);

        let r = req("0.3.0, 0.4.0");
        assert_eq!(r.to_string(), "^0.3.0, ^0.4.0".to_string());
        assert_not_match(&r, &["0.0.8", "0.3.0", "0.4.0"]);

        let r = req("<= 0.2.0, >= 0.5.0");
        assert_eq!(r.to_string(), "<= 0.2.0, >= 0.5.0".to_string());
        assert_not_match(&r, &["0.0.8", "0.3.0", "0.5.1"]);

        let r = req("0.1.0, 0.1.4, 0.1.6");
        assert_eq!(r.to_string(), "^0.1.0, ^0.1.4, ^0.1.6".to_string());
        assert_match(&r, &["0.1.6", "0.1.9"]);
        assert_not_match(&r, &["0.1.0", "0.1.4", "0.2.0"]);

        assert!(VersionReq::parse("> 0.1.0,").is_err());
        assert!(VersionReq::parse("> 0.3.0, ,").is_err());

        let r = req(">=0.5.1-alpha3, <0.6");
        assert_eq!(r.to_string(), ">= 0.5.1-alpha3, < 0.6".to_string());
        assert_match(&r,
                     &["0.5.1-alpha3", "0.5.1-alpha4", "0.5.1-beta", "0.5.1", "0.5.5"]);
        assert_not_match(&r,
                         &["0.5.1-alpha1", "0.5.2-alpha3", "0.5.5-pre", "0.5.0-pre"]);
        assert_not_match(&r, &["0.6.0", "0.6.0-pre"]);
    }

    #[test]
    pub fn test_parsing_tilde() {
        let r = req("~1");
        assert_match(&r, &["1.0.0", "1.0.1", "1.1.1"]);
        assert_not_match(&r, &["0.9.1", "2.9.0", "0.0.9"]);

        let r = req("~1.2");
        assert_match(&r, &["1.2.0", "1.2.1"]);
        assert_not_match(&r, &["1.1.1", "1.3.0", "0.0.9"]);

        let r = req("~1.2.2");
        assert_match(&r, &["1.2.2", "1.2.4"]);
        assert_not_match(&r, &["1.2.1", "1.9.0", "1.0.9", "2.0.1", "0.1.3"]);

        let r = req("~1.2.3-beta.2");
        assert_match(&r, &["1.2.3", "1.2.4", "1.2.3-beta.2", "1.2.3-beta.4"]);
        assert_not_match(&r, &["1.3.3", "1.1.4", "1.2.3-beta.1", "1.2.4-beta.2"]);
    }

    #[test]
    pub fn test_parsing_compatible() {
        let r = req("^1");
        assert_match(&r, &["1.1.2", "1.1.0", "1.2.1", "1.0.1"]);
        assert_not_match(&r, &["0.9.1", "2.9.0", "0.1.4"]);
        assert_not_match(&r, &["1.0.0-beta1", "0.1.0-alpha", "1.0.1-pre"]);

        let r = req("^1.1");
        assert_match(&r, &["1.1.2", "1.1.0", "1.2.1"]);
        assert_not_match(&r, &["0.9.1", "2.9.0", "1.0.1", "0.1.4"]);

        let r = req("^1.1.2");
        assert_match(&r, &["1.1.2", "1.1.4", "1.2.1"]);
        assert_not_match(&r, &["0.9.1", "2.9.0", "1.1.1", "0.0.1"]);
        assert_not_match(&r, &["1.1.2-alpha1", "1.1.3-alpha1", "2.9.0-alpha1"]);

        let r = req("^0.1.2");
        assert_match(&r, &["0.1.2", "0.1.4"]);
        assert_not_match(&r, &["0.9.1", "2.9.0", "1.1.1", "0.0.1"]);
        assert_not_match(&r, &["0.1.2-beta", "0.1.3-alpha", "0.2.0-pre"]);

        let r = req("^0.5.1-alpha3");
        assert_match(&r,
                     &["0.5.1-alpha3", "0.5.1-alpha4", "0.5.1-beta", "0.5.1", "0.5.5"]);
        assert_not_match(&r,
                         &["0.5.1-alpha1", "0.5.2-alpha3", "0.5.5-pre", "0.5.0-pre", "0.6.0"]);

        let r = req("^0.0.2");
        assert_match(&r, &["0.0.2"]);
        assert_not_match(&r, &["0.9.1", "2.9.0", "1.1.1", "0.0.1", "0.1.4"]);

        let r = req("^0.0");
        assert_match(&r, &["0.0.2", "0.0.0"]);
        assert_not_match(&r, &["0.9.1", "2.9.0", "1.1.1", "0.1.4"]);

        let r = req("^0");
        assert_match(&r, &["0.9.1", "0.0.2", "0.0.0"]);
        assert_not_match(&r, &["2.9.0", "1.1.1"]);

        let r = req("^1.4.2-beta.5");
        assert_match(&r,
                     &["1.4.2", "1.4.3", "1.4.2-beta.5", "1.4.2-beta.6", "1.4.2-c"]);
        assert_not_match(&r,
                         &["0.9.9", "2.0.0", "1.4.2-alpha", "1.4.2-beta.4", "1.4.3-beta.5"]);
    }

    #[test]
    pub fn test_parsing_wildcard() {
        let r = req("");
        assert_match(&r, &["0.9.1", "2.9.0", "0.0.9", "1.0.1", "1.1.1"]);
        assert_not_match(&r, &[]);
        let r = req("*");
        assert_match(&r, &["0.9.1", "2.9.0", "0.0.9", "1.0.1", "1.1.1"]);
        assert_not_match(&r, &[]);
        let r = req("x");
        assert_match(&r, &["0.9.1", "2.9.0", "0.0.9", "1.0.1", "1.1.1"]);
        assert_not_match(&r, &[]);
        let r = req("X");
        assert_match(&r, &["0.9.1", "2.9.0", "0.0.9", "1.0.1", "1.1.1"]);
        assert_not_match(&r, &[]);

        let r = req("1.*");
        assert_match(&r, &["1.2.0", "1.2.1", "1.1.1", "1.3.0"]);
        assert_not_match(&r, &["0.0.9"]);
        let r = req("1.x");
        assert_match(&r, &["1.2.0", "1.2.1", "1.1.1", "1.3.0"]);
        assert_not_match(&r, &["0.0.9"]);
        let r = req("1.X");
        assert_match(&r, &["1.2.0", "1.2.1", "1.1.1", "1.3.0"]);
        assert_not_match(&r, &["0.0.9"]);

        let r = req("1.2.*");
        assert_match(&r, &["1.2.0", "1.2.2", "1.2.4"]);
        assert_not_match(&r, &["1.9.0", "1.0.9", "2.0.1", "0.1.3"]);
        let r = req("1.2.x");
        assert_match(&r, &["1.2.0", "1.2.2", "1.2.4"]);
        assert_not_match(&r, &["1.9.0", "1.0.9", "2.0.1", "0.1.3"]);
        let r = req("1.2.X");
        assert_match(&r, &["1.2.0", "1.2.2", "1.2.4"]);
        assert_not_match(&r, &["1.9.0", "1.0.9", "2.0.1", "0.1.3"]);
    }

    #[test]
    pub fn test_any() {
        let r = VersionReq::any();
        assert_match(&r, &["0.0.1", "0.1.0", "1.0.0"]);
    }

    #[test]
    pub fn test_pre() {
        let r = req("=2.1.1-really.0");
        assert_match(&r, &["2.1.1-really.0"]);
    }

    // #[test]
    // pub fn test_parse_errors() {
    //    assert_eq!(Err(InvalidVersionRequirement), VersionReq::parse("\0"));
    //    assert_eq!(Err(OpAlreadySet), VersionReq::parse(">= >= 0.0.2"));
    //    assert_eq!(Err(InvalidSigil), VersionReq::parse(">== 0.0.2"));
    //    assert_eq!(Err(VersionComponentsMustBeNumeric),
    //               VersionReq::parse("a.0.0"));
    //    assert_eq!(Err(InvalidIdentifier), VersionReq::parse("1.0.0-"));
    //    assert_eq!(Err(MajorVersionRequired), VersionReq::parse(">="));
    // }

    #[test]
    pub fn test_from_str() {
        assert_eq!("1.0.0".parse::<VersionReq>().unwrap().to_string(),
                   "^1.0.0".to_string());
        assert_eq!("=1.0.0".parse::<VersionReq>().unwrap().to_string(),
                   "= 1.0.0".to_string());
        assert_eq!("~1".parse::<VersionReq>().unwrap().to_string(),
                   "~1".to_string());
        assert_eq!("~1.2".parse::<VersionReq>().unwrap().to_string(),
                   "~1.2".to_string());
        assert_eq!("^1".parse::<VersionReq>().unwrap().to_string(),
                   "^1".to_string());
        assert_eq!("^1.1".parse::<VersionReq>().unwrap().to_string(),
                   "^1.1".to_string());
        assert_eq!("*".parse::<VersionReq>().unwrap().to_string(),
                   "*".to_string());
        assert_eq!("1.*".parse::<VersionReq>().unwrap().to_string(),
                   "1.*".to_string());
        assert_eq!("< 1.0.0".parse::<VersionReq>().unwrap().to_string(),
                   "< 1.0.0".to_string());
    }

    // #[test]
    // pub fn test_from_str_errors() {
    //    assert_eq!(Err(InvalidVersionRequirement), "\0".parse::<VersionReq>());
    //    assert_eq!(Err(OpAlreadySet), ">= >= 0.0.2".parse::<VersionReq>());
    //    assert_eq!(Err(InvalidSigil), ">== 0.0.2".parse::<VersionReq>());
    //    assert_eq!(Err(VersionComponentsMustBeNumeric),
    //               "a.0.0".parse::<VersionReq>());
    //    assert_eq!(Err(InvalidIdentifier), "1.0.0-".parse::<VersionReq>());
    //    assert_eq!(Err(MajorVersionRequired), ">=".parse::<VersionReq>());
    // }

    #[test]
    fn test_cargo3202() {
        let v = "0.*.*".parse::<VersionReq>().unwrap();
        assert_eq!("0.*.*", format!("{}", v.predicates[0]));

        let v = "0.0.*".parse::<VersionReq>().unwrap();
        assert_eq!("0.0.*", format!("{}", v.predicates[0]));

        let r = req("0.*.*");
        assert_match(&r, &["0.5.0"]);
    }

    #[test]
    fn test_eq_hash() {
        assert!(req("^1") == req("^1"));
        assert!(calculate_hash(req("^1")) == calculate_hash(req("^1")));
        assert!(req("^1") != req("^2"));
    }

    #[test]
    fn test_ordering() {
        assert!(req("=1") < req("*"));
        assert!(req(">1") < req("*"));
        assert!(req(">=1") < req("*"));
        assert!(req("<1") < req("*"));
        assert!(req("<=1") < req("*"));
        assert!(req("~1") < req("*"));
        assert!(req("^1") < req("*"));
        assert!(req("*") == req("*"));
    }
}
