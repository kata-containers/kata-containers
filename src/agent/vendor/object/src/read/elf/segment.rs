use core::fmt::Debug;
use core::{slice, str};

use crate::elf;
use crate::endian::{self, Endianness};
use crate::pod::{Bytes, Pod};
use crate::read::{self, ObjectSegment, ReadError};

use super::{ElfFile, ElfNoteIterator, FileHeader};

/// An iterator over the segments of an `ElfFile32`.
pub type ElfSegmentIterator32<'data, 'file, Endian = Endianness> =
    ElfSegmentIterator<'data, 'file, elf::FileHeader32<Endian>>;
/// An iterator over the segments of an `ElfFile64`.
pub type ElfSegmentIterator64<'data, 'file, Endian = Endianness> =
    ElfSegmentIterator<'data, 'file, elf::FileHeader64<Endian>>;

/// An iterator over the segments of an `ElfFile`.
#[derive(Debug)]
pub struct ElfSegmentIterator<'data, 'file, Elf>
where
    'data: 'file,
    Elf: FileHeader,
{
    pub(super) file: &'file ElfFile<'data, Elf>,
    pub(super) iter: slice::Iter<'data, Elf::ProgramHeader>,
}

impl<'data, 'file, Elf: FileHeader> Iterator for ElfSegmentIterator<'data, 'file, Elf> {
    type Item = ElfSegment<'data, 'file, Elf>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(segment) = self.iter.next() {
            if segment.p_type(self.file.endian) == elf::PT_LOAD {
                return Some(ElfSegment {
                    file: self.file,
                    segment,
                });
            }
        }
        None
    }
}

/// A segment of an `ElfFile32`.
pub type ElfSegment32<'data, 'file, Endian = Endianness> =
    ElfSegment<'data, 'file, elf::FileHeader32<Endian>>;
/// A segment of an `ElfFile64`.
pub type ElfSegment64<'data, 'file, Endian = Endianness> =
    ElfSegment<'data, 'file, elf::FileHeader64<Endian>>;

/// A segment of an `ElfFile`.
#[derive(Debug)]
pub struct ElfSegment<'data, 'file, Elf>
where
    'data: 'file,
    Elf: FileHeader,
{
    pub(super) file: &'file ElfFile<'data, Elf>,
    pub(super) segment: &'data Elf::ProgramHeader,
}

impl<'data, 'file, Elf: FileHeader> ElfSegment<'data, 'file, Elf> {
    fn bytes(&self) -> read::Result<Bytes<'data>> {
        self.segment
            .data(self.file.endian, self.file.data)
            .read_error("Invalid ELF segment size or offset")
    }
}

impl<'data, 'file, Elf: FileHeader> read::private::Sealed for ElfSegment<'data, 'file, Elf> {}

impl<'data, 'file, Elf: FileHeader> ObjectSegment<'data> for ElfSegment<'data, 'file, Elf> {
    #[inline]
    fn address(&self) -> u64 {
        self.segment.p_vaddr(self.file.endian).into()
    }

    #[inline]
    fn size(&self) -> u64 {
        self.segment.p_memsz(self.file.endian).into()
    }

    #[inline]
    fn align(&self) -> u64 {
        self.segment.p_align(self.file.endian).into()
    }

    #[inline]
    fn file_range(&self) -> (u64, u64) {
        self.segment.file_range(self.file.endian)
    }

    #[inline]
    fn data(&self) -> read::Result<&'data [u8]> {
        Ok(self.bytes()?.0)
    }

    fn data_range(&self, address: u64, size: u64) -> read::Result<Option<&'data [u8]>> {
        Ok(read::data_range(
            self.bytes()?,
            self.address(),
            address,
            size,
        ))
    }

    #[inline]
    fn name(&self) -> read::Result<Option<&str>> {
        Ok(None)
    }
}

/// A trait for generic access to `ProgramHeader32` and `ProgramHeader64`.
#[allow(missing_docs)]
pub trait ProgramHeader: Debug + Pod {
    type Word: Into<u64>;
    type Endian: endian::Endian;
    type Elf: FileHeader<Word = Self::Word, Endian = Self::Endian>;

