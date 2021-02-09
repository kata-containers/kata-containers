use core::marker::PhantomData;
use core::{cmp, iter, result, slice, str};

use crate::endian::LittleEndian as LE;
use crate::pe;
use crate::pod::Bytes;
use crate::read::{
    self, CompressedData, ObjectSection, ObjectSegment, ReadError, Relocation, Result,
    SectionFlags, SectionIndex, SectionKind,
};

use super::{ImageNtHeaders, PeFile};

/// An iterator over the loadable sections of a `PeFile32`.
pub type PeSegmentIterator32<'data, 'file> = PeSegmentIterator<'data, 'file, pe::ImageNtHeaders32>;
/// An iterator over the loadable sections of a `PeFile64`.
pub type PeSegmentIterator64<'data, 'file> = PeSegmentIterator<'data, 'file, pe::ImageNtHeaders64>;

/// An iterator over the loadable sections of a `PeFile`.
#[derive(Debug)]
pub struct PeSegmentIterator<'data, 'file, Pe>
where
    'data: 'file,
    Pe: ImageNtHeaders,
{
    pub(super) file: &'file PeFile<'data, Pe>,
    pub(super) iter: slice::Iter<'file, pe::ImageSectionHeader>,
}

impl<'data, 'file, Pe: ImageNtHeaders> Iterator for PeSegmentIterator<'data, 'file, Pe> {
    type Item = PeSegment<'data, 'file, Pe>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|section| PeSegment {
            file: self.file,
            section,
        })
    }
}

/// A loadable section of a `PeFile32`.
pub type PeSegment32<'data, 'file> = PeSegment<'data, 'file, pe::ImageNtHeaders32>;
/// A loadable section of a `PeFile64`.
pub type PeSegment64<'data, 'file> = PeSegment<'data, 'file, pe::ImageNtHeaders64>;

/// A loadable section of a `PeFile`.
#[derive(Debug)]
pub struct PeSegment<'data, 'file, Pe>
where
    'data: 'file,
    Pe: ImageNtHeaders,
{
    file: &'file PeFile<'data, Pe>,
    section: &'file pe::ImageSectionHeader,
}

impl<'data, 'file, Pe: ImageNtHeaders> PeSegment<'data, 'file, Pe> {
    fn bytes(&self) -> Result<Bytes<'data>> {
        self.section
            .pe_data(self.file.data)
            .read_error("Invalid PE section offset or size")
    }
}

impl<'data, 'file, Pe: ImageNtHeaders> read::private::Sealed for PeSegment<'data, 'file, Pe> {}

impl<'data, 'file, Pe: ImageNtHeaders> ObjectSegment<'data> for PeSegment<'data, 'file, Pe> {
    #[inline]
    fn address(&self) -> u64 {
        u64::from(self.section.virtual_address.get(LE))
    }

    #[inline]
    fn size(&self) -> u64 {
        u64::from(self.section.virtual_size.get(LE))
    }

    #[inline]
    fn align(&self) -> u64 {
        self.file.section_alignment()
    }

    #[inline]
    fn file_range(&self) -> (u64, u64) {
        let (offset, size) = self.section.pe_file_range();
        (u64::from(offset), u64::from(size))
    }

    fn data(&self) -> Result<&'data [u8]> {
        Ok(self.bytes()?.0)
    }

    fn data_range(&self, address: u64, size: u64) -> Result<Option<&'data [u8]>> {
        Ok(read::data_range(
            self.bytes()?,
            self.address(),
            address,
            size,
        ))
    }

    #[inline]
    fn name(&self) -> Result<Option<&str>> {
        let name = self.section.name(self.file.symbols.strings())?;
        Ok(Some(
            str::from_utf8(name)
                .ok()
                .read_error("Non UTF-8 PE section name")?,
        ))
    }
}

/// An iterator over the sections of a `PeFile32`.
pub type PeSectionIterator32<'data, 'file> = PeSectionIterator<'data, 'file, pe::ImageNtHeaders32>;
/// An iterator over the sections of a `PeFile64`.
pub type PeSectionIterator64<'data, 'file> = PeSectionIterator<'data, 'file, pe::ImageNtHeaders64>;

/// An iterator over the sections of a `PeFile`.
#[derive(Debug)]
pub struct PeSectionIterator<'data, 'file, Pe>
where
    'data: 'file,
    Pe: ImageNtHeaders,
{
    pub(super) file: &'file PeFile<'data, Pe>,
    pub(super) iter: iter::Enumerate<slice::Iter<'file, pe::ImageSectionHeader>>,
}

