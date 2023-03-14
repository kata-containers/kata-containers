#![feature(test)]

extern crate test;

use aes::{Aes192, BlockCipher, NewBlockCipher};

#[bench]
pub fn aes192_encrypt(bh: &mut test::Bencher) {
    let cipher = Aes192::new(&Default::default());
    let mut input = Default::default();

    bh.iter(|| {
        cipher.encrypt_block(&mut input);
        test::black_box(&input);
    });
    bh.bytes = input.len() as u64;
}

#[bench]
pub fn aes192_decrypt(bh: &mut test::Bencher) {
    let cipher = Aes192::new(&Default::default());
    let mut input = Default::default();

    bh.iter(|| {
        cipher.decrypt_block(&mut input);
        test::black_box(&input);
    });
    bh.bytes = input.len() as u64;
}

#[bench]
pub fn aes192_encrypt8(bh: &mut test::Bencher) {
    let cipher = Aes192::new(&Default::default());
    let mut input = Default::default();

    bh.iter(|| {
        cipher.encrypt_blocks(&mut input);
        test::black_box(&input);
    });
    bh.bytes = (input[0].len() * input.len()) as u64;
}

#[bench]
pub fn aes192_decrypt8(bh: &mut test::Bencher) {
    let cipher = Aes192::new(&Default::default());
    let mut input = Default::default();

    bh.iter(|| {
        cipher.decrypt_blocks(&mut input);
        test::black_box(&input);
    });
    bh.bytes = (input[0].len() * input.len()) as u64;
}

/*
#[bench]
pub fn ctr_aes192(bh: &mut test::Bencher) {
    let mut cipher = aes::CtrAes192::new(&[0; 24], &[0; 16]);
    let mut input = [0u8; 10000];


    bh.iter(|| {
        cipher.xor(&mut input);
        test::black_box(&input);
    });
    bh.bytes = input.len() as u64;
}
*/
