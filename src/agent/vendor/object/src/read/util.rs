use crate::pod::Bytes;

#[inline]
pub(crate) fn align(offset: usize, size: usize) -> usize {
    (offset + (size - 1)) & !(size - 1)
}

/// A table of zero-terminated strings.
///
/// This is used for most file formats.
#[derive(Debug, Default, Clone, Copy)]
pub struct StringTable<'data> {
    data: Bytes<'data>,
}

impl<'data> StringTable<'data> {
    /// Interpret the given data as a string table.
    pub fn new(data: Bytes<'data>) -> Self {
        StringTable { data }
    }

    /// Return the string at the given offset.
    pub fn get(&self, offset: u32) -> Result<&'data [u8], ()> {
        self.data.read_string_at(offset as usize)
    }
}
