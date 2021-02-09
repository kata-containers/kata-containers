use crate::pod::WritableBuffer;

pub(crate) fn align(offset: usize, size: usize) -> usize {
    (offset + (size - 1)) & !(size - 1)
}

pub(crate) fn align_u64(offset: u64, size: u64) -> u64 {
    (offset + (size - 1)) & !(size - 1)
}

pub(crate) fn write_align(buffer: &mut dyn WritableBuffer, size: usize) {
    let new_len = align(buffer.len(), size);
    buffer.resize(new_len, 0);
}
