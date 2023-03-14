#![cfg(feature = "ctr")]
#![feature(test)]

#[cfg(feature = "ctr")]
cipher::bench_sync!(aesni::Aes256Ctr);
