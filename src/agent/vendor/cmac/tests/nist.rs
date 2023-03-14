//! Tests from NIST SP 800-38B:
//! https://csrc.nist.gov/projects/cryptographic-standards-and-guidelines/
#![no_std]
use aes::{Aes128, Aes192, Aes256};
use cmac::Cmac;
use crypto_mac::new_test;

new_test!(cmac_aes128, "aes128", Cmac<Aes128>);
new_test!(cmac_aes192, "aes192", Cmac<Aes192>);
new_test!(cmac_aes256, "aes256", Cmac<Aes256>);
