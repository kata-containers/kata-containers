//! Development-related functionality

/// Test core functionality of synchronous stream cipher
#[macro_export]
#[cfg_attr(docsrs, doc(cfg(feature = "dev")))]
macro_rules! stream_cipher_sync_test {
    ($name:ident, $cipher:ty, $test_name:expr) => {
        #[test]
        fn $name() {
            use cipher::generic_array::GenericArray;
            use cipher::stream::{blobby::Blob4Iterator, NewStreamCipher, SyncStreamCipher};

            let data = include_bytes!(concat!("data/", $test_name, ".blb"));
            for (i, row) in Blob4Iterator::new(data).unwrap().enumerate() {
                let [key, iv, pt, ct] = row.unwrap();

                for chunk_n in 1..256 {
                    let mut mode = <$cipher>::new_var(key, iv).unwrap();
                    let mut pt = pt.to_vec();
                    for chunk in pt.chunks_mut(chunk_n) {
                        mode.apply_keystream(chunk);
                    }
                    if pt != &ct[..] {
                        panic!(
                            "Failed main test №{}, chunk size: {}\n\
                            key:\t{:?}\n\
                            iv:\t{:?}\n\
                            plaintext:\t{:?}\n\
                            ciphertext:\t{:?}\n",
                            i, chunk_n, key, iv, pt, ct,
                        );
                    }
                }
            }
        }
    };
}

/// Test stream synchronous stream cipher seeking capabilities
#[macro_export]
#[cfg_attr(docsrs, doc(cfg(feature = "dev")))]
macro_rules! stream_cipher_seek_test {
    ($name:ident, $cipher:ty) => {
        #[test]
        fn $name() {
            use cipher::generic_array::GenericArray;
            use cipher::stream::{NewStreamCipher, SyncStreamCipher, SyncStreamCipherSeek};

            fn get_cipher() -> $cipher {
                <$cipher>::new(&Default::default(), &Default::default())
            }

            const MAX_SEEK: usize = 512;

            let mut ct = [0u8; MAX_SEEK];
            get_cipher().apply_keystream(&mut ct[..]);

            for n in 0..MAX_SEEK {
                let mut cipher = get_cipher();
                assert_eq!(cipher.current_pos::<usize>(), 0);
                cipher.seek(n);
                assert_eq!(cipher.current_pos::<usize>(), n);
                let mut buf = [0u8; MAX_SEEK];
                cipher.apply_keystream(&mut buf[n..]);
                assert_eq!(cipher.current_pos::<usize>(), MAX_SEEK);
                assert_eq!(&buf[n..], &ct[n..]);
            }

            const MAX_CHUNK: usize = 128;
            const MAX_LEN: usize = 1024;

            let mut buf = [0u8; MAX_CHUNK];
            let mut cipher = get_cipher();
            assert_eq!(cipher.current_pos::<usize>(), 0);
            cipher.apply_keystream(&mut []);
            assert_eq!(cipher.current_pos::<usize>(), 0);
            for n in 1..MAX_CHUNK {
                assert_eq!(cipher.current_pos::<usize>(), 0);
                for m in 1.. {
                    cipher.apply_keystream(&mut buf[..n]);
                    assert_eq!(cipher.current_pos::<usize>(), n * m);
                    if n * m > MAX_LEN {
                        break;
                    }
                }
                cipher.seek(0);
            }
        }
    };
}

/// Test core functionality of asynchronous stream cipher
#[macro_export]
#[cfg_attr(docsrs, doc(cfg(feature = "dev")))]
macro_rules! stream_cipher_async_test {
    ($name:ident, $test_name:expr, $cipher:ty) => {
        #[test]
        fn $name() {
            use cipher::generic_array::GenericArray;
            use cipher::stream::{blobby::Blob4Iterator, NewStreamCipher, StreamCipher};

            fn run_test(
                key: &[u8],
                iv: &[u8],
                plaintext: &[u8],
                ciphertext: &[u8],
            ) -> Option<&'static str> {
                for n in 1..=plaintext.len() {
                    let mut mode = <$cipher>::new_var(key, iv).unwrap();
                    let mut buf = plaintext.to_vec();
                    for chunk in buf.chunks_mut(n) {
                        mode.encrypt(chunk);
                    }
                    if buf != &ciphertext[..] {
                        return Some("encrypt");
                    }
                }

                for n in 1..=plaintext.len() {
                    let mut mode = <$cipher>::new_var(key, iv).unwrap();
                    let mut buf = ciphertext.to_vec();
                    for chunk in buf.chunks_mut(n) {
                        mode.decrypt(chunk);
                    }
                    if buf != &plaintext[..] {
                        return Some("decrypt");
                    }
                }

                None
            }

            let data = include_bytes!(concat!("data/", $test_name, ".blb"));

            for (i, row) in Blob4Iterator::new(data).unwrap().enumerate() {
                let [key, iv, pt, ct] = row.unwrap();
                if let Some(desc) = run_test(key, iv, pt, ct) {
                    panic!(
                        "\n\
                         Failed test №{}: {}\n\
                         key:\t{:?}\n\
                         iv:\t{:?}\n\
                         plaintext:\t{:?}\n\
                         ciphertext:\t{:?}\n",
                        i, desc, key, iv, pt, ct,
                    );
                }
            }
        }
    };
}

