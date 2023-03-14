#![cfg(feature = "xml")]

//! Introspection XML support (`xml` feature)
//!
//! Thanks to the [`org.freedesktop.DBus.Introspectable`] interface, objects may be introspected at
//! runtime, returning an XML string that describes the object.
//!
//! This optional `xml` module provides facilities to parse the XML data into more convenient Rust
//! structures. The XML string may be parsed to a tree with [`Node.from_reader()`].
//!
//! See also:
//!
//! * [Introspection format] in the DBus specification
//!
//! [`Node.from_reader()`]: struct.Node.html#method.from_reader
//! [Introspection format]: https://dbus.freedesktop.org/doc/dbus-specification.html#introspection-format
//! [`org.freedesktop.DBus.Introspectable`]: https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces-introspectable

use serde::{Deserialize, Serialize};
use serde_xml_rs::{from_reader, from_str, to_writer};
use static_assertions::assert_impl_all;
use std::{
    io::{Read, Write},
    result::Result,
};

use crate::Error;

// note: serde-xml-rs doesn't handle nicely interleaved elements, so we have to use enums:
// https://github.com/RReverser/serde-xml-rs/issues/55

macro_rules! get_vec {
    ($vec:expr, $kind:path) => {
        $vec.iter()
            .filter_map(|e| if let $kind(m) = e { Some(m) } else { None })
            .collect()
    };
}

/// Annotations are generic key/value pairs of metadata.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Annotation {
    name: String,
    value: String,
}

assert_impl_all!(Annotation: Send, Sync, Unpin);

impl Annotation {
    /// Return the annotation name/key.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the annotation value.
    pub fn value(&self) -> &str {
        &self.value
    }
}

/// An argument
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Arg {
    name: Option<String>,
    r#type: String,
    direction: Option<String>,
    #[serde(rename = "annotation", default)]
    annotations: Vec<Annotation>,
}

assert_impl_all!(Arg: Send, Sync, Unpin);

impl Arg {
    /// Return the argument name, if any.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Return the argument type.
    pub fn ty(&self) -> &str {
        &self.r#type
    }

    /// Return the argument direction (should be "in" or "out"), if any.
    pub fn direction(&self) -> Option<&str> {
        self.direction.as_deref()
    }