    fn p_type(&self, endian: Self::Endian) -> u32;
    fn p_flags(&self, endian: Self::Endian) -> u32;
    fn p_offset(&self, endian: Self::Endian) -> Self::Word;
    fn p_vaddr(&self, endian: Self::Endian) -> Self::Word;
    fn p_paddr(&self, endian: Self::Endian) -> Self::Word;
    fn p_filesz(&self, endian: Self::Endian) -> Self::Word;
    fn p_memsz(&self, endian: Self::Endian) -> Self::Word;
    fn p_align(&self, endian: Self::Endian) -> Self::Word;

    /// Return the offset and size of the segment in the file.
    fn file_range(&self, endian: Self::Endian) -> (u64, u64) {
        (self.p_offset(endian).into(), self.p_filesz(endian).into())
    }

    /// Return the segment data.
    ///
    /// Returns `Err` for invalid values.
    fn data<'data>(&self, endian: Self::Endian, data: Bytes<'data>) -> Result<Bytes<'data>, ()> {
        let (offset, size) = self.file_range(endian);
        data.read_bytes_at(offset as usize, size as usize)
    }

    /// Return a note iterator for the segment data.
    ///
    /// Returns `Ok(None)` if the segment does not contain notes.
    /// Returns `Err` for invalid values.
    fn notes<'data>(
        &self,
        endian: Self::Endian,
        data: Bytes<'data>,
    ) -> read::Result<Option<ElfNoteIterator<'data, Self::Elf>>> {
        if self.p_type(endian) == elf::PT_NOTE {
            return Ok(None);
        }
        let data = self
            .data(endian, data)
            .read_error("Invalid ELF note segment offset or size")?;
        let notes = ElfNoteIterator::new(endian, self.p_align(endian), data)?;
        Ok(Some(notes))
    }
}

impl<Endian: endian::Endian> ProgramHeader for elf::ProgramHeader32<Endian> {
    type Word = u32;
    type Endian = Endian;
    type Elf = elf::FileHeader32<Endian>;

    #[inline]
    fn p_type(&self, endian: Self::Endian) -> u32 {
        self.p_type.get(endian)
    }

    #[inline]
    fn p_flags(&self, endian: Self::Endian) -> u32 {
        self.p_flags.get(endian)
    }

    #[inline]
    fn p_offset(&self, endian: Self::Endian) -> Self::Word {
        self.p_offset.get(endian)
    }

    #[inline]
    fn p_vaddr(&self, endian: Self::Endian) -> Self::Word {
        self.p_vaddr.get(endian)
    }

    #[inline]
    fn p_paddr(&self, endian: Self::Endian) -> Self::Word {
        self.p_paddr.get(endian)
    }

    #[inline]
    fn p_filesz(&self, endian: Self::Endian) -> Self::Word {
        self.p_filesz.get(endian)
    }

    #[inline]
    fn p_memsz(&self, endian: Self::Endian) -> Self::Word {
        self.p_memsz.get(endian)
    }

    #[inline]
    fn p_align(&self, endian: Self::Endian) -> Self::Word {
        self.p_align.get(endian)
    }
}

impl<Endian: endian::Endian> ProgramHeader for elf::ProgramHeader64<Endian> {
    type Word = u64;
    type Endian = Endian;
    type Elf = elf::FileHeader64<Endian>;

    #[inline]
    fn p_type(&self, endian: Self::Endian) -> u32 {
        self.p_type.get(endian)
    }

    #[inline]
    fn p_flags(&self, endian: Self::Endian) -> u32 {
        self.p_flags.get(endian)
    }

    #[inline]
    fn p_offset(&self, endian: Self::Endian) -> Self::Word {
        self.p_offset.get(endian)
    }

    #[inline]
    fn p_vaddr(&self, endian: Self::Endian) -> Self::Word {
        self.p_vaddr.get(endian)
    }

    #[inline]
    fn p_paddr(&self, endian: Self::Endian) -> Self::Word {
        self.p_paddr.get(endian)
    }

    #[inline]
    fn p_filesz(&self, endian: Self::Endian) -> Self::Word {
        self.p_filesz.get(endian)
    }

    #[inline]
    fn p_memsz(&self, endian: Self::Endian) -> Self::Word {
        self.p_memsz.get(endian)
    }

    #[inline]
    fn p_align(&self, endian: Self::Endian) -> Self::Word {
        self.p_align.get(endian)
    }
}
