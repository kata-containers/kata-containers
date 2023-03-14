use std::fmt;

#[cfg(test)]
use quickcheck::{Arbitrary, Gen};

use crate::packet;
use crate::Packet;

/// Holds a Trust packet.
///
/// Trust packets are used to hold implementation specific information
/// in an implementation-defined format.  Trust packets are normally
/// not exported.
///
/// See [Section 5.10 of RFC 4880] for details.
///
///   [Section 5.10 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.10
// IMPORTANT: If you add fields to this struct, you need to explicitly
// IMPORTANT: implement PartialEq, Eq, and Hash.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Trust {
    pub(crate) common: packet::Common,
    value: Vec<u8>,
}

assert_send_and_sync!(Trust);

impl From<Vec<u8>> for Trust {
    fn from(u: Vec<u8>) -> Self {
        Trust {
            common: Default::default(),
            value: u,
        }
    }
}

impl fmt::Display for Trust {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let trust = String::from_utf8_lossy(&self.value[..]);
        write!(f, "{}", trust)
    }
}

impl fmt::Debug for Trust {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Trust")
            .field("value", &crate::fmt::hex::encode(&self.value))
            .finish()
    }
}

impl Trust {
    /// Gets the trust packet's value.
    pub fn value(&self) -> &[u8] {
        self.value.as_slice()
    }
}

impl From<Trust> for Packet {
    fn from(s: Trust) -> Self {
        Packet::Trust(s)
    }
}

#[cfg(test)]
impl Arbitrary for Trust {
    fn arbitrary(g: &mut Gen) -> Self {
        Vec::<u8>::arbitrary(g).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::Parse;
    use crate::serialize::MarshalInto;

    quickcheck! {
        fn roundtrip(p: Trust) -> bool {
            let q = Trust::from_bytes(&p.to_vec().unwrap()).unwrap();
            assert_eq!(p, q);
            true
        }
    }
}
