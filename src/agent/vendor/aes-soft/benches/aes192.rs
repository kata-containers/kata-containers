#![feature(test)]
extern crate test;

use aes_soft::cipher::{BlockCipher, NewBlockCipher};
use aes_soft::Aes192;

#[bench]
pub fn aes192_new(bh: &mut test::Bencher) {
    bh.iter(|| {
        let cipher = Aes192::new(&Default::default());
        test::black_box(&cipher);
    });
}

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