/// Create synchronous stream cipher benchmarks
#[macro_export]
#[cfg_attr(docsrs, doc(cfg(feature = "dev")))]
macro_rules! stream_cipher_sync_bench {
    ($name:ident, $cipher:path, $data_len:expr) => {
        #[bench]
        pub fn $name(bh: &mut Bencher) {
            let key = Default::default();
            let nonce = Default::default();
            let mut cipher = <$cipher>::new(&key, &nonce);
            let mut data = get_data($data_len);

            bh.iter(|| {
                cipher.apply_keystream(&mut data);
                test::black_box(&data);
            });
            bh.bytes = data.len() as u64;
        }
    };
    ($cipher:path) => {
        extern crate test;

        use cipher::generic_array::GenericArray;
        use cipher::stream::{NewStreamCipher, SyncStreamCipher};
        use test::Bencher;

        #[inline(never)]
        fn get_data(n: usize) -> Vec<u8> {
            vec![77; n]
        }

        $crate::stream_cipher_sync_bench!(bench1_10, $cipher, 10);
        $crate::stream_cipher_sync_bench!(bench2_100, $cipher, 100);
        $crate::stream_cipher_sync_bench!(bench3_1000, $cipher, 1000);
        $crate::stream_cipher_sync_bench!(bench4_10000, $cipher, 10000);
        $crate::stream_cipher_sync_bench!(bench5_100000, $cipher, 100000);
    };
}

/// Create asynchronous stream cipher benchmarks
#[macro_export]
#[cfg_attr(docsrs, doc(cfg(feature = "dev")))]
macro_rules! stream_cipher_async_bench {
    ($enc_name:ident, $dec_name:ident, $cipher:path, $data_len:expr) => {
        #[bench]
        pub fn $enc_name(bh: &mut Bencher) {
            let key = Default::default();
            let nonce = Default::default();
            let mut cipher = <$cipher>::new(&key, &nonce);
            let mut data = get_data($data_len);

            bh.iter(|| {
                cipher.encrypt(&mut data);
                test::black_box(&data);
            });
            bh.bytes = data.len() as u64;
        }

        #[bench]
        pub fn $dec_name(bh: &mut Bencher) {
            let key = Default::default();
            let nonce = Default::default();
            let mut cipher = <$cipher>::new(&key, &nonce);
            let mut data = get_data($data_len);

            bh.iter(|| {
                cipher.decrypt(&mut data);
                test::black_box(&data);
            });
            bh.bytes = data.len() as u64;
        }
    };
    ($cipher:path) => {
        extern crate test;

        use cipher::generic_array::GenericArray;
        use cipher::stream::{NewStreamCipher, StreamCipher};
        use test::Bencher;

        #[inline(never)]
        fn get_data(n: usize) -> Vec<u8> {
            vec![77; n]
        }

        $crate::stream_cipher_async_bench!(encrypt_10, decrypt_10, $cipher, 10);
        $crate::stream_cipher_async_bench!(encrypt_100, decrypt_100, $cipher, 100);
        $crate::stream_cipher_async_bench!(encrypt_1000, decrypt_1000, $cipher, 1000);
        $crate::stream_cipher_async_bench!(encrypt_10000, decrypt_10000, $cipher, 10000);
        $crate::stream_cipher_async_bench!(encrypt_100000, decrypt_100000, $cipher, 100000);
    };
}

//
// Below are deprecated legacy macro wrappers. They should be removed in v0.3.
//

/// Test core functionality of synchronous stream cipher
#[macro_export]
#[deprecated(since = "0.2.2", note = "use `stream_cipher_sync_test!` instead")]
macro_rules! new_sync_test {
    ($name:ident, $cipher:ty, $test_name:expr) => {
        $crate::stream_cipher_sync_test!($name, $cipher, $test_name);
    };
}

/// Test stream synchronous stream cipher seeking capabilities
#[macro_export]
#[deprecated(since = "0.2.2", note = "use `stream_cipher_seek_test!` instead")]
macro_rules! new_seek_test {
    ($name:ident, $cipher:ty) => {
        $crate::stream_cipher_seek_test!($name, $cipher);
    };
}

/// Test core functionality of asynchronous stream cipher
#[macro_export]
#[deprecated(since = "0.2.2", note = "use `stream_cipher_async_test!` instead")]
macro_rules! new_async_test {
    ($name:ident, $test_name:expr, $cipher:ty) => {
        $crate::stream_cipher_async_test!($name, $test_name, $cipher);
    };
}

/// Create synchronous stream cipher benchmarks
#[macro_export]
#[deprecated(since = "0.2.2", note = "use `stream_cipher_sync_bench!` instead")]
macro_rules! bench_sync {
    ($name:ident, $cipher:path, $data_len:expr) => {
        $crate::stream_cipher_sync_bench!($name, $cipher, $data_len);
    };
}

/// Create asynchronous stream cipher benchmarks
#[macro_export]
#[deprecated(since = "0.2.2", note = "use `stream_cipher_async_bench!` instead")]
macro_rules! bench_async {
    ($enc_name:ident, $dec_name:ident, $cipher:path, $data_len:expr) => {
        $crate::stream_cipher_async_bench!($enc_name, $dec_name, $cipher, $data_len);
    };
}