impl<'data, 'file, Pe: ImageNtHeaders> Iterator for PeSectionIterator<'data, 'file, Pe> {
    type Item = PeSection<'data, 'file, Pe>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(index, section)| PeSection {
            file: self.file,
            index: SectionIndex(index + 1),
            section,
        })
    }
}

/// A section of a `PeFile32`.
pub type PeSection32<'data, 'file> = PeSection<'data, 'file, pe::ImageNtHeaders32>;
/// A section of a `PeFile64`.
pub type PeSection64<'data, 'file> = PeSection<'data, 'file, pe::ImageNtHeaders64>;

/// A section of a `PeFile`.
#[derive(Debug)]
pub struct PeSection<'data, 'file, Pe>
where
    'data: 'file,
    Pe: ImageNtHeaders,
{
    pub(super) file: &'file PeFile<'data, Pe>,
    pub(super) index: SectionIndex,
    pub(super) section: &'file pe::ImageSectionHeader,
}

impl<'data, 'file, Pe: ImageNtHeaders> PeSection<'data, 'file, Pe> {
    fn bytes(&self) -> Result<Bytes<'data>> {
        self.section
            .pe_data(self.file.data)
            .read_error("Invalid PE section offset or size")
    }
}

impl<'data, 'file, Pe: ImageNtHeaders> read::private::Sealed for PeSection<'data, 'file, Pe> {}

impl<'data, 'file, Pe: ImageNtHeaders> ObjectSection<'data> for PeSection<'data, 'file, Pe> {
    type RelocationIterator = PeRelocationIterator<'data, 'file>;

    #[inline]
    fn index(&self) -> SectionIndex {
        self.index
    }

    #[inline]
    fn address(&self) -> u64 {
        u64::from(self.section.virtual_address.get(LE))
    }

    #[inline]
    fn size(&self) -> u64 {
        u64::from(self.section.virtual_size.get(LE))
    }

    #[inline]
    fn align(&self) -> u64 {
        self.file.section_alignment()
    }

    #[inline]
    fn file_range(&self) -> Option<(u64, u64)> {
        let (offset, size) = self.section.pe_file_range();
        if size == 0 {
            None
        } else {
            Some((u64::from(offset), u64::from(size)))
        }
    }

    fn data(&self) -> Result<&'data [u8]> {
        Ok(self.bytes()?.0)
    }

    fn data_range(&self, address: u64, size: u64) -> Result<Option<&'data [u8]>> {
        Ok(read::data_range(
            self.bytes()?,
            self.address(),
            address,
            size,
        ))
    }

    #[inline]
    fn compressed_data(&self) -> Result<CompressedData<'data>> {
        self.data().map(CompressedData::none)
    }

    #[inline]
    fn name(&self) -> Result<&str> {
        let name = self.section.name(self.file.symbols.strings())?;
        str::from_utf8(name)
            .ok()
            .read_error("Non UTF-8 PE section name")
    }

    #[inline]
    fn segment_name(&self) -> Result<Option<&str>> {
        Ok(None)
    }

    #[inline]
    fn kind(&self) -> SectionKind {
        self.section.kind()
    }

    fn relocations(&self) -> PeRelocationIterator<'data, 'file> {
        PeRelocationIterator::default()
    }

    fn flags(&self) -> SectionFlags {
        SectionFlags::Coff {
            characteristics: self.section.characteristics.get(LE),
        }
    }
}

impl pe::ImageSectionHeader {
    fn pe_file_range(&self) -> (u32, u32) {
        // Pointer and size will be zero for uninitialized data; we don't need to validate this.
        let offset = self.pointer_to_raw_data.get(LE);
        let size = cmp::min(self.virtual_size.get(LE), self.size_of_raw_data.get(LE));
        (offset, size)
    }

    /// Return the data for a PE section.
    pub fn pe_data<'data>(&self, data: Bytes<'data>) -> result::Result<Bytes<'data>, ()> {
        let (offset, size) = self.pe_file_range();
        data.read_bytes_at(offset as usize, size as usize)
    }
}

/// An iterator over the relocations in an `PeSection`.
#[derive(Debug, Default)]
pub struct PeRelocationIterator<'data, 'file>(PhantomData<(&'data (), &'file ())>);

impl<'data, 'file> Iterator for PeRelocationIterator<'data, 'file> {
    type Item = (u64, Relocation);

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}
