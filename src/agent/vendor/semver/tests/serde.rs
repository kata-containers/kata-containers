#![cfg(feature = "serde")]

#[macro_use]
extern crate serde_derive;

extern crate semver;
extern crate serde_json;

use semver::{Identifier, Version, VersionReq};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Identified {
    name: String,
    identifier: Identifier,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Versioned {
    name: String,
    vers: Version,
}

#[test]
fn serialize_identifier() {
    let id = Identified {
        name: "serde".to_owned(),
        identifier: Identifier::Numeric(100),
    };
    let j = serde_json::to_string(&id).unwrap();
    assert_eq!(j, r#"{"name":"serde","identifier":100}"#);

    let id = Identified {
        name: "serde".to_owned(),
        identifier: Identifier::AlphaNumeric("b100".to_owned()),
    };
    let j = serde_json::to_string(&id).unwrap();
    assert_eq!(j, r#"{"name":"serde","identifier":"b100"}"#);
}

#[test]
fn deserialize_identifier() {
    let j = r#"{"name":"serde","identifier":100}"#;
    let id = serde_json::from_str::<Identified>(j).unwrap();
    let expected = Identified {
        name: "serde".to_owned(),
        identifier: Identifier::Numeric(100),
    };
    assert_eq!(id, expected);

    let j = r#"{"name":"serde","identifier":"b100"}"#;
    let id = serde_json::from_str::<Identified>(j).unwrap();
    let expected = Identified {
        name: "serde".to_owned(),
        identifier: Identifier::AlphaNumeric("b100".to_owned()),
    };
    assert_eq!(id, expected);
}

#[test]
fn serialize_version() {
    let v = Versioned {
        name: "serde".to_owned(),
        vers: Version::parse("1.0.0").unwrap(),
    };
    let j = serde_json::to_string(&v).unwrap();
    assert_eq!(j, r#"{"name":"serde","vers":"1.0.0"}"#);
}

#[test]
fn deserialize_version() {
    let j = r#"{"name":"serde","vers":"1.0.0"}"#;
    let v = serde_json::from_str::<Versioned>(j).unwrap();
    let expected = Versioned {
        name: "serde".to_owned(),
        vers: Version::parse("1.0.0").unwrap(),
    };
    assert_eq!(v, expected);
}

#[test]
fn serialize_versionreq() {
    let v = VersionReq::exact(&Version::parse("1.0.0").unwrap());

    assert_eq!(serde_json::to_string(&v).unwrap(), r#""= 1.0.0""#);
}

#[test]
fn deserialize_versionreq() {
    assert_eq!("1.0.0".parse::<VersionReq>().unwrap(), serde_json::from_str(r#""1.0.0""#).unwrap());
}