    /// Return the associated annotations.
    pub fn annotations(&self) -> Vec<&Annotation> {
        self.annotations.iter().collect()
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
enum MethodElement {
    Arg(Arg),
    Annotation(Annotation),
}

/// A method
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Method {
    name: String,

    #[serde(rename = "$value", default)]
    elems: Vec<MethodElement>,
}

assert_impl_all!(Method: Send, Sync, Unpin);

impl Method {
    /// Return the method name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the method arguments.
    pub fn args(&self) -> Vec<&Arg> {
        get_vec!(self.elems, MethodElement::Arg)
    }

    /// Return the method annotations.
    pub fn annotations(&self) -> Vec<&Annotation> {
        get_vec!(self.elems, MethodElement::Annotation)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
enum SignalElement {
    Arg(Arg),
    Annotation(Annotation),
}

/// A signal
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Signal {
    name: String,

    #[serde(rename = "$value", default)]
    elems: Vec<SignalElement>,
}

assert_impl_all!(Signal: Send, Sync, Unpin);

impl Signal {
    /// Return the signal name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the signal arguments.
    pub fn args(&self) -> Vec<&Arg> {
        get_vec!(self.elems, SignalElement::Arg)
    }

    /// Return the signal annotations.
    pub fn annotations(&self) -> Vec<&Annotation> {
        get_vec!(self.elems, SignalElement::Annotation)
    }
}

/// A property
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Property {
    name: String,
    r#type: String,
    access: String,

    #[serde(rename = "annotation", default)]
    annotations: Vec<Annotation>,
}

assert_impl_all!(Property: Send, Sync, Unpin);

impl Property {
    /// Returns the property name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the property type.
    pub fn ty(&self) -> &str {
        &self.r#type
    }

    /// Returns the property access flags (should be "read", "write" or "readwrite").
    pub fn access(&self) -> &str {
        &self.access
    }

    /// Return the associated annotations.
    pub fn annotations(&self) -> Vec<&Annotation> {
        self.annotations.iter().collect()
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
enum InterfaceElement {
    Method(Method),
    Signal(Signal),
    Property(Property),
    Annotation(Annotation),
}

/// An interface
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Interface {
    name: String,

    #[serde(rename = "$value", default)]
    elems: Vec<InterfaceElement>,
}

assert_impl_all!(Interface: Send, Sync, Unpin);

impl Interface {
    /// Returns the interface name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the interface methods.
    pub fn methods(&self) -> Vec<&Method> {
        get_vec!(self.elems, InterfaceElement::Method)
    }

    /// Returns the interface signals.
    pub fn signals(&self) -> Vec<&Signal> {
        get_vec!(self.elems, InterfaceElement::Signal)
    }

    /// Returns the interface properties.
    pub fn properties(&self) -> Vec<&Property> {
        get_vec!(self.elems, InterfaceElement::Property)
    }

    /// Return the associated annotations.
    pub fn annotations(&self) -> Vec<&Annotation> {
        get_vec!(self.elems, InterfaceElement::Annotation)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
enum NodeElement {
    Node(Node),
    Interface(Interface),
}

/// An introspection tree node (typically the root of the XML document).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Node {
    name: Option<String>,

    #[serde(rename = "$value", default)]
    elems: Vec<NodeElement>,
}

assert_impl_all!(Node: Send, Sync, Unpin);

impl Node {
    /// Parse the introspection XML document from reader.
    pub fn from_reader<R: Read>(reader: R) -> Result<Node, Error> {
        Ok(from_reader(reader)?)
    }

    /// Write the XML document to writer.
    pub fn to_writer<W: Write>(&self, writer: W) -> Result<(), Error> {
        Ok(to_writer(writer, &self)?)
    }

    /// Returns the node name, if any.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Returns the children nodes.
    pub fn nodes(&self) -> Vec<&Node> {
        get_vec!(self.elems, NodeElement::Node)
    }

    /// Returns the interfaces on this node.
    pub fn interfaces(&self) -> Vec<&Interface> {
        get_vec!(self.elems, NodeElement::Interface)
    }
}

impl std::str::FromStr for Node {
    type Err = Error;

    /// Parse the introspection XML document from `s`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(from_str(s)?)
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error, str::FromStr};
    use test_log::test;

    use super::Node;

    static EXAMPLE: &str = r##"
<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN"
  "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
 <node name="/com/example/sample_object0">
   <node name="first"/>
   <interface name="com.example.SampleInterface0">
     <method name="Frobate">
       <arg name="foo" type="i" direction="in"/>
       <arg name="bar" type="s" direction="out"/>
       <arg name="baz" type="a{us}" direction="out"/>
       <annotation name="org.freedesktop.DBus.Deprecated" value="true"/>
     </method>
     <method name="Bazify">
       <arg name="bar" type="(iiu)" direction="in"/>
       <arg name="bar" type="v" direction="out"/>
     </method>
     <method name="Mogrify">
       <arg name="bar" type="(iiav)" direction="in"/>
     </method>
     <signal name="Changed">
       <arg name="new_value" type="b"/>
     </signal>
     <property name="Bar" type="y" access="readwrite"/>
   </interface>
   <node name="child_of_sample_object"/>
   <node name="another_child_of_sample_object"/>
</node>
"##;

    #[test]
    fn serde() -> Result<(), Box<dyn Error>> {
        let node = Node::from_reader(EXAMPLE.as_bytes())?;
        assert_eq!(node.interfaces().len(), 1);
        assert_eq!(node.nodes().len(), 3);

        let node_str = Node::from_str(EXAMPLE)?;
        assert_eq!(node_str.interfaces().len(), 1);
        assert_eq!(node_str.nodes().len(), 3);

        // TODO: Fails at the moment, this seems fresh & related:
        // https://github.com/RReverser/serde-xml-rs/pull/129
        //let mut writer = Vec::with_capacity(128);
        //node.to_writer(&mut writer).unwrap();
        Ok(())
    }
}
