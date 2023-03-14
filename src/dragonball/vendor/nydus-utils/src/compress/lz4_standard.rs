// Copyright (C) 2020 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::io::Result;

use libc::c_char;
use lz4_sys::{LZ4_compressBound, LZ4_compress_default, LZ4_decompress_safe};

pub(super) fn lz4_compress(src: &[u8]) -> Result<Vec<u8>> {
    // 0 iff src too large
    let compress_bound: i32 = unsafe { LZ4_compressBound(src.len() as i32) };

    if src.len() > (i32::max_value() as usize) || compress_bound <= 0 {
        return Err(einval!("compression input data is too big"));
    }

    let mut dst_buf = Vec::with_capacity(compress_bound as usize);
    let cmp_size = unsafe {
        LZ4_compress_default(
            src.as_ptr() as *const c_char,
            dst_buf.as_mut_ptr() as *mut c_char,
            src.len() as i32,
            compress_bound,
        )
    };
    if cmp_size <= 0 {
        return Err(eio!("compression failed"));
    }

    assert!(cmp_size as usize <= dst_buf.capacity());
    unsafe { dst_buf.set_len(cmp_size as usize) };

    Ok(dst_buf)
}

pub(super) fn lz4_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize> {
    if dst.len() >= std::i32::MAX as usize {
        return Err(einval!("the destination buffer is big than i32::MAX"));
    }
    let size = dst.len() as i32;

    if unsafe { LZ4_compressBound(size) } <= 0 {
        return Err(einval!("given size parameter is too big"));
    }

    let dec_bytes = unsafe {
        LZ4_decompress_safe(
            src.as_ptr() as *const c_char,
            dst.as_mut_ptr() as *mut c_char,
            src.len() as i32,
            size,
        )
    };

    if dec_bytes < 0 {
        return Err(eio!("decompression failed"));
    }

    Ok(dec_bytes as usize)
}
