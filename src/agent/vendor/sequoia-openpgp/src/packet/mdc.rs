use std::cmp::Ordering;

use crate::crypto;
use crate::crypto::mem;
use crate::packet;
use crate::Packet;

/// Holds an MDC packet.
///
/// A modification detection code packet.  This packet appears after a
/// SEIP packet.  See [Section 5.14 of RFC 4880] for details.
///
/// [Section 5.14 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.14
///
/// # A note on equality
///
/// Two `MDC` packets are considered equal if their serialized form is
/// equal.  This excludes the computed digest.
#[derive(Clone, Debug)]
pub struct MDC {
    /// CTB packet header fields.
    pub(crate) common: packet::Common,
    /// Our SHA-1 hash.
    computed_digest: [u8; 20],
    /// A 20-octet SHA-1 hash of the preceding plaintext data.
    digest: [u8; 20],
}
assert_send_and_sync!(MDC);

impl PartialEq for MDC {
    fn eq(&self, other: &MDC) -> bool {
        self.common == other.common
            && self.digest == other.digest
    }
}

impl Eq for MDC {}

impl std::hash::Hash for MDC {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.common, state);
        std::hash::Hash::hash(&self.digest, state);
    }
}

impl MDC {
    /// Creates an MDC packet.
    pub fn new(digest: [u8; 20], computed_digest: [u8; 20]) -> Self {
        MDC {
            common: Default::default(),
            computed_digest,
            digest,
        }
    }

    /// Gets the packet's hash value.
    pub fn digest(&self) -> &[u8] {
        &self.digest[..]
    }

    /// Gets the computed hash value.
    pub fn computed_digest(&self) -> &[u8] {
        &self.computed_digest[..]
    }

    /// Returns whether the data protected by the MDC is valid.
    pub fn valid(&self) -> bool {
        if self.digest == [ 0; 20 ] {
            // If the computed_digest and digest are uninitialized, then
            // return false.
            false
        } else {
            mem::secure_cmp(&self.computed_digest, &self.digest) == Ordering::Equal
        }
    }
}

impl From<MDC> for Packet {
    fn from(s: MDC) -> Self {
        Packet::MDC(s)
    }
}

impl From<[u8; 20]> for MDC {
    fn from(digest: [u8; 20]) -> Self {
        MDC {
            common: Default::default(),
            // All 0s.
            computed_digest: Default::default(),
            digest,
        }
    }
}

impl From<Box<dyn crypto::hash::Digest>> for MDC {
    fn from(mut hash: Box<dyn crypto::hash::Digest>) -> Self {
        let mut value : [u8; 20] = Default::default();
        let _ = hash.digest(&mut value[..]);
        value.into()
    }
}
