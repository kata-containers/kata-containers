//! Data Encryption Standard (DES) block cipher.

#![allow(clippy::unreadable_literal)]

use byteorder::{ByteOrder, BE};
use cipher::{
    consts::{U1, U8},
    generic_array::GenericArray,
    BlockCipher, NewBlockCipher,
};

use crate::consts::{SBOXES, SHIFTS};

/// Data Encryption Standard (DES) block cipher.
#[derive(Copy, Clone)]
pub struct Des {
    pub(crate) keys: [u64; 16],
}

/// Swap bits in `a` using a delta swap
fn delta_swap(a: u64, delta: u64, mask: u64) -> u64 {
    let b = (a ^ (a >> delta)) & mask;
    a ^ b ^ (b << delta)
}

/// Swap bits using the PC-1 table
fn pc1(mut key: u64) -> u64 {
    key = delta_swap(key, 2, 0x3333000033330000);
    key = delta_swap(key, 4, 0x0f0f0f0f00000000);
    key = delta_swap(key, 8, 0x009a000a00a200a8);
    key = delta_swap(key, 16, 0x00006c6c0000cccc);
    key = delta_swap(key, 1, 0x1045500500550550);
    key = delta_swap(key, 32, 0x00000000f0f0f5fa);
    key = delta_swap(key, 8, 0x00550055006a00aa);
    key = delta_swap(key, 2, 0x0000333330000300);
    key & 0xFFFFFFFFFFFFFF00
}

/// Swap bits using the PC-2 table
fn pc2(key: u64) -> u64 {
    let key = key.rotate_left(61);
    let b1 = (key & 0x0021000002000000) >> 7;
    let b2 = (key & 0x0008020010080000) << 1;
    let b3 = key & 0x0002200000000000;
    let b4 = (key & 0x0000000000100020) << 19;
    let b5 = (key.rotate_left(54) & 0x0005312400000011).wrapping_mul(0x0000000094200201)
        & 0xea40100880000000;
    let b6 = (key.rotate_left(7) & 0x0022110000012001).wrapping_mul(0x0001000000610006)
        & 0x1185004400000000;
    let b7 = (key.rotate_left(6) & 0x0000520040200002).wrapping_mul(0x00000080000000c1)
        & 0x0028811000200000;
    let b8 = (key & 0x01000004c0011100).wrapping_mul(0x0000000000004284) & 0x0400082244400000;
    let b9 = (key.rotate_left(60) & 0x0000000000820280).wrapping_mul(0x0000000000089001)
        & 0x0000000110880000;
    let b10 = (key.rotate_left(49) & 0x0000000000024084).wrapping_mul(0x0000000002040005)
        & 0x000000000a030000;
    b1 | b2 | b3 | b4 | b5 | b6 | b7 | b8 | b9 | b10
}

/// Swap bits using the reverse FP table
fn fp(mut message: u64) -> u64 {
    message = delta_swap(message, 24, 0x000000FF000000FF);
    message = delta_swap(message, 24, 0x00000000FF00FF00);
    message = delta_swap(message, 36, 0x000000000F0F0F0F);
    message = delta_swap(message, 18, 0x0000333300003333);
    delta_swap(message, 9, 0x0055005500550055)
}

/// Swap bits using the IP table
fn ip(mut message: u64) -> u64 {
    message = delta_swap(message, 9, 0x0055005500550055);
    message = delta_swap(message, 18, 0x0000333300003333);
    message = delta_swap(message, 36, 0x000000000F0F0F0F);
    message = delta_swap(message, 24, 0x00000000FF00FF00);
    delta_swap(message, 24, 0x000000FF000000FF)
}

/// Swap bits using the E table
fn e(block: u64) -> u64 {
    const BLOCK_LEN: usize = 32;
    const RESULT_LEN: usize = 48;

    let b1 = (block << (BLOCK_LEN - 1)) & 0x8000000000000000;
    let b2 = (block >> 1) & 0x7C00000000000000;
    let b3 = (block >> 3) & 0x03F0000000000000;
    let b4 = (block >> 5) & 0x000FC00000000000;
    let b5 = (block >> 7) & 0x00003F0000000000;
    let b6 = (block >> 9) & 0x000000FC00000000;
    let b7 = (block >> 11) & 0x00000003F0000000;
    let b8 = (block >> 13) & 0x000000000FC00000;
    let b9 = (block >> 15) & 0x00000000003E0000;
    let b10 = (block >> (RESULT_LEN - 1)) & 0x0000000000010000;
    b1 | b2 | b3 | b4 | b5 | b6 | b7 | b8 | b9 | b10
}

