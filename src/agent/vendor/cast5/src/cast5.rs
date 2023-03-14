use cipher::{
    consts::{U1, U16, U8},
    generic_array::GenericArray,
};

use byteorder::{BigEndian, ByteOrder};
use cipher::block::{BlockCipher, InvalidKeyLength, NewBlockCipher};

use crate::{
    consts::{S1, S2, S3, S4},
    schedule::key_schedule,
};

type Block = GenericArray<u8, U8>;

/// The CAST5 block cipher.
#[derive(Clone, Copy)]
pub struct Cast5 {
    masking: [u32; 16],
    rotate: [u8; 16],
    /// If this is set to true, it means a small key is used and only 12 rounds instead of 16
    /// rounds are used in the algorithm.
    small_key: bool,
}

impl Cast5 {
    fn init_state(key_len: usize) -> Cast5 {
        let small_key = key_len <= 10;

        Cast5 {
            masking: [0u32; 16],
            rotate: [0u8; 16],
            small_key,
        }
    }

    /// Implements the key schedule according to RFC 2144 2.4.
    /// https://tools.ietf.org/html/rfc2144#section-2.4
    fn key_schedule(&mut self, key: &[u8]) {
        let mut x = [0; 4];
        BigEndian::read_u32_into(&key, &mut x);

        let mut z = [0u32; 4];
        let mut k = [0u32; 16];

        key_schedule(&mut x, &mut z, &mut k);
        self.masking[..].clone_from_slice(&k[..]);

        key_schedule(&mut x, &mut z, &mut k);

        for (i, ki) in k.iter().enumerate() {
            self.rotate[i] = (ki & 0x1f) as u8;
        }
    }
}

macro_rules! f1 {
    ($D:expr, $m:expr, $r:expr) => {{
        let i = ($m.wrapping_add($D)).rotate_left(u32::from($r));
        (S1[(i >> 24) as usize] ^ S2[((i >> 16) & 0xff) as usize])
            .wrapping_sub(S3[((i >> 8) & 0xff) as usize])
            .wrapping_add(S4[(i & 0xff) as usize])
    }};
}

macro_rules! f2 {
    ($D:expr, $m:expr, $r:expr) => {{
        let i = ($m ^ $D).rotate_left(u32::from($r));
        S1[(i >> 24) as usize]
            .wrapping_sub(S2[((i >> 16) & 0xff) as usize])
            .wrapping_add(S3[((i >> 8) & 0xff) as usize])
            ^ S4[(i & 0xff) as usize]
    }};
}

macro_rules! f3 {
    ($D:expr, $m:expr, $r:expr) => {{
        let i = ($m.wrapping_sub($D)).rotate_left(u32::from($r));
        (S1[(i >> 24) as usize].wrapping_add(S2[((i >> 16) & 0xff) as usize])
            ^ S3[((i >> 8) & 0xff) as usize])
            .wrapping_sub(S4[(i & 0xff) as usize])
    }};
}

impl NewBlockCipher for Cast5 {
    type KeySize = U16;

    fn new(key: &GenericArray<u8, U16>) -> Self {
        Self::new_varkey(&key).unwrap()
    }

    fn new_varkey(key: &[u8]) -> Result<Self, InvalidKeyLength> {
        // Available key sizes are 40...128 bits.
        if key.len() < 5 || key.len() > 16 {
            return Err(InvalidKeyLength);
        }
        let mut cast5 = Cast5::init_state(key.len());

        if key.len() < 16 {
            // Pad keys that are less than 128 bits long.
            let mut padded_key = [0u8; 16];
            padded_key[..key.len()].copy_from_slice(key);
            cast5.key_schedule(&padded_key[..]);
        } else {
            cast5.key_schedule(key);
        }
        Ok(cast5)
    }
}

impl BlockCipher for Cast5 {
    type BlockSize = U8;
    type ParBlocks = U1;

