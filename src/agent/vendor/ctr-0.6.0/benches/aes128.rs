#![feature(test)]

cipher::bench_sync!(ctr::Ctr128<aes::Aes128>);
