#![no_std]

use digest::dev::{fixed_reset_test, xof_reset_test};
use digest::new_test;

new_test!(keccak_224, "keccak_224", sha3::Keccak224, fixed_reset_test);
new_test!(keccak_256, "keccak_256", sha3::Keccak256, fixed_reset_test);
new_test!(keccak_384, "keccak_384", sha3::Keccak384, fixed_reset_test);
new_test!(keccak_512, "keccak_512", sha3::Keccak512, fixed_reset_test);
// tests are from https://github.com/kazcw/yellowsun/blob/test-keccak/src/lib.rs#L171
new_test!(
    keccak_256_full,
    "keccak_256_full",
    sha3::Keccak256Full,
    fixed_reset_test
);

new_test!(sha3_224, "sha3_224", sha3::Sha3_224, fixed_reset_test);
new_test!(sha3_256, "sha3_256", sha3::Sha3_256, fixed_reset_test);
new_test!(sha3_384, "sha3_384", sha3::Sha3_384, fixed_reset_test);
new_test!(sha3_512, "sha3_512", sha3::Sha3_512, fixed_reset_test);

new_test!(shake128, "shake128", sha3::Shake128, xof_reset_test);
new_test!(shake256, "shake256", sha3::Shake256, xof_reset_test);
