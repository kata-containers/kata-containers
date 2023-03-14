//! Const-friendly decoding operations for [`UInt`]

mod decoder;

#[cfg(feature = "rlp")]
mod rlp;

use super::UInt;
use crate::{Encoding, Limb};
use decoder::Decoder;

impl<const LIMBS: usize> UInt<LIMBS> {
    /// Create a new [`UInt`] from the provided big endian bytes.
    pub const fn from_be_slice(bytes: &[u8]) -> Self {
        const_assert!(
            bytes.len() == Limb::BYTE_SIZE * LIMBS,
            "bytes are not the expected size"
        );

        let mut decoder = Decoder::new();
        let mut i = 0;

        while i < Limb::BYTE_SIZE * LIMBS {
            i += 1;
            decoder = decoder.add_byte(bytes[bytes.len() - i]);
        }

        decoder.finish()
    }

    /// Create a new [`UInt`] from the provided big endian hex string.
    pub const fn from_be_hex(hex: &str) -> Self {
        let bytes = hex.as_bytes();

        const_assert!(
            bytes.len() == Limb::BYTE_SIZE * LIMBS * 2,
            "hex string is not the expected size"
        );

        let mut decoder = Decoder::new();
        let mut i = 0;

        while i < Limb::BYTE_SIZE * LIMBS * 2 {
            i += 2;
            let offset = bytes.len() - i;
            let byte = decode_hex_byte([bytes[offset], bytes[offset + 1]]);
            decoder = decoder.add_byte(byte);
        }

        decoder.finish()
    }

    /// Create a new [`UInt`] from the provided little endian bytes.
    pub const fn from_le_slice(bytes: &[u8]) -> Self {
        const_assert!(
            bytes.len() == Limb::BYTE_SIZE * LIMBS,
            "bytes are not the expected size"
        );

        let mut decoder = Decoder::new();
        let mut i = 0;

        while i < Limb::BYTE_SIZE * LIMBS {
            decoder = decoder.add_byte(bytes[i]);
            i += 1;
        }

        decoder.finish()
    }

    /// Create a new [`UInt`] from the provided little endian hex string.
    pub const fn from_le_hex(hex: &str) -> Self {
        let bytes = hex.as_bytes();

        const_assert!(
            bytes.len() == Limb::BYTE_SIZE * LIMBS * 2,
            "bytes are not the expected size"
        );

        let mut decoder = Decoder::new();
        let mut i = 0;

        while i < Limb::BYTE_SIZE * LIMBS * 2 {
            let byte = decode_hex_byte([bytes[i], bytes[i + 1]]);
            decoder = decoder.add_byte(byte);
            i += 2;
        }

        decoder.finish()
    }

    /// Serialize this [`UInt`] as big-endian, writing it into the provided
    /// byte slice.
    #[inline]
    #[cfg_attr(docsrs, doc(cfg(feature = "generic-array")))]
    pub(crate) fn write_be_bytes(&self, out: &mut [u8]) {
        debug_assert_eq!(out.len(), Limb::BYTE_SIZE * LIMBS);

        for (src, dst) in self
            .limbs
            .iter()
            .rev()
            .cloned()
            .zip(out.chunks_exact_mut(Limb::BYTE_SIZE))
        {
            dst.copy_from_slice(&src.to_be_bytes());
        }
    }

    /// Serialize this [`UInt`] as little-endian, writing it into the provided
    /// byte slice.
    #[inline]
    #[cfg_attr(docsrs, doc(cfg(feature = "generic-array")))]
    pub(crate) fn write_le_bytes(&self, out: &mut [u8]) {
        debug_assert_eq!(out.len(), Limb::BYTE_SIZE * LIMBS);

        for (src, dst) in self
            .limbs
            .iter()
            .cloned()
            .zip(out.chunks_exact_mut(Limb::BYTE_SIZE))
        {
            dst.copy_from_slice(&src.to_le_bytes());
        }
    }
}

/// Decode a single byte encoded as two hexadecimal characters.
const fn decode_hex_byte(bytes: [u8; 2]) -> u8 {
    let mut i = 0;
    let mut result = 0u8;

    while i < 2 {
        result <<= 4;
        result |= match bytes[i] {
            b @ b'0'..=b'9' => b - b'0',
            b @ b'a'..=b'f' => 10 + b - b'a',
            b @ b'A'..=b'F' => 10 + b - b'A',
            b => {
                const_assert!(
                    matches!(b, b'0'..=b'9' | b'a' ..= b'f' | b'A'..=b'F'),
                    "invalid hex byte"
                );
                0
            }
        };

        i += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use crate::Limb;
    use hex_literal::hex;

    #[cfg(feature = "alloc")]
    use {crate::U128, alloc::format};

    #[cfg(target_pointer_width = "32")]
    use crate::U64 as UIntEx;

    #[cfg(target_pointer_width = "64")]
    use crate::U128 as UIntEx;

    #[test]
    #[cfg(target_pointer_width = "32")]
    fn from_be_bytes() {
        let bytes = hex!("0011223344556677");
        let n = UIntEx::from_be_slice(&bytes);
        assert_eq!(n.limbs(), &[Limb(0x44556677), Limb(0x00112233)]);
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn from_be_bytes() {
        let bytes = hex!("00112233445566778899aabbccddeeff");
        let n = UIntEx::from_be_slice(&bytes);
        assert_eq!(
            n.limbs(),
            &[Limb(0x8899aabbccddeeff), Limb(0x0011223344556677)]
        );
    }

    #[test]
    #[cfg(target_pointer_width = "32")]
    fn from_le_bytes() {
        let bytes = hex!("7766554433221100");
        let n = UIntEx::from_le_slice(&bytes);
        assert_eq!(n.limbs(), &[Limb(0x44556677), Limb(0x00112233)]);
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn from_le_bytes() {
        let bytes = hex!("ffeeddccbbaa99887766554433221100");
        let n = UIntEx::from_le_slice(&bytes);
        assert_eq!(
            n.limbs(),
            &[Limb(0x8899aabbccddeeff), Limb(0x0011223344556677)]
        );
    }

    #[test]
    #[cfg(target_pointer_width = "32")]
    fn from_be_hex() {
        let n = UIntEx::from_be_hex("0011223344556677");
        assert_eq!(n.limbs(), &[Limb(0x44556677), Limb(0x00112233)]);
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn from_be_hex() {
        let n = UIntEx::from_be_hex("00112233445566778899aabbccddeeff");
        assert_eq!(
            n.limbs(),
            &[Limb(0x8899aabbccddeeff), Limb(0x0011223344556677)]
        );
    }

    #[test]
    #[cfg(target_pointer_width = "32")]
    fn from_le_hex() {
        let n = UIntEx::from_le_hex("7766554433221100");
        assert_eq!(n.limbs(), &[Limb(0x44556677), Limb(0x00112233)]);
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn from_le_hex() {
        let n = UIntEx::from_le_hex("ffeeddccbbaa99887766554433221100");
        assert_eq!(
            n.limbs(),
            &[Limb(0x8899aabbccddeeff), Limb(0x0011223344556677)]
        );
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn hex_upper() {
        let hex = "AAAAAAAABBBBBBBBCCCCCCCCDDDDDDDD";
        let n = U128::from_be_hex(hex);
        assert_eq!(hex, format!("{:X}", n));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn hex_lower() {
        let hex = "aaaaaaaabbbbbbbbccccccccdddddddd";
        let n = U128::from_be_hex(hex);
        assert_eq!(hex, format!("{:x}", n));
    }
}