/// Swap bits using the P table
fn p(block: u64) -> u64 {
    let block = block.rotate_left(44);
    let b1 = (block & 0x0000000000200000) << 32;
    let b2 = (block & 0x0000000000480000) << 13;
    let b3 = (block & 0x0000088000000000) << 12;
    let b4 = (block & 0x0000002020120000) << 25;
    let b5 = (block & 0x0000000442000000) << 14;
    let b6 = (block & 0x0000000001800000) << 37;
    let b7 = (block & 0x0000000004000000) << 24;
    let b8 = (block & 0x0000020280015000).wrapping_mul(0x0000020080800083) & 0x02000a6400000000;
    let b9 = (block.rotate_left(29) & 0x01001400000000aa).wrapping_mul(0x0000210210008081)
        & 0x0902c01200000000;
    let b10 = (block & 0x0000000910040000).wrapping_mul(0x0000000c04000020) & 0x8410010000000000;
    b1 | b2 | b3 | b4 | b5 | b6 | b7 | b8 | b9 | b10
}

/// Generate the 16 subkeys
pub(crate) fn gen_keys(key: u64) -> [u64; 16] {
    let mut keys: [u64; 16] = [0; 16];
    let key = pc1(key);

    // The most significant bit is bit zero, and there are only 56 bits in
    // the key after applying PC1, so we need to remove the eight least
    // significant bits from the key.
    let key = key >> 8;

    let mut c = key >> 28;
    let mut d = key & 0x0FFFFFFF;
    for i in 0..16 {
        c = rotate(c, SHIFTS[i]);
        d = rotate(d, SHIFTS[i]);

        // We need the `<< 8` because the most significant bit is bit zero,
        // so we need to shift our 56 bit value 8 bits to the left.
        keys[i] = pc2(((c << 28) | d) << 8);
    }

    keys
}

/// Performs a left rotate on a 28 bit number
fn rotate(mut val: u64, shift: u8) -> u64 {
    let top_bits = val >> (28 - shift);
    val <<= shift;

    (val | top_bits) & 0x0FFFFFFF
}

fn round(input: u64, key: u64) -> u64 {
    let l = input & (0xFFFF_FFFF << 32);
    let r = input << 32;

    r | ((f(r, key) ^ l) >> 32)
}

fn f(input: u64, key: u64) -> u64 {
    let mut val = e(input as u64);
    val ^= key;
    val = apply_sboxes(val);
    p(val)
}

/// Applies all eight sboxes to the input
fn apply_sboxes(input: u64) -> u64 {
    let mut output: u64 = 0;

    for (i, sbox) in SBOXES.iter().enumerate() {
        let val = (input >> (58 - (i * 6))) & 0x3F;
        output |= u64::from(sbox[val as usize]) << (60 - (i * 4));
    }

    output
}

impl Des {
    pub(crate) fn encrypt(&self, mut data: u64) -> u64 {
        data = ip(data);
        for key in &self.keys {
            data = round(data, *key);
        }
        fp((data << 32) | (data >> 32))
    }

    pub(crate) fn decrypt(&self, mut data: u64) -> u64 {
        data = ip(data);
        for key in self.keys.iter().rev() {
            data = round(data, *key);
        }
        fp((data << 32) | (data >> 32))
    }
}

impl NewBlockCipher for Des {
    type KeySize = U8;

    fn new(key: &GenericArray<u8, U8>) -> Self {
        Des {
            keys: gen_keys(BE::read_u64(key)),
        }
    }
}

impl BlockCipher for Des {
    type BlockSize = U8;
    type ParBlocks = U1;

    fn encrypt_block(&self, block: &mut GenericArray<u8, U8>) {
        let data = BE::read_u64(block);
        BE::write_u64(block, self.encrypt(data));
    }

    fn decrypt_block(&self, block: &mut GenericArray<u8, U8>) {
        let data = BE::read_u64(block);
        BE::write_u64(block, self.decrypt(data));
    }
}

opaque_debug::implement!(Des);
