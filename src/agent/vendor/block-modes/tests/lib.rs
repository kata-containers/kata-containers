//! Test vectors generated with OpenSSL

use aes::Aes128;
use block_modes::block_padding::ZeroPadding;
use block_modes::BlockMode;
use block_modes::{Cbc, Ecb};

#[test]
fn ecb_aes128() {
    let key = include_bytes!("data/aes128.key.bin");
    let plaintext = include_bytes!("data/aes128.plaintext.bin");
    let ciphertext = include_bytes!("data/ecb-aes128.ciphertext.bin");

    // ECB mode ignores IV
    let iv = Default::default();
    let mode = Ecb::<Aes128, ZeroPadding>::new_var(key, iv).unwrap();
    let mut pt = plaintext.to_vec();
    let n = pt.len();
    mode.encrypt(&mut pt, n).unwrap();
    assert_eq!(pt, &ciphertext[..]);

    let mode = Ecb::<Aes128, ZeroPadding>::new_var(key, iv).unwrap();
    let mut ct = ciphertext.to_vec();
    mode.decrypt(&mut ct).unwrap();
    assert_eq!(ct, &plaintext[..]);
}

#[test]
fn cbc_aes128() {
    let key = include_bytes!("data/aes128.key.bin");
    let iv = include_bytes!("data/aes128.iv.bin");
    let plaintext = include_bytes!("data/aes128.plaintext.bin");
    let ciphertext = include_bytes!("data/cbc-aes128.ciphertext.bin");

    let mode = Cbc::<Aes128, ZeroPadding>::new_var(key, iv).unwrap();
    let mut pt = plaintext.to_vec();
    let n = pt.len();
    mode.encrypt(&mut pt, n).unwrap();
    assert_eq!(pt, &ciphertext[..]);

    let mode = Cbc::<Aes128, ZeroPadding>::new_var(key, iv).unwrap();
    let mut ct = ciphertext.to_vec();
    mode.decrypt(&mut ct).unwrap();
    assert_eq!(ct, &plaintext[..]);
}

/// Test that parallel code works correctly
#[test]
fn par_blocks() {
    use block_modes::block_padding::Pkcs7;
    fn run<M: BlockMode<Aes128, Pkcs7>>() {
        let key: &[u8; 16] = b"secret key data.";
        let iv: &[u8; 16] = b"public iv data..";

        for i in 1..160 {
            let mut buf = [128u8; 160];

            let cipher = M::new_var(key, iv).unwrap();
            let ct_len = cipher.encrypt(&mut buf, i).unwrap().len();
            let cipher = M::new_var(key, iv).unwrap();
            let pt = cipher.decrypt(&mut buf[..ct_len]).unwrap();
            assert!(pt.iter().all(|&b| b == 128));
        }
    }

    run::<block_modes::Cbc<_, _>>();
    run::<block_modes::Cfb8<_, _>>();
    run::<block_modes::Ecb<_, _>>();
    run::<block_modes::Ofb<_, _>>();
    run::<block_modes::Pcbc<_, _>>();
}
