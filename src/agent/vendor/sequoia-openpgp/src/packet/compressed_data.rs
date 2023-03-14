use std::fmt;

#[cfg(test)]
use quickcheck::{Arbitrary, Gen};

use crate::packet;
use crate::Packet;
use crate::types::CompressionAlgorithm;

/// Holds a compressed data packet.
///
/// A compressed data packet is a container.  See [Section 5.6 of RFC
/// 4880] for details.
///
/// When the parser encounters a compressed data packet with an
/// unknown compress algorithm, it returns an `Unknown` packet instead
/// of a `CompressedData` packet.
///
/// [Section 5.6 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.6
// IMPORTANT: If you add fields to this struct, you need to explicitly
// IMPORTANT: implement PartialEq, Eq, and Hash.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CompressedData {
    /// CTB packet header fields.
    pub(crate) common: packet::Common,
    /// Algorithm used to compress the payload.
    algo: CompressionAlgorithm,

    /// This is a container packet.
    container: packet::Container,
}
assert_send_and_sync!(CompressedData);

impl std::ops::Deref for CompressedData {
    type Target = packet::Container;
    fn deref(&self) -> &Self::Target {
        &self.container
    }
}

impl std::ops::DerefMut for CompressedData {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.container
    }
}

impl fmt::Debug for CompressedData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CompressedData")
            .field("algo", &self.algo)
            .field("container", &self.container)
            .finish()
    }
}

impl CompressedData {
    /// Returns a new `CompressedData` packet.
    pub fn new(algo: CompressionAlgorithm) -> Self {
        CompressedData {
            common: Default::default(),
            algo,
            container: Default::default(),
        }
    }

    /// Gets the compression algorithm.
    pub fn algo(&self) -> CompressionAlgorithm {
        self.algo
    }

    /// Sets the compression algorithm.
    pub fn set_algo(&mut self, algo: CompressionAlgorithm) -> CompressionAlgorithm {
        ::std::mem::replace(&mut self.algo, algo)
    }

    /// Adds a new packet to the container.
    #[cfg(test)]
    pub fn push(mut self, packet: Packet) -> Self {
        self.container.children_mut().unwrap().push(packet);
        self
    }

    /// Inserts a new packet to the container at a particular index.
    /// If `i` is 0, the new packet is insert at the front of the
    /// container.  If `i` is one, it is inserted after the first
    /// packet, etc.
    #[cfg(test)]
    pub fn insert(mut self, i: usize, packet: Packet) -> Self {
        self.container.children_mut().unwrap().insert(i, packet);
        self
    }
}

impl From<CompressedData> for Packet {
    fn from(s: CompressedData) -> Self {
        Packet::CompressedData(s)
    }
}

#[cfg(test)]
impl Arbitrary for CompressedData {
    fn arbitrary(g: &mut Gen) -> Self {
        use crate::serialize::SerializeInto;
        use crate::arbitrary_helper::gen_arbitrary_from_range;

        loop {
            let a =
                CompressionAlgorithm::from(gen_arbitrary_from_range(0..4, g));
            if a.is_supported() {
                let mut c = CompressedData::new(a);
                // We arbitrarily chose to create packets with
                // processed bodies, so that
                // Packet::from_bytes(c.to_vec()) will roundtrip them.
                c.set_body(packet::Body::Processed(
                    Packet::arbitrary(g).to_vec().unwrap()
                ));
                return c;
            }
        }
    }
}
