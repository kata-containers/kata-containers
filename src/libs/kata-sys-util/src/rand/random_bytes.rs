// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fmt;

use rand::RngCore;

pub struct RandomBytes {
    pub bytes: Vec<u8>,
}

impl RandomBytes {
    pub fn new(n: usize) -> Self {
        let mut bytes = vec![0u8; n];
        rand::thread_rng().fill_bytes(&mut bytes);
        Self { bytes }
    }
}

impl fmt::LowerHex for RandomBytes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for byte in &self.bytes {
            write!(f, "{:x}", byte)?;
        }
        Ok(())
    }
}

impl fmt::UpperHex for RandomBytes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for byte in &self.bytes {
            write!(f, "{:X}", byte)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_bytes() {
        let b = RandomBytes::new(16);
        assert_eq!(b.bytes.len(), 16);

        // check lower hex
        let lower_hex = format!("{:x}", b);
        assert_eq!(lower_hex, lower_hex.to_lowercase());

        // check upper hex
        let upper_hex = format!("{:X}", b);
        assert_eq!(upper_hex, upper_hex.to_uppercase());

        // check new random bytes
        let b1 = RandomBytes::new(16);
        assert_ne!(b.bytes, b1.bytes);
    }
}
