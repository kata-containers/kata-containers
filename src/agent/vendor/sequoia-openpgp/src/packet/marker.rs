#[cfg(test)]
use quickcheck::{Arbitrary, Gen};

use crate::packet;
use crate::Packet;

/// Holds a Marker packet.
///
/// See [Section 5.8 of RFC 4880] for details.
///
///   [Section 5.8 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.8
// IMPORTANT: If you add fields to this struct, you need to explicitly
// IMPORTANT: implement PartialEq, Eq, and Hash.
#[derive(Default, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Marker {
    /// CTB packet header fields.
    pub(crate) common: packet::Common,
}
assert_send_and_sync!(Marker);

impl Marker {
    pub(crate) const BODY: &'static [u8] = &[0x50, 0x47, 0x50];
}

impl From<Marker> for Packet {
    fn from(p: Marker) -> Self {
        Packet::Marker(p)
    }
}

#[cfg(test)]
impl Arbitrary for Marker {
    fn arbitrary(_: &mut Gen) -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::Parse;
    use crate::serialize::MarshalInto;

    #[test]
    fn roundtrip() {
        let p = Marker::default();
        let q = Marker::from_bytes(&p.to_vec().unwrap()).unwrap();
        assert_eq!(p, q);
    }
}
