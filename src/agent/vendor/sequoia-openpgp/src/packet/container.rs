//! Packet container support.
//!
//! Some packets contain other packets.  This creates a tree
//! structure.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::slice;
use std::vec;

use xxhash_rust::xxh3::Xxh3;

use crate::{
    Packet,
    packet::Iter,
};

/// A packet's body holds either unprocessed bytes, processed bytes,
/// or packets.
///
/// We conceptually divide packets into two parts: the header and the
/// body.  Whereas the header is read eagerly when the packet is
/// deserialized, the body is only read on demand.
///
/// A packet's body is stored here either when configured via
/// [`PacketParserBuilder::buffer_unread_content`], when one of the
/// [`PacketPile`] deserialization routines is used, or on demand for
/// a particular packet using the
/// [`PacketParser::buffer_unread_content`] method.
///
///   [`PacketParserBuilder::buffer_unread_content`]: crate::parse::PacketParserBuilder::buffer_unread_content()
///   [`PacketPile`]: crate::PacketPile
///   [`PacketParser::buffer_unread_content`]: crate::parse::PacketParser::buffer_unread_content()
///
/// There are three different types of packets:
///
///   - Most packets, like the [`UserID`] and [`Signature`] packets, don't
///     actually have a body.
///
///   [`UserID`]: crate::packet::UserID
///   [`Signature`]: crate::packet::Signature
///
///   - Some packets have an unprocessed body.  The [`Literal`] data
///     packet wraps unstructured plaintext, and the [`Unknown`]
///     packet contains data that we failed to process, say because we
///     didn't support the packet's version.
///
///   [`Literal`]: crate::packet::Literal
///   [`Unknown`]: crate::packet::Unknown
///
///   - Some packets are containers.  If the parser does not parse the
///     packet's child, either because the caller used
///     [`PacketParser::next`] to get the next packet, or the maximum
///     recursion depth was reached, then the packets can be stored
///     here as an unstructured byte stream.  (If the caller so
///     chooses, the content can be parsed later using the regular
///     deserialization routines, since the content is just an OpenPGP
///     message.)
///
///   [`PacketParser::next`]: crate::parse::PacketParser::next()
#[derive(Clone, Debug)]
pub enum Body {
    /// Unprocessed packet body.
    ///
    /// The body has not been processed.  This happens in the
    /// following cases:
    ///
    ///   - The packet is a [`Literal`] packet.
    ///
    ///   - The packet is an [`Unknown`] packet, i.e. it contains data
    ///     that we failed to process, say because we didn't support
    ///     the packet's version.
    ///
    ///   - The packet is an encryption container ([`SEIP`] or
    ///     [`AED`]) and the body is encrypted.
    ///
    /// Note: if some of a packet's data is streamed, and the
    /// `PacketParser` is configured to buffer unread content, then
    /// this is not the packet's entire content; it is just the unread
    /// content.
    ///
    ///   [`Literal`]: crate::packet::Literal
    ///   [`Unknown`]: crate::packet::Unknown
    ///   [`SEIP`]: crate::packet::SEIP
    ///   [`AED`]: crate::packet::AED
    Unprocessed(Vec<u8>),

    /// Processed packed body.
    ///
    /// The body has been processed, i.e. decompressed or decrypted,
    /// but not parsed into packets.
    ///
    /// Note: if some of a packet's data is streamed, and the
    /// `PacketParser` is configured to buffer unread content, then
    /// this is not the packet's entire content; it is just the unread
    /// content.
    Processed(Vec<u8>),

    /// Parsed packet body.
    ///
    /// Used by container packets (such as the encryption and
    /// compression packets) to reference their immediate children.
    /// This results in a tree structure.
    ///
    /// This is automatically populated when using the [`PacketPile`]
    /// deserialization routines, e.g., [`PacketPile::from_file`].  By
    /// default, it is *not* automatically filled in by the
    /// [`PacketParser`] deserialization routines; this needs to be
    /// done manually.
    ///
    ///   [`PacketPile`]: crate::PacketPile
    ///   [`PacketPile::from_file`]: crate::PacketPile#method.from_file
    ///   [`PacketParser`]: crate::parse::PacketParser
    Structured(Vec<Packet>),
}

assert_send_and_sync!(Body);

/// Holds packet bodies.
///
/// This is used by OpenPGP container packets, like the compressed
/// data packet, to store the containing packets.
#[derive(Clone)]
pub struct Container {
    /// Holds a packet's body.
    body: Body,

    /// We compute a digest over the body to implement comparison.
    body_digest: u64,
}

assert_send_and_sync!(Container);

