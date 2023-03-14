#![feature(test)]
extern crate test;

use digest::bench_update;
use sha3::{Sha3_224, Sha3_256, Sha3_384, Sha3_512, Shake128, Shake256};
use test::Bencher;

bench_update!(
    Sha3_224::default();
    sha3_224_10 10;
    sha3_224_100 100;
    sha3_224_1000 1000;
    sha3_224_10000 10000;
);

bench_update!(
    Sha3_256::default();
    sha3_256_10 10;
    sha3_265_100 100;
    sha3_256_1000 1000;
    sha3_256_10000 10000;
);

bench_update!(
    Sha3_384::default();
    sha3_384_10 10;
    sha3_384_100 100;
    sha3_384_1000 1000;
    sha3_384_10000 10000;
);

bench_update!(
    Sha3_512::default();
    sha3_512_10 10;
    sha3_512_100 100;
    sha3_512_1000 1000;
    sha3_512_10000 10000;
);

bench_update!(
    Shake128::default();
    shake128_10 10;
    shake128_100 100;
    shake128_1000 1000;
    shake128_10000 10000;
);

bench_update!(
    Shake256::default();
    shake256_10 10;
    shake256_100 100;
    shake256_1000 1000;
    shake256_10000 10000;
);
