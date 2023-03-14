use crate::{Tag, Tagged};

#[derive(Debug)]
pub struct ConstInt {
    buffer: [u8; 10],
    n: usize,
}

// XXX only ToBer/ToDer trait supported?

impl Tagged for ConstInt {
    const TAG: Tag = Tag::Integer;
}

#[derive(Debug)]
pub struct IntBuilder {}

impl IntBuilder {
    pub const fn build(&self, i: u64) -> ConstInt {
        let b = i.to_be_bytes();
        let mut out = [0u8; 10];
        out[0] = 0x4;
        let src_len = b.len();
        let mut src_index = 0;
        while src_index < src_len && b[src_index] == 0 {
            src_index += 1;
        }
        out[1] = (src_len - src_index) as u8;
        let mut dst_index = 2;
        while src_index < src_len {
            out[dst_index] = b[src_index];
            src_index += 1;
            dst_index += 1;
        }
        // XXX will not work: we need to allocate a Vec
        // also, we cannot just store the bytes (there are extra zeroes at end)
        // Integer::new(&out[..dst_index])
        ConstInt {
            buffer: out,
            n: dst_index,
        }
    }
}