impl std::ops::Deref for Container {
    type Target = Body;
    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

impl PartialEq for Container {
    fn eq(&self, other: &Container) -> bool {
        use Body::*;
        match (&self.body, &other.body) {
            (Unprocessed(_), Unprocessed(_)) =>
                self.body_digest == other.body_digest,
            (Processed(_), Processed(_)) =>
                self.body_digest == other.body_digest,
            (Structured(a), Structured(b)) =>
                a == b,
            _ => false,
        }
    }
}

impl Eq for Container {}

impl Hash for Container {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if let Body::Structured(packets) = &self.body {
            packets.hash(state);
        } else {
            self.body_digest.hash(state);
        }
    }
}

impl Default for Container {
    fn default() -> Self {
        Self {
            body: Body::Structured(Vec::with_capacity(0)),
            body_digest: 0,
        }
    }
}

impl From<Vec<Packet>> for Container {
    fn from(packets: Vec<Packet>) -> Self {
        Self {
            body: Body::Structured(packets),
            body_digest: 0,
        }
    }
}

impl fmt::Debug for Container {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fn fmt_bytes(f: &mut fmt::Formatter, tag: &str, bytes: &[u8],
                     digest: String)
                     -> fmt::Result
        {
            let threshold = 16;
            let prefix = &bytes[..std::cmp::min(threshold, bytes.len())];
            let mut prefix_fmt = crate::fmt::hex::encode(prefix);
            if bytes.len() > threshold {
                prefix_fmt.push_str("...");
            }
            prefix_fmt.push_str(&format!(" ({} bytes)", bytes.len())[..]);

            f.debug_struct("Container")
                .field(tag, &prefix_fmt)
                .field("digest", &digest)
                .finish()
        }

        use Body::*;
        match &self.body {
            Unprocessed(bytes) =>
                fmt_bytes(f, "unprocessed", bytes, self.body_digest()),
            Processed(bytes) =>
                fmt_bytes(f, "processed", bytes, self.body_digest()),
            Structured(packets) =>
                f.debug_struct("Container").field("packets", packets).finish(),
        }
    }
}

impl Container {
    pub(crate) fn default_unprocessed() -> Self {
        Self {
            body: Body::Unprocessed(Vec::with_capacity(0)),
            body_digest: Self::empty_body_digest(),
        }
    }

    /// Returns a reference to this Packet's children.
    ///
    /// Returns `None` if the body is not structured.
    pub fn children_ref(&self) -> Option<&[Packet]> {
        if let Body::Structured(packets) = &self.body {
            Some(&packets[..])
        } else {
            None
        }
    }

    /// Returns a mutable reference to this Packet's children.
    ///
    /// Returns `None` if the body is not structured.
    pub fn children_mut(&mut self) -> Option<&mut Vec<Packet>> {
        if let Body::Structured(packets) = &mut self.body {
            Some(packets)
        } else {
           None
        }
    }

    /// Returns an iterator over the packet's descendants.  The
    /// descendants are visited in depth-first order.
    ///
    /// Returns `None` if the body is not structured.
    pub fn descendants(&self) -> Option<Iter> {
        Some(Iter {
            // Iterate over each packet in the message.
            children: self.children()?,
            child: None,
            grandchildren: None,
            depth: 0,
        })
    }

    /// Returns an iterator over the packet's immediate children.
    ///
    /// Returns `None` if the body is not structured.
    pub fn children(&self) -> Option<slice::Iter<Packet>> {
        Some(self.children_ref()?.iter())
    }

    /// Returns an `IntoIter` over the packet's immediate children.
    ///
    /// Returns `None` if the body is not structured.
    pub fn into_children(self) -> Option<vec::IntoIter<Packet>> {
        if let Body::Structured(packets) = self.body {
            Some(packets.into_iter())
        } else {
            None
        }
    }

    /// Gets the packet's body.
    pub fn body(&self) -> &Body {
        &self.body
    }

    /// Sets the packet's body.
    pub fn set_body(&mut self, body: Body) -> Body {
        use Body::*;
        let mut h = Self::make_body_hash();
        match &body {
            Unprocessed(bytes) => h.update(bytes),
            Processed(bytes) => h.update(bytes),
            Structured(_) => (),
        }
        self.set_body_hash(h);
        std::mem::replace(&mut self.body, body)
    }

    /// Returns the hash for the empty body.
    fn empty_body_digest() -> u64 {
        lazy_static::lazy_static!{
            static ref DIGEST: u64 = {
                Container::make_body_hash().digest()
            };
        }

        *DIGEST
    }

    /// Creates a hash context for hashing the body.
    pub(crate) // For parse.rs
    fn make_body_hash() -> Box<Xxh3> {
        Box::new(Xxh3::new())
    }