    #[inline]
    fn encrypt_block(&self, block: &mut Block) {
        let masking = self.masking;
        let rotate = self.rotate;

        // (L0,R0) <-- (m1...m64). (Split the plaintext into left and
        // right 32-bit halves L0 = m1...m32 and R0 = m33...m64.)
        let l = BigEndian::read_u32(&block[0..4]);
        let r = BigEndian::read_u32(&block[4..8]);
        // (16 rounds) for i from 1 to 16, compute Li and Ri as follows:
        //   Li = Ri-1;
        //   Ri = Li-1 ^ f(Ri-1,Kmi,Kri), where f is defined in Section 2.2
        // (f is of Type 1, Type 2, or Type 3, depending on i).
        //
        // Rounds 1, 4, 7, 10, 13, and 16 use f function Type 1.
        // Rounds 2, 5, 8, 11, and 14 use f function Type 2.
        // Rounds 3, 6, 9, 12, and 15 use f function Type 3.

        let (l, r) = (r, l ^ f1!(r, masking[0], rotate[0]));
        let (l, r) = (r, l ^ f2!(r, masking[1], rotate[1]));
        let (l, r) = (r, l ^ f3!(r, masking[2], rotate[2]));
        let (l, r) = (r, l ^ f1!(r, masking[3], rotate[3]));
        let (l, r) = (r, l ^ f2!(r, masking[4], rotate[4]));
        let (l, r) = (r, l ^ f3!(r, masking[5], rotate[5]));
        let (l, r) = (r, l ^ f1!(r, masking[6], rotate[6]));
        let (l, r) = (r, l ^ f2!(r, masking[7], rotate[7]));
        let (l, r) = (r, l ^ f3!(r, masking[8], rotate[8]));
        let (l, r) = (r, l ^ f1!(r, masking[9], rotate[9]));
        let (l, r) = (r, l ^ f2!(r, masking[10], rotate[10]));
        let (l, r) = (r, l ^ f3!(r, masking[11], rotate[11]));

        let (l, r) = if self.small_key {
            (l, r)
        } else {
            // Rounds 13..16 are only executed for keys > 80 bits.
            let (l, r) = (r, l ^ f1!(r, masking[12], rotate[12]));
            let (l, r) = (r, l ^ f2!(r, masking[13], rotate[13]));
            let (l, r) = (r, l ^ f3!(r, masking[14], rotate[14]));
            (r, l ^ f1!(r, masking[15], rotate[15]))
        };

        // c1...c64 <-- (R16,L16).  (Exchange final blocks L16, R16 and
        // concatenate to form the ciphertext.)
        BigEndian::write_u32(&mut block[0..4], r);
        BigEndian::write_u32(&mut block[4..8], l);
    }

    #[inline]
    fn decrypt_block(&self, block: &mut Block) {
        let masking = self.masking;
        let rotate = self.rotate;

        let l = BigEndian::read_u32(&block[0..4]);
        let r = BigEndian::read_u32(&block[4..8]);

        let (l, r) = if self.small_key {
            (l, r)
        } else {
            let (l, r) = (r, l ^ f1!(r, masking[15], rotate[15]));
            let (l, r) = (r, l ^ f3!(r, masking[14], rotate[14]));
            let (l, r) = (r, l ^ f2!(r, masking[13], rotate[13]));
            (r, l ^ f1!(r, masking[12], rotate[12]))
        };

        let (l, r) = (r, l ^ f3!(r, masking[11], rotate[11]));
        let (l, r) = (r, l ^ f2!(r, masking[10], rotate[10]));
        let (l, r) = (r, l ^ f1!(r, masking[9], rotate[9]));
        let (l, r) = (r, l ^ f3!(r, masking[8], rotate[8]));
        let (l, r) = (r, l ^ f2!(r, masking[7], rotate[7]));
        let (l, r) = (r, l ^ f1!(r, masking[6], rotate[6]));
        let (l, r) = (r, l ^ f3!(r, masking[5], rotate[5]));
        let (l, r) = (r, l ^ f2!(r, masking[4], rotate[4]));
        let (l, r) = (r, l ^ f1!(r, masking[3], rotate[3]));
        let (l, r) = (r, l ^ f3!(r, masking[2], rotate[2]));
        let (l, r) = (r, l ^ f2!(r, masking[1], rotate[1]));
        let (l, r) = (r, l ^ f1!(r, masking[0], rotate[0]));

        BigEndian::write_u32(&mut block[0..4], r);
        BigEndian::write_u32(&mut block[4..8], l);
    }
}

opaque_debug::implement!(Cast5);
