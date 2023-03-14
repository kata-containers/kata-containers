//! Test vectors are from NESSIE:
//! https://www.cosic.esat.kuleuven.be/nessie/testvectors/

cipher::new_test!(aes128_test, "aes128", aes::Aes128);
cipher::new_test!(aes192_test, "aes192", aes::Aes192);
cipher::new_test!(aes256_test, "aes256", aes::Aes256);
