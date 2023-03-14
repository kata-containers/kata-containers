//! Twofish block cipher

#![no_std]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg"
)]
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]
#![allow(clippy::needless_range_loop, clippy::unreadable_literal)]

pub use cipher;

use byteorder::{ByteOrder, LE};
use cipher::{
    block::{BlockCipher, InvalidKeyLength, NewBlockCipher},
    consts::{U1, U16, U32},
    generic_array::GenericArray,
};

mod consts;
use crate::consts::{MDS_POLY, QBOX, QORD, RS, RS_POLY};

type Block = GenericArray<u8, U16>;

/// Twofish block cipher
pub struct Twofish {
    s: [u8; 16],  // S-box key
    k: [u32; 40], // Subkeys
    start: usize,
}

fn gf_mult(mut a: u8, mut b: u8, p: u8) -> u8 {
    let mut result = 0;
    while a > 0 {
        if a & 1 == 1 {
            result ^= b;
        }
        a >>= 1;
        if b & 0x80 == 0x80 {
            b = (b << 1) ^ p;
        } else {
            b <<= 1;
        }
    }
    result
}

// q_i sbox
fn sbox(i: usize, x: u8) -> u8 {
    let (a0, b0) = (x >> 4 & 15, x & 15);
    let a1 = a0 ^ b0;
    let b1 = (a0 ^ ((b0 << 3) | (b0 >> 1)) ^ (a0 << 3)) & 15;
    let (a2, b2) = (QBOX[i][0][a1 as usize], QBOX[i][1][b1 as usize]);
    let a3 = a2 ^ b2;
    let b3 = (a2 ^ ((b2 << 3) | (b2 >> 1)) ^ (a2 << 3)) & 15;
    let (a4, b4) = (QBOX[i][2][a3 as usize], QBOX[i][3][b3 as usize]);
    (b4 << 4) + a4
}

fn mds_column_mult(x: u8, column: usize) -> u32 {
    let x5b = gf_mult(x, 0x5b, MDS_POLY);
    let xef = gf_mult(x, 0xef, MDS_POLY);

    let v = match column {
        0 => [x, x5b, xef, xef],
        1 => [xef, xef, x5b, x],
        2 => [x5b, xef, x, xef],
        3 => [x5b, x, xef, x5b],
        _ => unreachable!(),
    };
    LE::read_u32(&v)
}

fn mds_mult(y: [u8; 4]) -> u32 {
    let mut z = 0;
    for i in 0..4 {
        z ^= mds_column_mult(y[i], i);
    }
    z
}

fn rs_mult(m: &[u8], out: &mut [u8]) {
    for i in 0..4 {
        out[i] = 0;
        for j in 0..8 {
            out[i] ^= gf_mult(m[j], RS[i][j], RS_POLY);
        }
    }
}

#[allow(clippy::many_single_char_names)]
fn h(x: u32, m: &[u8], k: usize, offset: usize) -> u32 {
    let mut y = [0u8; 4];
    LE::write_u32(&mut y, x);

    if k == 4 {
        y[0] = sbox(1, y[0]) ^ m[4 * (6 + offset)];
        y[1] = sbox(0, y[1]) ^ m[4 * (6 + offset) + 1];
        y[2] = sbox(0, y[2]) ^ m[4 * (6 + offset) + 2];
        y[3] = sbox(1, y[3]) ^ m[4 * (6 + offset) + 3];
    }

    if k >= 3 {
        y[0] = sbox(1, y[0]) ^ m[4 * (4 + offset)];
        y[1] = sbox(1, y[1]) ^ m[4 * (4 + offset) + 1];
        y[2] = sbox(0, y[2]) ^ m[4 * (4 + offset) + 2];
        y[3] = sbox(0, y[3]) ^ m[4 * (4 + offset) + 3];
    }

    let a = 4 * (2 + offset);
    let b = 4 * offset;
    y[0] = sbox(1, sbox(0, sbox(0, y[0]) ^ m[a]) ^ m[b]);
    y[1] = sbox(0, sbox(0, sbox(1, y[1]) ^ m[a + 1]) ^ m[b + 1]);
    y[2] = sbox(1, sbox(1, sbox(0, y[2]) ^ m[a + 2]) ^ m[b + 2]);
    y[3] = sbox(0, sbox(1, sbox(1, y[3]) ^ m[a + 3]) ^ m[b + 3]);

    mds_mult(y)
}

impl Twofish {
    fn g_func(&self, x: u32) -> u32 {
        let mut result: u32 = 0;
        for y in 0..4 {
            let mut g = sbox(QORD[y][self.start], (x >> (8 * y)) as u8);

            for z in self.start + 1..5 {
                g ^= self.s[4 * (z - self.start - 1) + y];
                g = sbox(QORD[y][z], g);
            }

            result ^= mds_column_mult(g, y);
        }
        result
    }

