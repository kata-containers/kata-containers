use std::io::Result;

use zstd::{
    bulk::{compress, decompress_to_buffer},
    DEFAULT_COMPRESSION_LEVEL,
};

pub(super) fn zstd_compress(src: &[u8]) -> Result<Vec<u8>> {
    compress(src, DEFAULT_COMPRESSION_LEVEL)
}

pub(super) fn zstd_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize> {
    decompress_to_buffer(src, dst)
}
