//! Utility functions that don't fit anywhere else.
use std::convert::TryFrom;

pub fn read_be_u64(b: &[u8]) -> u64 {
    let array = <[u8; 8]>::try_from(b).unwrap();
    u64::from_be_bytes(array)
}

pub fn write_be_u64(b: &mut [u8], n: u64) {
    b.copy_from_slice(&n.to_be_bytes());
}

#[cfg(test)]
mod test {
    use super::*;

    quickcheck! {
        fn be_u64_roundtrip(n: u64) -> bool {
            let mut b = [0; 8];
            write_be_u64(&mut b, n);
            n == read_be_u64(&b)
        }
    }
}
