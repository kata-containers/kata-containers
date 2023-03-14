//! Triple DES (3DES) block cipher.

use crate::des::{gen_keys, Des};
use byteorder::{ByteOrder, BE};
use cipher::{
    consts::{U1, U16, U24, U8},
    generic_array::GenericArray,
    BlockCipher, NewBlockCipher,
};

/// Triple DES (3DES) block cipher.
#[derive(Copy, Clone)]
pub struct TdesEde3 {
    d1: Des,
    d2: Des,
    d3: Des,
}

/// Triple DES (3DES) block cipher.
#[derive(Copy, Clone)]
pub struct TdesEee3 {
    d1: Des,
    d2: Des,
    d3: Des,
}

/// Triple DES (3DES) block cipher.
#[derive(Copy, Clone)]
pub struct TdesEde2 {
    d1: Des,
    d2: Des,
}

/// Triple DES (3DES) block cipher.
#[derive(Copy, Clone)]
pub struct TdesEee2 {
    d1: Des,
    d2: Des,
}

impl NewBlockCipher for TdesEde3 {
    type KeySize = U24;

    fn new(key: &GenericArray<u8, U24>) -> Self {
        let d1 = Des {
            keys: gen_keys(BE::read_u64(&key[0..8])),
        };
        let d2 = Des {
            keys: gen_keys(BE::read_u64(&key[8..16])),
        };
        let d3 = Des {
            keys: gen_keys(BE::read_u64(&key[16..24])),
        };
        Self { d1, d2, d3 }
    }
}

impl BlockCipher for TdesEde3 {
    type BlockSize = U8;
    type ParBlocks = U1;

    fn encrypt_block(&self, block: &mut GenericArray<u8, U8>) {
        let mut data = BE::read_u64(block);

        data = self.d1.encrypt(data);
        data = self.d2.decrypt(data);
        data = self.d3.encrypt(data);

        BE::write_u64(block, data);
    }

    fn decrypt_block(&self, block: &mut GenericArray<u8, U8>) {
        let mut data = BE::read_u64(block);

        data = self.d3.decrypt(data);
        data = self.d2.encrypt(data);
        data = self.d1.decrypt(data);

        BE::write_u64(block, data);
    }
}

impl NewBlockCipher for TdesEee3 {
    type KeySize = U24;

    fn new(key: &GenericArray<u8, U24>) -> Self {
        let d1 = Des {
            keys: gen_keys(BE::read_u64(&key[0..8])),
        };
        let d2 = Des {
            keys: gen_keys(BE::read_u64(&key[8..16])),
        };
        let d3 = Des {
            keys: gen_keys(BE::read_u64(&key[16..24])),
        };
        Self { d1, d2, d3 }
    }
}

impl BlockCipher for TdesEee3 {
    type BlockSize = U8;
    type ParBlocks = U1;

    fn encrypt_block(&self, block: &mut GenericArray<u8, U8>) {
        let mut data = BE::read_u64(block);

        data = self.d1.encrypt(data);
        data = self.d2.encrypt(data);
        data = self.d3.encrypt(data);

        BE::write_u64(block, data);
    }

    fn decrypt_block(&self, block: &mut GenericArray<u8, U8>) {
        let mut data = BE::read_u64(block);

        data = self.d3.decrypt(data);
        data = self.d2.decrypt(data);
        data = self.d1.decrypt(data);

        BE::write_u64(block, data);
    }
}

impl NewBlockCipher for TdesEde2 {
    type KeySize = U16;

    fn new(key: &GenericArray<u8, U16>) -> Self {
        let d1 = Des {
            keys: gen_keys(BE::read_u64(&key[0..8])),
        };
        let d2 = Des {
            keys: gen_keys(BE::read_u64(&key[8..16])),
        };
        Self { d1, d2 }
    }
}

impl BlockCipher for TdesEde2 {
    type BlockSize = U8;
    type ParBlocks = U1;

    fn encrypt_block(&self, block: &mut GenericArray<u8, U8>) {
        let mut data = BE::read_u64(block);

        data = self.d1.encrypt(data);
        data = self.d2.decrypt(data);
        data = self.d1.encrypt(data);

        BE::write_u64(block, data);
    }

    fn decrypt_block(&self, block: &mut GenericArray<u8, U8>) {
        let mut data = BE::read_u64(block);

        data = self.d1.decrypt(data);
        data = self.d2.encrypt(data);
        data = self.d1.decrypt(data);

        BE::write_u64(block, data);
    }
}

impl NewBlockCipher for TdesEee2 {
    type KeySize = U16;

    fn new(key: &GenericArray<u8, U16>) -> Self {
        let d1 = Des {
            keys: gen_keys(BE::read_u64(&key[0..8])),
        };
        let d2 = Des {
            keys: gen_keys(BE::read_u64(&key[8..16])),
        };
        Self { d1, d2 }
    }
}

impl BlockCipher for TdesEee2 {
    type BlockSize = U8;
    type ParBlocks = U1;

    fn encrypt_block(&self, block: &mut GenericArray<u8, U8>) {
        let mut data = BE::read_u64(block);

        data = self.d1.encrypt(data);
        data = self.d2.encrypt(data);
        data = self.d1.encrypt(data);

        BE::write_u64(block, data);
    }

    fn decrypt_block(&self, block: &mut GenericArray<u8, U8>) {
        let mut data = BE::read_u64(block);

        data = self.d1.decrypt(data);
        data = self.d2.decrypt(data);
        data = self.d1.decrypt(data);

        BE::write_u64(block, data);
    }
}

opaque_debug::implement!(TdesEde3);
opaque_debug::implement!(TdesEee3);
opaque_debug::implement!(TdesEde2);
opaque_debug::implement!(TdesEee2);
