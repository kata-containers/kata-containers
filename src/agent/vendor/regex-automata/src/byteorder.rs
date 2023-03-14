use core::convert::TryInto;

pub trait ByteOrder {
    fn read_u16(buf: &[u8]) -> u16;
    fn read_u32(buf: &[u8]) -> u32;
    fn read_u64(buf: &[u8]) -> u64;
    fn read_uint(buf: &[u8], nbytes: usize) -> u64;
    fn write_u16(buf: &mut [u8], n: u16);
    fn write_u32(buf: &mut [u8], n: u32);
    fn write_u64(buf: &mut [u8], n: u64);
    fn write_uint(buf: &mut [u8], n: u64, nbytes: usize);
}

pub enum BigEndian {}
pub enum LittleEndian {}
pub enum NativeEndian {}

macro_rules! impl_endian {
    ($t:ty, $from_endian:ident, $to_endian:ident) => {
        impl ByteOrder for $t {
            #[inline]
            fn read_u16(buf: &[u8]) -> u16 {
                u16::$from_endian(buf[0..2].try_into().unwrap())
            }

            #[inline]
            fn read_u32(buf: &[u8]) -> u32 {
                u32::$from_endian(buf[0..4].try_into().unwrap())
            }

            #[inline]
            fn read_u64(buf: &[u8]) -> u64 {
                u64::$from_endian(buf[0..8].try_into().unwrap())
            }

            #[inline]
            fn read_uint(buf: &[u8], nbytes: usize) -> u64 {
                let mut dst = [0u8; 8];
                dst[..nbytes].copy_from_slice(&buf[..nbytes]);
                u64::$from_endian(dst)
            }

            #[inline]
            fn write_u16(buf: &mut [u8], n: u16) {
                buf[0..2].copy_from_slice(&n.$to_endian()[..]);
            }

            #[inline]
            fn write_u32(buf: &mut [u8], n: u32) {
                buf[0..4].copy_from_slice(&n.$to_endian()[..]);
            }

            #[inline]
            fn write_u64(buf: &mut [u8], n: u64) {
                buf[0..8].copy_from_slice(&n.$to_endian()[..]);
            }

            #[inline]
            fn write_uint(buf: &mut [u8], n: u64, nbytes: usize) {
                buf[..nbytes].copy_from_slice(&n.$to_endian()[..nbytes]);
            }
        }
    };
}

impl_endian! {
    BigEndian, from_be_bytes, to_be_bytes
}

impl_endian! {
    LittleEndian, from_le_bytes, to_le_bytes
}

impl_endian! {
    NativeEndian, from_ne_bytes, to_ne_bytes
}