    /// Hashes content that has been streamed.
    pub(crate) // For parse.rs
    fn set_body_hash(&mut self, h: Box<Xxh3>) {
        self.body_digest = h.digest();
    }

    pub(crate)
    fn body_digest(&self) -> String {
        format!("{:08X}", self.body_digest)
    }

    // Converts an indentation level to whitespace.
    fn indent(depth: usize) -> &'static str {
        use std::cmp;

        let s = "                                                  ";
        &s[0..cmp::min(depth, s.len())]
    }

    // Pretty prints the container to stderr.
    //
    // This function is primarily intended for debugging purposes.
    //
    // `indent` is the number of spaces to indent the output.
    pub(crate) fn pretty_print(&self, indent: usize) {
        for (i, p) in self.children_ref().iter().enumerate() {
            eprintln!("{}{}: {:?}",
                      Self::indent(indent), i + 1, p);
            if let Some(children) = self.children_ref()
                .and_then(|c| c.get(i)).and_then(|p| p.container_ref())
            {
                children.pretty_print(indent + 1);
            }
        }
    }
}

macro_rules! impl_body_forwards {
    ($typ:ident) => {
        /// This packet implements the unprocessed container
        /// interface.
        ///
        /// Container packets like this one can contain unprocessed
        /// data.
        impl $typ {
            /// Returns a reference to the container.
            pub(crate) fn container_ref(&self) -> &packet::Container {
                &self.container
            }

            /// Returns a mutable reference to the container.
            pub(crate) fn container_mut(&mut self) -> &mut packet::Container {
                &mut self.container
            }

            /// Gets a reference to the this packet's body.
            pub fn body(&self) -> &[u8] {
                use crate::packet::Body::*;
                match self.container.body() {
                    Unprocessed(bytes) => bytes,
                    Processed(_) => unreachable!(
                        "Unprocessed container has processed body"),
                    Structured(_) => unreachable!(
                        "Unprocessed container has structured body"),
                }
            }

            /// Sets the this packet's body.
            pub fn set_body(&mut self, data: Vec<u8>) -> Vec<u8> {
                use crate::packet::{Body, Body::*};
                match self.container.set_body(Body::Unprocessed(data)) {
                    Unprocessed(bytes) => bytes,
                    Processed(_) => unreachable!(
                        "Unprocessed container has processed body"),
                    Structured(_) => unreachable!(
                        "Unprocessed container has structured body"),
                }
            }
        }
    };
}

impl Packet {
    pub(crate) // for packet_pile.rs
    fn container_ref(&self) -> Option<&Container> {
        use std::ops::Deref;
        match self {
            Packet::CompressedData(p) => Some(p.deref()),
            Packet::SEIP(p) => Some(p.deref()),
            Packet::AED(p) => Some(p.deref()),
            Packet::Literal(p) => Some(p.container_ref()),
            Packet::Unknown(p) => Some(p.container_ref()),
            _ => None,
        }
    }

    pub(crate) // for packet_pile.rs, packet_pile_parser.rs, parse.rs
    fn container_mut(&mut self) -> Option<&mut Container> {
        use std::ops::DerefMut;
        match self {
            Packet::CompressedData(p) => Some(p.deref_mut()),
            Packet::SEIP(p) => Some(p.deref_mut()),
            Packet::AED(p) => Some(p.deref_mut()),
            Packet::Literal(p) => Some(p.container_mut()),
            Packet::Unknown(p) => Some(p.container_mut()),
            _ => None,
        }
    }

    /// Returns an iterator over the packet's immediate children.
    pub(crate) fn children(& self)
                           -> Option<impl Iterator<Item = &Packet>> {
        self.container_ref().and_then(|c| c.children())
    }

    /// Returns an iterator over all of the packet's descendants, in
    /// depth-first order.
    pub(crate) fn descendants(&self) -> Option<Iter> {
        self.container_ref().and_then(|c| c.descendants())
    }

    /// Retrieves the packet's unprocessed body.
    #[cfg(test)]
    #[allow(dead_code)] // Not used if no compression feature is enabled.
    pub(crate) fn unprocessed_body(&self) -> Option<&[u8]> {
        self.container_ref().and_then(|c| match c.body() {
            Body::Unprocessed(bytes) => Some(&bytes[..]),
            _ => None,
        })
    }

    /// Retrieves the packet's processed body.
    #[cfg(test)]
    pub(crate) fn processed_body(&self) -> Option<&[u8]> {
        self.container_ref().and_then(|c| match c.body() {
            Body::Processed(bytes) => Some(&bytes[..]),
            _ => None,
        })
    }
}
