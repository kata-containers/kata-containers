//! [Object Identifiers] are a standard of the [ITU] used to reference objects, things, and
//! concepts in a globally unique way. This crate provides for data structures and methods
//! to build, parse, and format OIDs.
//!
//!
//! ## Parsing OID String Representation
//! ```rust
//! use oid::prelude::*;
//!
//! fn main() -> Result<(), ObjectIdentifierError> {
//!     let oid = ObjectIdentifier::try_from("0.1.2.3")?;
//!     Ok(())
//! }
//! ```
//!
//! ## Parsing OID Binary Representation
//! ```rust
//! use oid::prelude::*;
//!
//! fn main() -> Result<(), ObjectIdentifierError> {
//!     let oid = ObjectIdentifier::try_from(vec![0x00, 0x01, 0x02, 0x03])?;
//!     Ok(())
//! }
//! ```
//!
//! ## Encoding OID as String Representation
//! ```rust
//! use oid::prelude::*;
//!
//! fn main() -> Result<(), ObjectIdentifierError> {
//!     let oid = ObjectIdentifier::try_from("0.1.2.3")?;
//!     let oid: String = oid.into();
//!     assert_eq!(oid, "0.1.2.3");
//!     Ok(())
//! }
//! ```
//!
//! ## Encoding OID as Binary Representation
//! ```rust
//! use oid::prelude::*;
//!
//! fn main() -> Result<(), ObjectIdentifierError> {
//!     let oid = ObjectIdentifier::try_from(vec![0x00, 0x01, 0x02, 0x03])?;
//!     let oid: Vec<u8> = oid.into();
//!     assert_eq!(oid, vec![0x00, 0x01, 0x02, 0x03]);
//!     Ok(())
//! }
//! ```
//!
//! [Object Identifiers]: https://en.wikipedia.org/wiki/Object_identifier
//! [ITU]: https://en.wikipedia.org/wiki/International_Telecommunications_Union
#![doc(
    html_logo_url = "https://labs.unnecessary.engineering/logo.png",
    html_favicon_url = "https://labs.unnecessary.engineering/favicon.ico",
    issue_tracker_base_url = "https://github.com/UnnecessaryEngineering/oid/issues/"
)]
#![deny(missing_docs, unused_imports, missing_debug_implementations)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
#[macro_use]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use core::convert::{TryFrom, TryInto};

// OID spec doesn't specify the maximum integer size of each node, so we default to usize
#[cfg(not(feature = "u32"))]
type Node = usize;

// Can also specifically set node size as u32, helpful for 8-bit embedded platforms
#[cfg(feature = "u32")]
type Node = u32;

