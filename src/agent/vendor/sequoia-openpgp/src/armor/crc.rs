//! Computes the CRC-24, (see [RFC 4880, section 6.1]).
//!
//! [RFC 4880, section 6.1]: https://tools.ietf.org/html/rfc4880#section-6.1

const CRC24_INIT: u32 = 0xB704CE;
const CRC24_POLY: u32 = 0x864CFB;

#[derive(Debug)]
pub struct Crc {
    n: u32,
}

/// Computes the CRC-24, (see [RFC 4880, section 6.1]).
///
/// [RFC 4880, section 6.1]: https://tools.ietf.org/html/rfc4880#section-6.1
impl Crc {
    pub fn new() -> Self {
        Self { n: CRC24_INIT }
    }

    /// Updates the CRC sum using the given data.
    ///
    /// This implementation uses a lookup table.  See:
    ///
    /// Sarwate, Dilip V. "Computation of cyclic redundancy checks via
    /// table look-up." Communications of the ACM 31.8 (1988):
    /// 1008-1013.
    pub fn update(&mut self, buf: &[u8]) -> &Self {
        lazy_static::lazy_static! {
            static ref TABLE: Vec<u32> = {
                let mut t = vec![0u32; 256];

                let mut crc = 0x80_0000; // 24 bit polynomial
                let mut i = 1;
                loop {
                    if crc & 0x80_0000 > 0 {
                        crc = (crc << 1) ^ CRC24_POLY;
                    } else {
                        crc <<= 1;
                    }
                    for j in 0..i {
                        t[i + j] = crc ^ t[j];
                    }
                    i <<= 1;
                    if i == 256 {
                        break;
                    }
                }
                t
            };
        }

        for octet in buf {
            self.n = (self.n << 8)
                ^ TABLE[(*octet ^ ((self.n >> 16) as u8)) as usize];
        }

        self
    }

    pub fn finalize(&self) -> u32 {
        self.n & 0xFFFFFF
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foobarbaz() {
        let b = b"foobarbaz";
        let crcs = [
            0xb704ce,
            0x6d2804,
            0xa2d10d,
            0x4fc255,
            0x7aafca,
            0xc79c46,
            0x7334de,
            0x77dc72,
            0x000f65,
            0xf40d86,
        ];

        for len in 0..b.len() + 1 {
            assert_eq!(Crc::new().update(&b[..len]).finalize(), crcs[len]);
        }
    }

    /// Reference implementation of the iterative CRC24 computation.
    fn iterative(buf: &[u8]) -> u32 {
        let mut n = CRC24_INIT;
        for octet in buf {
            n ^= (*octet as u32) << 16;
            for _ in 0..8 {
                n <<= 1;
                if n & 0x1000000 > 0 {
                    n ^= CRC24_POLY;
                }
            }
        }
        n & 0xFFFFFF
    }

    quickcheck! {
        fn compare(b: Vec<u8>) -> bool {
            let mut c = Crc::new();
            c.update(&b);
            assert_eq!(c.finalize(), iterative(&b));
            true
        }
    }
}
