#![feature(test)]
extern crate test;

use aes_soft::cipher::{BlockCipher, NewBlockCipher};
use aes_soft::Aes128;

#[bench]
pub fn aes128_new(bh: &mut test::Bencher) {
    bh.iter(|| {
        let cipher = Aes128::new(&Default::default());
        test::black_box(&cipher);
    });
}

#[bench]
pub fn aes128_encrypt(bh: &mut test::Bencher) {
    let cipher = Aes128::new(&Default::default());
    let mut input = Default::default();

    bh.iter(|| {
        cipher.encrypt_block(&mut input);
        test::black_box(&input);
    });
    bh.bytes = input.len() as u64;
}

#[bench]
pub fn aes128_decrypt(bh: &mut test::Bencher) {
    let cipher = Aes128::new(&Default::default());
    let mut input = Default::default();

    bh.iter(|| {
        cipher.decrypt_block(&mut input);
        test::black_box(&input);
    });
    bh.bytes = input.len() as u64;
}

#[bench]
pub fn aes128_encrypt8(bh: &mut test::Bencher) {
    let cipher = Aes128::new(&Default::default());
    let mut input = Default::default();

    bh.iter(|| {
        cipher.encrypt_blocks(&mut input);
        test::black_box(&input);
    });
    bh.bytes = (input[0].len() * input.len()) as u64;
}

#[bench]
pub fn aes128_decrypt8(bh: &mut test::Bencher) {
    let cipher = Aes128::new(&Default::default());
    let mut input = Default::default();

    bh.iter(|| {
        cipher.decrypt_blocks(&mut input);
        test::black_box(&input);
    });
    bh.bytes = (input[0].len() * input.len()) as u64;
}