/// Convenience module for quickly importing the public interface (e.g., `use oid::prelude::*`)
pub mod prelude {
    pub use crate::ObjectIdentifier;
    pub use crate::ObjectIdentifierError;
    pub use core::convert::{TryFrom, TryInto};
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ObjectIdentifierRoot {
    ItuT = 0,
    Iso = 1,
    JointIsoItuT = 2,
}

impl Into<String> for ObjectIdentifierRoot {
    fn into(self) -> String {
        format!("{}", self as u8)
    }
}

impl TryFrom<u8> for ObjectIdentifierRoot {
    type Error = ObjectIdentifierError;
    fn try_from(value: u8) -> Result<ObjectIdentifierRoot, Self::Error> {
        match value {
            0 => Ok(ObjectIdentifierRoot::ItuT),
            1 => Ok(ObjectIdentifierRoot::Iso),
            2 => Ok(ObjectIdentifierRoot::JointIsoItuT),
            _ => Err(ObjectIdentifierError::IllegalRootNode),
        }
    }
}

/// Object Identifier Errors
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectIdentifierError {
    /// Failed to parse OID due to illegal root node (must be 0-2 decimal)
    IllegalRootNode,
    /// Failed to parse OID due to illegal first node (must be 0-39 decimal)
    IllegalFirstChildNode,
    /// Failed to parse OID due to illegal child node value (except first node)
    IllegalChildNodeValue,
}

/// Object Identifier (OID)
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectIdentifier {
    root: ObjectIdentifierRoot,
    first_node: u8,
    child_nodes: Vec<Node>,
}

fn parse_string_first_node(
    nodes: &mut dyn Iterator<Item = &str>,
) -> Result<u8, ObjectIdentifierError> {
    if let Some(first_child_node) = nodes.next() {
        let first_child_node: u8 = first_child_node
            .parse()
            .map_err(|_| ObjectIdentifierError::IllegalFirstChildNode)?;
        if first_child_node > 39 {
            return Err(ObjectIdentifierError::IllegalFirstChildNode);
        }
        Ok(first_child_node)
    } else {
        Err(ObjectIdentifierError::IllegalFirstChildNode)
    }
}

fn parse_string_child_nodes(
    nodes: &mut dyn Iterator<Item = &str>,
) -> Result<Vec<Node>, ObjectIdentifierError> {
    let mut result: Vec<Node> = vec![];
    while let Some(node) = nodes.next() {
        result.push(
            node.parse()
                .map_err(|_| ObjectIdentifierError::IllegalChildNodeValue)?,
        );
    }
    Ok(result)
}

impl ObjectIdentifier {
    fn from_string<S>(value: S) -> Result<ObjectIdentifier, ObjectIdentifierError>
    where
        S: Into<String>,
    {
        let value = value.into();
        let mut nodes = value.split(".");
        match &nodes.next() {
            Some(root_node_value) => {
                let root_node_value: Result<u8, _> = root_node_value.parse();
                match root_node_value {
                    Ok(root_node) => {
                        let root_node: Result<ObjectIdentifierRoot, _> = root_node.try_into();
                        match root_node {
                            Ok(root) => {
                                let first_node = parse_string_first_node(&mut nodes)?;
                                Ok(ObjectIdentifier {
                                    root,
                                    first_node,
                                    child_nodes: parse_string_child_nodes(&mut nodes)?,
                                })
                            }
                            Err(_err) => Err(ObjectIdentifierError::IllegalRootNode),
                        }
                    }
                    Err(_) => Err(ObjectIdentifierError::IllegalRootNode),
                }
            }
            None => Err(ObjectIdentifierError::IllegalRootNode),
        }
    }
}

impl Into<String> for &ObjectIdentifier {
    fn into(self) -> String {
        let mut result: String = self.root.into();
        result.push_str(&format!(".{}", self.first_node));
        for node in &self.child_nodes {
            result.push_str(&format!(".{}", node));
        }
        result
    }
}

impl Into<String> for ObjectIdentifier {
    fn into(self) -> String {
        (&self).into()
    }
}

impl Into<Vec<u8>> for &ObjectIdentifier {
    fn into(self) -> Vec<u8> {
        let mut result: Vec<u8> = vec![self.root as u8];
        result[0] = result[0] * 40 + self.first_node;
        for node in self.child_nodes.iter() {
            // TODO bench against !*node &= 0x80, compiler may already optimize better
            if *node <= 127 {
                result.push(*node as u8);
            } else {
                let mut value = *node;
                let mut mask: Node = 0;
                let mut encoded: Vec<u8> = vec![];
                while value > 0x80 {
                    encoded.insert(0, (value & 0x7f | mask) as u8);
                    value >>= 7;
                    mask = 0x80;
                }
                encoded.insert(0, (value | mask) as u8);
                result.append(&mut encoded);
            }
        }
        result
    }
}

impl Into<Vec<u8>> for ObjectIdentifier {
    fn into(self) -> Vec<u8> {
        (&self).into()
    }
}

impl TryFrom<&str> for ObjectIdentifier {
    type Error = ObjectIdentifierError;
    fn try_from(value: &str) -> Result<ObjectIdentifier, Self::Error> {
        ObjectIdentifier::from_string(value)
    }
}

impl TryFrom<String> for ObjectIdentifier {
    type Error = ObjectIdentifierError;
    fn try_from(value: String) -> Result<ObjectIdentifier, Self::Error> {
        ObjectIdentifier::from_string(value)
    }
}

impl TryFrom<&[u8]> for ObjectIdentifier {
    type Error = ObjectIdentifierError;
    fn try_from(value: &[u8]) -> Result<ObjectIdentifier, Self::Error> {
        if value.len() < 1 {
            return Err(ObjectIdentifierError::IllegalRootNode);
        };
        let root = ObjectIdentifierRoot::try_from(value[0] / 40)?;
        let first_node = value[0] % 40;
        let mut child_nodes = vec![];
        let mut parsing_big_int = false;
        let mut big_int: Node = 0;
        for i in 1..value.len() {
            if !parsing_big_int && value[i] < 128 {
                child_nodes.push(value[i] as Node);
            } else {
                if big_int > 0 {
                    if big_int >= Node::max_value() >> 7 {
                        return Err(ObjectIdentifierError::IllegalChildNodeValue);
                    }
                    big_int <<= 7;
                };
                big_int |= (value[i] & !0x80) as Node;
                parsing_big_int = value[i] & 0x80 != 0;
            }
            if big_int > 0 && !parsing_big_int {
                child_nodes.push(big_int);
                big_int = 0;
            }
        }
        Ok(ObjectIdentifier {
            root,
            first_node,
            child_nodes,
        })
    }
}

impl TryFrom<Vec<u8>> for ObjectIdentifier {
    type Error = ObjectIdentifierError;
    fn try_from(value: Vec<u8>) -> Result<ObjectIdentifier, Self::Error> {
        value.as_slice().try_into()
    }
}

#[cfg(feature = "serde_support")]
mod serde_support {
    use super::*;
    use core::fmt;
    use serde::{de, ser};

