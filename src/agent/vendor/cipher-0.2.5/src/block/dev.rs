//! Development-related functionality

pub use blobby;

/// Define block cipher test
#[macro_export]
#[cfg_attr(docsrs, doc(cfg(feature = "dev")))]
macro_rules! block_cipher_test {
    ($name:ident, $test_name:expr, $cipher:ty) => {
        #[test]
        fn $name() {
            use cipher::block::{dev::blobby::Blob3Iterator, BlockCipher, NewBlockCipher};
            use cipher::generic_array::{typenum::Unsigned, GenericArray};

            fn run_test(key: &[u8], pt: &[u8], ct: &[u8]) -> bool {
                let state = <$cipher as NewBlockCipher>::new_varkey(key).unwrap();

                let mut block = GenericArray::clone_from_slice(pt);
                state.encrypt_block(&mut block);
                if ct != block.as_slice() {
                    return false;
                }

                state.decrypt_block(&mut block);
                if pt != block.as_slice() {
                    return false;
                }

                true
            }

            fn run_par_test(key: &[u8], pt: &[u8]) -> bool {
                type ParBlocks = <$cipher as BlockCipher>::ParBlocks;
                type BlockSize = <$cipher as BlockCipher>::BlockSize;
                type Block = GenericArray<u8, BlockSize>;
                type ParBlock = GenericArray<Block, ParBlocks>;

                let state = <$cipher as NewBlockCipher>::new_varkey(key).unwrap();

                let block = Block::clone_from_slice(pt);
                let mut blocks1 = ParBlock::default();
                for (i, b) in blocks1.iter_mut().enumerate() {
                    *b = block;
                    b[0] = b[0].wrapping_add(i as u8);
                }
                let mut blocks2 = blocks1.clone();

                // check that `encrypt_blocks` and `encrypt_block`
                // result in the same ciphertext
                state.encrypt_blocks(&mut blocks1);
                for b in blocks2.iter_mut() {
                    state.encrypt_block(b);
                }
                if blocks1 != blocks2 {
                    return false;
                }

                // check that `encrypt_blocks` and `encrypt_block`
                // result in the same plaintext
                state.decrypt_blocks(&mut blocks1);
                for b in blocks2.iter_mut() {
                    state.decrypt_block(b);
                }
                if blocks1 != blocks2 {
                    return false;
                }

                true
            }

            let pb = <$cipher as BlockCipher>::ParBlocks::to_usize();
            let data = include_bytes!(concat!("data/", $test_name, ".blb"));
            for (i, row) in Blob3Iterator::new(data).unwrap().enumerate() {
                let [key, pt, ct] = row.unwrap();
                if !run_test(key, pt, ct) {
                    panic!(
                        "\n\
                         Failed test №{}\n\
                         key:\t{:?}\n\
                         plaintext:\t{:?}\n\
                         ciphertext:\t{:?}\n",
                        i, key, pt, ct,
                    );
                }

                // test parallel blocks encryption/decryption
                if pb != 1 {
                    if !run_par_test(key, pt) {
                        panic!(
                            "\n\
                             Failed parallel test №{}\n\
                             key:\t{:?}\n\
                             plaintext:\t{:?}\n\
                             ciphertext:\t{:?}\n",
                            i, key, pt, ct,
                        );
                    }
                }
            }
            // test if cipher can be cloned
            let key = Default::default();
            let _ = <$cipher as NewBlockCipher>::new(&key).clone();
        }
    };
}

/// Define block cipher benchmark
#[macro_export]
#[cfg_attr(docsrs, doc(cfg(feature = "dev")))]
macro_rules! block_cipher_bench {
    ($cipher:path, $key_len:expr) => {
        extern crate test;

        use cipher::block::{BlockCipher, NewBlockCipher};
        use test::Bencher;

        #[bench]
        pub fn encrypt(bh: &mut Bencher) {
            let state = <$cipher>::new_varkey(&[1u8; $key_len]).unwrap();
            let mut block = Default::default();

            bh.iter(|| {
                state.encrypt_block(&mut block);
                test::black_box(&block);
            });
            bh.bytes = block.len() as u64;
        }

        #[bench]
        pub fn decrypt(bh: &mut Bencher) {
            let state = <$cipher>::new_varkey(&[1u8; $key_len]).unwrap();
            let mut block = Default::default();

            bh.iter(|| {
                state.decrypt_block(&mut block);
                test::black_box(&block);
            });
            bh.bytes = block.len() as u64;
        }
    };
}

//
// Below are deprecated legacy macro wrappers. They should be removed in v0.3.
//

/// Define tests
#[macro_export]
#[deprecated(since = "0.2.2", note = "use `block_cipher_test!` instead")]
macro_rules! new_test {
    ($name:ident, $test_name:expr, $cipher:ty) => {
        $crate::block_cipher_test!($name, $test_name, $cipher);
    };
}

/// Define benchmark
#[macro_export]
#[deprecated(since = "0.2.2", note = "use `block_cipher_bench!` instead")]
macro_rules! bench {
    ($cipher:path, $key_len:expr) => {
        $crate::block_cipher_bench!($cipher, $key_len);
    };
}