    fn key_schedule(&mut self, key: &[u8]) {
        let k = key.len() / 8;

        let rho: u32 = 0x1010101;

        for x in 0..20 {
            let a = h(rho * (2 * x), key, k, 0);
            let b = h(rho * (2 * x + 1), key, k, 1).rotate_left(8);
            let v = a.wrapping_add(b);
            self.k[(2 * x) as usize] = v;
            self.k[(2 * x + 1) as usize] = (v.wrapping_add(b)).rotate_left(9);
        }
        self.start = match k {
            4 => 0,
            3 => 1,
            2 => 2,
            _ => unreachable!(),
        };

        // Compute S_i.
        for i in 0..k {
            rs_mult(&key[i * 8..i * 8 + 8], &mut self.s[i * 4..(i + 1) * 4]);
        }
    }
}

impl NewBlockCipher for Twofish {
    type KeySize = U32;

    fn new(key: &GenericArray<u8, U32>) -> Self {
        Self::new_varkey(key).unwrap()
    }

    fn new_varkey(key: &[u8]) -> Result<Self, InvalidKeyLength> {
        let n = key.len();
        if n != 16 && n != 24 && n != 32 {
            return Err(InvalidKeyLength);
        }
        let mut twofish = Self {
            s: [0u8; 16],
            k: [0u32; 40],
            start: 0,
        };
        twofish.key_schedule(key);
        Ok(twofish)
    }
}

impl BlockCipher for Twofish {
    type BlockSize = U16;
    type ParBlocks = U1;

    fn encrypt_block(&self, block: &mut Block) {
        let mut p = [0u32; 4];
        LE::read_u32_into(block, &mut p);

        // Input whitening
        for i in 0..4 {
            p[i] ^= self.k[i];
        }

        for r in 0..8 {
            let k = 4 * r + 8;

            let t1 = self.g_func(p[1].rotate_left(8));
            let t0 = self.g_func(p[0]).wrapping_add(t1);
            p[2] = (p[2] ^ (t0.wrapping_add(self.k[k]))).rotate_right(1);
            let t2 = t1.wrapping_add(t0).wrapping_add(self.k[k + 1]);
            p[3] = p[3].rotate_left(1) ^ t2;

            let t1 = self.g_func(p[3].rotate_left(8));
            let t0 = self.g_func(p[2]).wrapping_add(t1);
            p[0] = (p[0] ^ (t0.wrapping_add(self.k[k + 2]))).rotate_right(1);
            let t2 = t1.wrapping_add(t0).wrapping_add(self.k[k + 3]);
            p[1] = (p[1].rotate_left(1)) ^ t2;
        }

        // Undo last swap and output whitening
        LE::write_u32(&mut block[0..4], p[2] ^ self.k[4]);
        LE::write_u32(&mut block[4..8], p[3] ^ self.k[5]);
        LE::write_u32(&mut block[8..12], p[0] ^ self.k[6]);
        LE::write_u32(&mut block[12..16], p[1] ^ self.k[7]);
    }

    fn decrypt_block(&self, block: &mut Block) {
        let mut c = [0u32; 4];

        c[0] = LE::read_u32(&block[8..12]) ^ self.k[6];
        c[1] = LE::read_u32(&block[12..16]) ^ self.k[7];
        c[2] = LE::read_u32(&block[0..4]) ^ self.k[4];
        c[3] = LE::read_u32(&block[4..8]) ^ self.k[5];

        for r in (0..8).rev() {
            let k = 4 * r + 8;

            let t1 = self.g_func(c[3].rotate_left(8));
            let t0 = self.g_func(c[2]).wrapping_add(t1);
            c[0] = c[0].rotate_left(1) ^ (t0.wrapping_add(self.k[k + 2]));
            let t2 = t1.wrapping_add(t0).wrapping_add(self.k[k + 3]);
            c[1] = (c[1] ^ t2).rotate_right(1);

            let t1 = self.g_func(c[1].rotate_left(8));
            let t0 = self.g_func(c[0]).wrapping_add(t1);
            c[2] = c[2].rotate_left(1) ^ (t0.wrapping_add(self.k[k]));
            let t2 = t1.wrapping_add(t0).wrapping_add(self.k[k + 1]);
            c[3] = (c[3] ^ t2).rotate_right(1);
        }

        for i in 0..4 {
            c[i] ^= self.k[i];
        }

        LE::write_u32_into(&c[..], block);
    }
}

opaque_debug::implement!(Twofish);

#[cfg(test)]
mod tests;