    struct OidVisitor;

    impl<'de> de::Visitor<'de> for OidVisitor {
        type Value = ObjectIdentifier;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a valid buffer representing an OID")
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            ObjectIdentifier::try_from(v).map_err(|err| {
                E::invalid_value(
                    de::Unexpected::Other(match err {
                        ObjectIdentifierError::IllegalRootNode => "illegal root node",
                        ObjectIdentifierError::IllegalFirstChildNode => "illegal first child node",
                        ObjectIdentifierError::IllegalChildNodeValue => "illegal child node value",
                    }),
                    &"a valid buffer representing an OID",
                )
            })
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            ObjectIdentifier::try_from(v).map_err(|err| {
                E::invalid_value(
                    de::Unexpected::Other(match err {
                        ObjectIdentifierError::IllegalRootNode => "illegal root node",
                        ObjectIdentifierError::IllegalFirstChildNode => "illegal first child node",
                        ObjectIdentifierError::IllegalChildNodeValue => "illegal child node value",
                    }),
                    &"a string representing an OID",
                )
            })
        }
    }

    impl<'de> de::Deserialize<'de> for ObjectIdentifier {
        fn deserialize<D>(deserializer: D) -> Result<ObjectIdentifier, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            if deserializer.is_human_readable() {
                deserializer.deserialize_str(OidVisitor)
            } else {
                deserializer.deserialize_bytes(OidVisitor)
            }
        }
    }

    impl ser::Serialize for ObjectIdentifier {
        fn serialize<S>(
            &self,
            serializer: S,
        ) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
        where
            S: ser::Serializer,
        {
            if serializer.is_human_readable() {
                let encoded: String = self.into();
                serializer.serialize_str(&encoded)
            } else {
                let encoded: Vec<u8> = self.into();
                serializer.serialize_bytes(&encoded)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "serde_support")]
    mod serde_support {
        use super::*;
        use serde_derive::{Deserialize, Serialize};
        use serde_xml_rs;

        #[test]
        fn bincode_serde_serialize() {
            let expected: Vec<u8> = vec![
                0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x02, 0x03, 0x05, 0x08,
                0x0D, 0x15,
            ];
            let oid = ObjectIdentifier {
                root: ObjectIdentifierRoot::ItuT,
                first_node: 0x01,
                child_nodes: vec![1, 2, 3, 5, 8, 13, 21],
            };
            let actual: Vec<u8> = bincode::serialize(&oid).unwrap();
            assert_eq!(expected, actual);
        }

        #[test]
        fn bincode_serde_deserialize() {
            let expected = ObjectIdentifier {
                root: ObjectIdentifierRoot::ItuT,
                first_node: 0x01,
                child_nodes: vec![1, 2, 3, 5, 8, 13, 21],
            };
            let actual: ObjectIdentifier = bincode::deserialize(&[
                0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x02, 0x03, 0x05, 0x08,
                0x0D, 0x15,
            ])
            .unwrap();
            assert_eq!(expected, actual);
        }

        #[derive(Debug, Deserialize, PartialEq, Serialize)]
        struct MyStruct {
            oid: ObjectIdentifier
        }

        #[test]
        fn xml_serde_serialize() {
            let mydata = MyStruct {
                oid: ObjectIdentifier::try_from("1.2.3.5.8.13.21").unwrap()
            };
            let expected = r#"<MyStruct><oid>1.2.3.5.8.13.21</oid></MyStruct>"#;
            let actual = serde_xml_rs::to_string(&mydata).unwrap();
            assert_eq!(expected, actual);
        }

        #[test]
        fn xml_serde_deserialize_element() {
            let src = r#"<mystruct><oid>1.2.3.5.8.13.21</oid></mystruct>"#;

            let expected = MyStruct {
                oid: ObjectIdentifier::try_from("1.2.3.5.8.13.21").unwrap()
            };
            let actual: MyStruct = serde_xml_rs::from_str(&src).unwrap();
            assert_eq!(expected, actual);
        }

        #[test]
        fn xml_serde_deserialize_attribute() {
            let src = r#"<mystruct oid="1.2.3.5.8.13.21" />"#;

            let expected = MyStruct {
                oid: ObjectIdentifier::try_from("1.2.3.5.8.13.21").unwrap()
            };
            let actual: MyStruct = serde_xml_rs::from_str(&src).unwrap();
            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn encode_binary_root_node_0() {
        let expected: Vec<u8> = vec![0];
        let oid = ObjectIdentifier {
            root: ObjectIdentifierRoot::ItuT,
            first_node: 0x00,
            child_nodes: vec![],
        };
        let actual: Vec<u8> = (&oid).into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_binary_root_node_1() {
        let expected: Vec<u8> = vec![40];
        let oid = ObjectIdentifier {
            root: ObjectIdentifierRoot::Iso,
            first_node: 0x00,
            child_nodes: vec![],
        };
        let actual: Vec<u8> = (&oid).into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_binary_root_node_2() {
        let expected: Vec<u8> = vec![80];
        let oid = ObjectIdentifier {
            root: ObjectIdentifierRoot::JointIsoItuT,
            first_node: 0x00,
            child_nodes: vec![],
        };
        let actual: Vec<u8> = (&oid).into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_binary_example_1() {
        let expected: Vec<u8> = vec![0x01, 0x01, 0x02, 0x03, 0x05, 0x08, 0x0D, 0x15];
        let oid = ObjectIdentifier {
            root: ObjectIdentifierRoot::ItuT,
            first_node: 0x01,
            child_nodes: vec![1, 2, 3, 5, 8, 13, 21],
        };
        let actual: Vec<u8> = (&oid).into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_binary_example_2() {
        let expected: Vec<u8> = vec![
            0x77, 0x2A, 0x93, 0x45, 0x83, 0xFF, 0x7F, 0x87, 0xFF, 0xFF, 0xFF, 0x7F, 0x89, 0x53,
            0x92, 0x30,
        ];
        let oid = ObjectIdentifier {
            root: ObjectIdentifierRoot::JointIsoItuT,
            first_node: 39,
            child_nodes: vec![42, 2501, 65535, 2147483647, 1235, 2352],
        };
        let actual: Vec<u8> = (&oid).into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_string_root_node_0() {
        let expected = "0.0";
        let oid = ObjectIdentifier {
            root: ObjectIdentifierRoot::ItuT,
            first_node: 0x00,
            child_nodes: vec![],
        };
        let actual: String = (&oid).into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_string_root_node_1() {
        let expected = "1.0";
        let oid = ObjectIdentifier {
            root: ObjectIdentifierRoot::Iso,
            first_node: 0x00,
            child_nodes: vec![],
        };
        let actual: String = (&oid).into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_string_root_node_2() {
        let expected = "2.0";
        let oid = ObjectIdentifier {
            root: ObjectIdentifierRoot::JointIsoItuT,
            first_node: 0x00,
            child_nodes: vec![],
        };
        let actual: String = (&oid).into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_string_example_1() {
        let expected = "0.1.1.2.3.5.8.13.21";
        let oid = ObjectIdentifier {
            root: ObjectIdentifierRoot::ItuT,
            first_node: 0x01,
            child_nodes: vec![1, 2, 3, 5, 8, 13, 21],
        };
        let actual: String = (&oid).into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_string_example_2() {
        let expected = "2.39.42.2501.65535.2147483647.1235.2352";
        let oid = ObjectIdentifier {
            root: ObjectIdentifierRoot::JointIsoItuT,
            first_node: 39,
            child_nodes: vec![42, 2501, 65535, 2147483647, 1235, 2352],
        };
        let actual: String = (&oid).into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_binary_root_node_0() {
        let expected = Ok(ObjectIdentifier {
            root: ObjectIdentifierRoot::ItuT,
            first_node: 0x00,
            child_nodes: vec![],
        });
        let actual = vec![0x00].try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_binary_root_node_1() {
        let expected = Ok(ObjectIdentifier {
            root: ObjectIdentifierRoot::Iso,
            first_node: 0x00,
            child_nodes: vec![],
        });
        let actual = vec![40].try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_binary_root_node_2() {
        let expected = Ok(ObjectIdentifier {
            root: ObjectIdentifierRoot::JointIsoItuT,
            first_node: 0x00,
            child_nodes: vec![],
        });
        let actual = vec![80].try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_binary_example_1() {
        let expected = Ok(ObjectIdentifier {
            root: ObjectIdentifierRoot::ItuT,
            first_node: 0x01,
            child_nodes: vec![1, 2, 3, 5, 8, 13, 21],
        });
        let actual = vec![0x01, 0x01, 0x02, 0x03, 0x05, 0x08, 0x0D, 0x15].try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_binary_example_2() {
        let expected = Ok(ObjectIdentifier {
            root: ObjectIdentifierRoot::JointIsoItuT,
            first_node: 39,
            child_nodes: vec![42, 2501, 65535, 2147483647, 1235, 2352],
        });
        let actual = vec![
            0x77, 0x2A, 0x93, 0x45, 0x83, 0xFF, 0x7F, 0x87, 0xFF, 0xFF, 0xFF, 0x7F, 0x89, 0x53,
            0x92, 0x30,
        ]
        .try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_string_root_node_0() {
        let expected = Ok(ObjectIdentifier {
            root: ObjectIdentifierRoot::ItuT,
            first_node: 0x00,
            child_nodes: vec![],
        });
        let actual = "0.0".try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_string_root_node_1() {
        let expected = Ok(ObjectIdentifier {
            root: ObjectIdentifierRoot::Iso,
            first_node: 0x00,
            child_nodes: vec![],
        });
        let actual = "1.0".try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_string_root_node_2() {
        let expected = Ok(ObjectIdentifier {
            root: ObjectIdentifierRoot::JointIsoItuT,
            first_node: 0x00,
            child_nodes: vec![],
        });
        let actual = "2.0".try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_string_example_1() {
        let expected = Ok(ObjectIdentifier {
            root: ObjectIdentifierRoot::ItuT,
            first_node: 0x01,
            child_nodes: vec![1, 2, 3, 5, 8, 13, 21],
        });
        let actual = "0.1.1.2.3.5.8.13.21".try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_string_example_2() {
        let expected = Ok(ObjectIdentifier {
            root: ObjectIdentifierRoot::JointIsoItuT,
            first_node: 39,
            child_nodes: vec![42, 2501, 65535, 2147483647, 1235, 2352],
        });
        let actual = "2.39.42.2501.65535.2147483647.1235.2352".try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn illegal_oid_root() {
        let expected = Err(ObjectIdentifierError::IllegalRootNode);
        for i in 3..core::u8::MAX {
            let actual = ObjectIdentifierRoot::try_from(i);
            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn illegal_first_node_too_large() {
        let expected = Err(ObjectIdentifierError::IllegalFirstChildNode);
        for i in 40..core::u8::MAX {
            let string_val = format!("{}.2.3.4", i);
            let mut nodes_iter = string_val.split(".");
            let actual = parse_string_first_node(&mut nodes_iter);
            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn illegal_first_node_empty() {
        let expected = Err(ObjectIdentifierError::IllegalFirstChildNode);
        let string_val = String::new();
        let mut nodes_iter = string_val.split(".");
        let actual = parse_string_first_node(&mut nodes_iter);
        assert_eq!(expected, actual);
    }

    #[test]
    fn illegal_first_node_none() {
        let expected = Err(ObjectIdentifierError::IllegalFirstChildNode);
        let string_val = String::new();
        let mut nodes_iter = string_val.split(".");
        let _ = nodes_iter.next();
        let actual = parse_string_first_node(&mut nodes_iter);
        assert_eq!(expected, actual);
    }

    #[test]
    fn illegal_first_node_large() {
        let expected = Err(ObjectIdentifierError::IllegalFirstChildNode);
        let string_val = String::from("40");
        let mut nodes_iter = string_val.split(".");
        let actual = parse_string_first_node(&mut nodes_iter);
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_string_crap() {
        let expected: Result<ObjectIdentifier, ObjectIdentifierError> =
            Err(ObjectIdentifierError::IllegalRootNode);
        let actual = "wtf".try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_string_empty() {
        let expected: Result<ObjectIdentifier, ObjectIdentifierError> =
            Err(ObjectIdentifierError::IllegalRootNode);
        let actual = String::new().try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_binary_empty() {
        let expected: Result<ObjectIdentifier, ObjectIdentifierError> =
            Err(ObjectIdentifierError::IllegalRootNode);
        let actual = vec![].try_into();
        assert_eq!(expected, actual);
    }

    #[cfg(feature = "u32")]
    #[test]
    fn parse_binary_example_over_u32() {
        let expected: Result<ObjectIdentifier, ObjectIdentifierError> =
            Err(ObjectIdentifierError::IllegalChildNodeValue);
        let actual = vec![0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0x00].try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_binary_example_over_u128() {
        let expected: Result<ObjectIdentifier, ObjectIdentifierError> =
            Err(ObjectIdentifierError::IllegalChildNodeValue);
        let actual = vec![
            0x00, 0x89, 0x97, 0xBF, 0xA3, 0xB8, 0xE8, 0xB3, 0xE6, 0xFB, 0xF2, 0xEA, 0xC3, 0xCA,
            0xF2, 0xBF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F,
        ]
        .try_into();
        assert_eq!(expected, actual);
    }
    #[test]
    fn parse_string_root_node_3plus() {
        for i in 3..=core::u8::MAX {
            let expected: Result<ObjectIdentifier, ObjectIdentifierError> =
                Err(ObjectIdentifierError::IllegalRootNode);
            let actual = format!("{}", i).try_into();
            assert_eq!(expected, actual);
        }
    }

    #[cfg(feature = "u32")]
    #[test]
    fn parse_string_example_over_u32() {
        let expected: Result<ObjectIdentifier, ObjectIdentifierError> =
            Err(ObjectIdentifierError::IllegalChildNodeValue);
        let actual = "1.1.10000000000".try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_string_example_over_u128() {
        let expected: Result<ObjectIdentifier, ObjectIdentifierError> =
            Err(ObjectIdentifierError::IllegalChildNodeValue);
        let actual = "1.1.349239782398732987223423423423423423423423423423434982342342342342342342324523453452345234523452345234523452345234537234987234".try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_string_example_first_node_over_39() {
        let expected: Result<ObjectIdentifier, ObjectIdentifierError> =
            Err(ObjectIdentifierError::IllegalFirstChildNode);
        let actual = "1.40.1.2.3".try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_to_string() {
        let expected = String::from("1.2.3.4");
        let actual: String = ObjectIdentifier {
            root: ObjectIdentifierRoot::Iso,
            first_node: 2,
            child_nodes: vec![3, 4],
        }
        .into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_to_bytes() {
        let expected = vec![0x2A, 0x03, 0x04];
        let actual: Vec<u8> = ObjectIdentifier {
            root: ObjectIdentifierRoot::Iso,
            first_node: 2,
            child_nodes: vec![3, 4],
        }
        .into();
        assert_eq!(expected, actual);
    }
}
