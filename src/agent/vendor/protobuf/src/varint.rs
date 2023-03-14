use std::mem::MaybeUninit;

use crate::misc::maybe_uninit_write;

/// Encode u64 as varint.
/// Panics if buffer length is less than 10.
#[inline]
pub fn encode_varint64(mut value: u64, buf: &mut [MaybeUninit<u8>]) -> usize {
    assert!(buf.len() >= 10);

    unsafe {
        let mut i = 0;
        while (value & !0x7F) > 0 {
            maybe_uninit_write(buf.get_unchecked_mut(i), ((value & 0x7F) | 0x80) as u8);
            value >>= 7;
            i += 1;
        }
        maybe_uninit_write(buf.get_unchecked_mut(i), value as u8);
        i + 1
    }
}

/// Encode u32 value as varint.
/// Panics if buffer length is less than 5.
#[inline]
pub fn encode_varint32(mut value: u32, buf: &mut [MaybeUninit<u8>]) -> usize {
    assert!(buf.len() >= 5);

    unsafe {
        let mut i = 0;
        while (value & !0x7F) > 0 {
            maybe_uninit_write(buf.get_unchecked_mut(i), ((value & 0x7F) | 0x80) as u8);
            value >>= 7;
            i += 1;
        }
        maybe_uninit_write(buf.get_unchecked_mut(i), value as u8);
        i + 1
    }
}
