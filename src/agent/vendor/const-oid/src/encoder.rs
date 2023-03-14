//! OID encoder with `const` support.

use crate::{
    arcs::{ARC_MAX_FIRST, ARC_MAX_SECOND},
    Arc, Error, ObjectIdentifier, Result,
};

/// BER/DER encoder
pub(crate) struct Encoder {
    /// Current state
    state: State,

    /// Bytes of the OID being encoded in-progress
    bytes: [u8; ObjectIdentifier::MAX_SIZE],

    /// Current position within the byte buffer
    cursor: usize,
}

/// Current state of the encoder
enum State {
    /// Initial state - no arcs yet encoded
    Initial,

    /// First arc parsed
    FirstArc(Arc),

    /// Encoding base 128 body of the OID
    Body,
}

impl Encoder {
    /// Create a new encoder initialized to an empty default state
    pub(crate) const fn new() -> Self {
        Self {
            state: State::Initial,
            bytes: [0u8; ObjectIdentifier::MAX_SIZE],
            cursor: 0,
        }
    }

    /// Encode an [`Arc`] as base 128 into the internal buffer
    pub(crate) const fn encode(mut self, arc: Arc) -> Self {
        match self.state {
            State::Initial => {
                const_assert!(arc <= ARC_MAX_FIRST, "invalid first arc (must be 0-2)");
                self.state = State::FirstArc(arc);
                self
            }
            State::FirstArc(first_arc) => {
                const_assert!(arc <= ARC_MAX_SECOND, "invalid second arc (must be 0-39)");
                self.state = State::Body;
                self.bytes[0] = (first_arc * (ARC_MAX_SECOND + 1)) as u8 + arc as u8;
                self.cursor = 1;
                self
            }
            State::Body => {
                // Total number of bytes in encoded arc - 1
                let nbytes = base128_len(arc);

                const_assert!(
                    self.cursor + nbytes + 1 < ObjectIdentifier::MAX_SIZE,
                    "OID too long (exceeded max DER bytes)"
                );

                let new_cursor = self.cursor + nbytes + 1;
                let mut result = self.encode_base128_byte(arc, nbytes, false);
                result.cursor = new_cursor;
                result
            }
        }
    }

    /// Finish encoding an OID
    pub(crate) const fn finish(self) -> ObjectIdentifier {
        const_assert!(self.cursor >= 2, "OID too short (minimum 3 arcs)");
        ObjectIdentifier {
            bytes: self.bytes,
            length: self.cursor as u8,
        }
    }

    /// Encode a single byte of a base128 value
    const fn encode_base128_byte(mut self, mut n: u32, i: usize, continued: bool) -> Self {
        let mask = if continued { 0b10000000 } else { 0 };

        if n > 0x80 {
            self.bytes[self.cursor + i] = (n & 0b1111111) as u8 | mask;
            n >>= 7;

            const_assert!(i > 0, "Base 128 offset miscalculation");
            self.encode_base128_byte(n, i.saturating_sub(1), true)
        } else {
            self.bytes[self.cursor] = n as u8 | mask;
            self
        }
    }
}

/// Compute the length - 1 of an arc when encoded in base 128
const fn base128_len(arc: Arc) -> usize {
    match arc {
        0..=0x7f => 0,
        0x80..=0x3fff => 1,
        0x4000..=0x1fffff => 2,
        0x200000..=0x1fffffff => 3,
        _ => 4,
    }
}

/// Write the given unsigned integer in base 128
// TODO(tarcieri): consolidate encoding logic with `encode_base128_byte`
pub(crate) fn write_base128(bytes: &mut [u8], mut n: Arc) -> Result<usize> {
    let nbytes = base128_len(n);
    let mut i = nbytes;
    let mut mask = 0;

    while n > 0x80 {
        let byte = bytes.get_mut(i).ok_or(Error)?;
        *byte = (n & 0b1111111 | mask) as u8;
        n >>= 7;
        i = i.checked_sub(1).expect("overflow");
        mask = 0b10000000;
    }

    bytes[0] = (n | mask) as u8;

    Ok(nbytes + 1)
}

#[cfg(test)]
mod tests {
    use super::Encoder;
    use hex_literal::hex;

    /// OID `1.2.840.10045.2.1` encoded as ASN.1 BER/DER
    const EXAMPLE_OID_BER: &[u8] = &hex!("2A8648CE3D0201");

    #[test]
    fn encode() {
        let encoder = Encoder::new();
        let encoder = encoder.encode(1);
        let encoder = encoder.encode(2);
        let encoder = encoder.encode(840);
        let encoder = encoder.encode(10045);
        let encoder = encoder.encode(2);
        let encoder = encoder.encode(1);
        assert_eq!(&encoder.bytes[..encoder.cursor], EXAMPLE_OID_BER);
    }
}
