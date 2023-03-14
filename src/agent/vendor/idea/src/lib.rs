//! An implementation of the [IDEA][1] block cipher.
//!
//! [1]: https://en.wikipedia.org/wiki/International_Data_Encryption_Algorithm

#![no_std]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg"
)]
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]
#![allow(clippy::many_single_char_names)]

pub use cipher::{self, BlockCipher, NewBlockCipher};

use cipher::{
    consts::{U1, U16, U8},
    generic_array::GenericArray,
};

mod consts;
use crate::consts::{FUYI, LENGTH_SUB_KEYS, MAXIM, ONE, ROUNDS};

/// The International Data Encryption Algorithm (IDEA) block cipher.
#[derive(Copy, Clone)]
pub struct Idea {
    encryption_sub_keys: [u16; LENGTH_SUB_KEYS],
    decryption_sub_keys: [u16; LENGTH_SUB_KEYS],
}

impl Idea {
    fn expand_key(&mut self, key: &GenericArray<u8, U16>) {
        let length_key = key.len();
        for i in 0..(length_key / 2) {
            self.encryption_sub_keys[i] = (u16::from(key[2 * i]) << 8) + u16::from(key[2 * i + 1]);
        }

        let mut a: u16;
        let mut b: u16;
        for i in (length_key / 2)..LENGTH_SUB_KEYS {
            if (i + 1) % 8 == 0 {
                a = self.encryption_sub_keys[i - 15];
            } else {
                a = self.encryption_sub_keys[i - 7];
            }

            if (i + 2) % 8 < 2 {
                b = self.encryption_sub_keys[i - 14];
            } else {
                b = self.encryption_sub_keys[i - 6];
            }

            self.encryption_sub_keys[i] = (a << 9) + (b >> 7);
        }
    }

    fn invert_sub_keys(&mut self) {
        let mut k = ROUNDS * 6;
        for i in 0..ROUNDS + 1 {
            let j = i * 6;
            let l = k - j;

            let (m, n) = if i > 0 && i < 8 { (2, 1) } else { (1, 2) };

            self.decryption_sub_keys[j] = self.mul_inv(self.encryption_sub_keys[l]);
            self.decryption_sub_keys[j + 1] = self.add_inv(self.encryption_sub_keys[l + m]);
            self.decryption_sub_keys[j + 2] = self.add_inv(self.encryption_sub_keys[l + n]);
            self.decryption_sub_keys[j + 3] = self.mul_inv(self.encryption_sub_keys[l + 3]);
        }

        k = (ROUNDS - 1) * 6;
        for i in 0..ROUNDS {
            let j = i * 6;
            let l = k - j;
            self.decryption_sub_keys[j + 4] = self.encryption_sub_keys[l + 4];
            self.decryption_sub_keys[j + 5] = self.encryption_sub_keys[l + 5];
        }
    }

    fn crypt(&self, block: &mut GenericArray<u8, U8>, sub_keys: &[u16; LENGTH_SUB_KEYS]) {
        let mut x1 = (u16::from(block[0]) << 8) + (u16::from(block[1]));
        let mut x2 = (u16::from(block[2]) << 8) + (u16::from(block[3]));
        let mut x3 = (u16::from(block[4]) << 8) + (u16::from(block[5]));
        let mut x4 = (u16::from(block[6]) << 8) + (u16::from(block[7]));

        for i in 0..ROUNDS {
            let j = i * 6;
            let y1 = self.mul(x1, sub_keys[j]);
            let y2 = self.add(x2, sub_keys[j + 1]);
            let y3 = self.add(x3, sub_keys[j + 2]);
            let y4 = self.mul(x4, sub_keys[j + 3]);

            let t0 = self.mul(y1 ^ y3, sub_keys[j + 4]);
            let _t = self.add(y2 ^ y4, t0);
            let t1 = self.mul(_t, sub_keys[j + 5]);
            let t2 = self.add(t0, t1);

            x1 = y1 ^ t1;
            x2 = y3 ^ t1;
            x3 = y2 ^ t2;
            x4 = y4 ^ t2;
        }

        let y1 = self.mul(x1, sub_keys[48]);
        let y2 = self.add(x3, sub_keys[49]);
        let y3 = self.add(x2, sub_keys[50]);
        let y4 = self.mul(x4, sub_keys[51]);

        block[0] = (y1 >> 8) as u8;
        block[1] = y1 as u8;
        block[2] = (y2 >> 8) as u8;
        block[3] = y2 as u8;
        block[4] = (y3 >> 8) as u8;
        block[5] = y3 as u8;
        block[6] = (y4 >> 8) as u8;
        block[7] = y4 as u8;
    }

    fn mul(&self, a: u16, b: u16) -> u16 {
        let x = u32::from(a);
        let y = u32::from(b);
        let mut r: i32;

        if x == 0 {
            r = (MAXIM - y) as i32;
        } else if y == 0 {
            r = (MAXIM - x) as i32;
        } else {
            let c: u32 = x * y;
            r = ((c & ONE) as i32) - ((c >> 16) as i32);
            if r < 0 {
                r += MAXIM as i32;
            }
        }

        (r & (ONE as i32)) as u16
    }

    fn add(&self, a: u16, b: u16) -> u16 {
        ((u32::from(a) + u32::from(b)) & ONE) as u16
    }

    fn mul_inv(&self, a: u16) -> u16 {
        if a <= 1 {
            a
        } else {
            let mut x = u32::from(a);
            let mut y = MAXIM;
            let mut t0 = 1u32;
            let mut t1 = 0u32;
            loop {
                t1 += y / x * t0;
                y %= x;
                if y == 1 {
                    return (MAXIM - t1) as u16;
                }
                t0 += x / y * t1;
                x %= y;
                if x == 1 {
                    return t0 as u16;
                }
            }
        }
    }

    fn add_inv(&self, a: u16) -> u16 {
        ((FUYI - (u32::from(a))) & ONE) as u16
    }
}

impl NewBlockCipher for Idea {
    type KeySize = U16;

    fn new(key: &GenericArray<u8, U16>) -> Self {
        let mut cipher = Self {
            encryption_sub_keys: [0u16; 52],
            decryption_sub_keys: [0u16; 52],
        };
        cipher.expand_key(key);
        cipher.invert_sub_keys();
        cipher
    }
}

impl BlockCipher for Idea {
    type BlockSize = U8;
    type ParBlocks = U1;

    fn encrypt_block(&self, block: &mut GenericArray<u8, U8>) {
        self.crypt(block, &self.encryption_sub_keys);
    }

    fn decrypt_block(&self, block: &mut GenericArray<u8, U8>) {
        self.crypt(block, &self.decryption_sub_keys);
    }
}

opaque_debug::implement!(Idea);

#[cfg(test)]
mod tests;
