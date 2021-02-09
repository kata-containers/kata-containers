semver
======

Semantic version parsing and comparison.

[![Build Status](https://api.travis-ci.org/steveklabnik/semver.svg?branch=master)](https://travis-ci.org/steveklabnik/semver)

[Documentation](https://steveklabnik.github.io/semver)

Semantic versioning (see http://semver.org/) is a set of rules for
assigning version numbers.

## SemVer and the Rust ecosystem

Rust itself follows the SemVer specification, as does its standard libraries. The two are
not tied together.

[Cargo](https://crates.io), Rust's package manager, uses SemVer to determine which versions of
packages you need installed.

## Installation

To use `semver`, add this to your `[dependencies]` section:

```toml
semver = "0.7.0"
```

And this to your crate root:

```rust
extern crate semver;
```

## Versions

At its simplest, the `semver` crate allows you to construct `Version` objects using the `parse`
method:

```rust
use semver::Version;

assert!(Version::parse("1.2.3") == Ok(Version {
   major: 1,
   minor: 2,
   patch: 3,
   pre: vec!(),
   build: vec!(),
}));
```

If you have multiple `Version`s, you can use the usual comparison operators to compare them:

```rust
use semver::Version;

assert!(Version::parse("1.2.3-alpha")  != Version::parse("1.2.3-beta"));
assert!(Version::parse("1.2.3-alpha2") >  Version::parse("1.2.0"));
```

## Requirements

The `semver` crate also provides the ability to compare requirements, which are more complex
comparisons.

For example, creating a requirement that only matches versions greater than or
equal to 1.0.0:

```rust
use semver::Version;
use semver::VersionReq;

let r = VersionReq::parse(">= 1.0.0").unwrap();
let v = Version::parse("1.0.0").unwrap();

assert!(r.to_string() == ">= 1.0.0".to_string());
assert!(r.matches(&v))
```

It also allows parsing of `~x.y.z` and `^x.y.z` requirements as defined at
https://www.npmjs.org/doc/misc/semver.html

**Tilde requirements** specify a minimal version with some updates:

```notrust
~1.2.3 := >=1.2.3 <1.3.0
~1.2   := >=1.2.0 <1.3.0
~1     := >=1.0.0 <2.0.0
```

**Caret requirements** allow SemVer compatible updates to a specified version,
`0.x` and `0.x+1` are not considered compatible, but `1.x` and `1.x+1` are.

`0.0.x` is not considered compatible with any other version.
Missing minor and patch versions are desugared to `0` but allow flexibility for that value.

```notrust
^1.2.3 := >=1.2.3 <2.0.0
^0.2.3 := >=0.2.3 <0.3.0
^0.0.3 := >=0.0.3 <0.0.4
^0.0   := >=0.0.0 <0.1.0
^0     := >=0.0.0 <1.0.0
```
