use core::fmt::Debug;
use core::{iter, slice, str};

use crate::elf;
use crate::endian::{Endianness, U32Bytes};
use crate::pod::Bytes;
use crate::read::{self, ComdatKind, ObjectComdat, ReadError, SectionIndex, SymbolIndex};

use super::{ElfFile, FileHeader, SectionHeader, Sym};

/// An iterator over the COMDAT section groups of an `ElfFile32`.
pub type ElfComdatIterator32<'data, 'file, Endian = Endianness> =
    ElfComdatIterator<'data, 'file, elf::FileHeader32<Endian>>;
/// An iterator over the COMDAT section groups of an `ElfFile64`.
pub type ElfComdatIterator64<'data, 'file, Endian = Endianness> =
    ElfComdatIterator<'data, 'file, elf::FileHeader64<Endian>>;

/// An iterator over the COMDAT section groups of an `ElfFile`.
#[derive(Debug)]
pub struct ElfComdatIterator<'data, 'file, Elf>
where
    'data: 'file,
    Elf: FileHeader,
{
    pub(super) file: &'file ElfFile<'data, Elf>,
    pub(super) iter: iter::Enumerate<slice::Iter<'data, Elf::SectionHeader>>,
}

impl<'data, 'file, Elf: FileHeader> Iterator for ElfComdatIterator<'data, 'file, Elf> {
    type Item = ElfComdat<'data, 'file, Elf>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((index, section)) = self.iter.next() {
            if let Some(comdat) = ElfComdat::parse(self.file, index, section) {
                return Some(comdat);
            }
        }
        None
    }
}

/// A COMDAT section group of an `ElfFile32`.
pub type ElfComdat32<'data, 'file, Endian = Endianness> =
    ElfComdat<'data, 'file, elf::FileHeader32<Endian>>;
/// A COMDAT section group of an `ElfFile64`.
pub type ElfComdat64<'data, 'file, Endian = Endianness> =
    ElfComdat<'data, 'file, elf::FileHeader64<Endian>>;

/// A COMDAT section group of an `ElfFile`.
#[derive(Debug)]
pub struct ElfComdat<'data, 'file, Elf>
where
    'data: 'file,
    Elf: FileHeader,
{
    file: &'file ElfFile<'data, Elf>,
    index: SectionIndex,
    section: &'data Elf::SectionHeader,
    data: Bytes<'data>,
}

impl<'data, 'file, Elf: FileHeader> ElfComdat<'data, 'file, Elf> {
    fn parse(
        file: &'file ElfFile<'data, Elf>,
        index: usize,
        section: &'data Elf::SectionHeader,
    ) -> Option<ElfComdat<'data, 'file, Elf>> {
        if section.sh_type(file.endian) != elf::SHT_GROUP {
            return None;
        }
        let mut data = section.data(file.endian, file.data).ok()?;
        let flags = data.read::<U32Bytes<_>>().ok()?;
        if flags.get(file.endian) != elf::GRP_COMDAT {
            return None;
        }
        Some(ElfComdat {
            file,
            index: SectionIndex(index),
            section,
            data,
        })
    }
}

impl<'data, 'file, Elf: FileHeader> read::private::Sealed for ElfComdat<'data, 'file, Elf> {}

impl<'data, 'file, Elf: FileHeader> ObjectComdat<'data> for ElfComdat<'data, 'file, Elf> {
    type SectionIterator = ElfComdatSectionIterator<'data, 'file, Elf>;

    #[inline]
    fn kind(&self) -> ComdatKind {
        ComdatKind::Any
    }

    #[inline]
    fn symbol(&self) -> SymbolIndex {
        SymbolIndex(self.section.sh_info(self.file.endian) as usize)
    }

    fn name(&self) -> read::Result<&str> {
        // FIXME: check sh_link
        let index = self.section.sh_info(self.file.endian) as usize;
        let symbol = self.file.symbols.symbol(index)?;
        let name = symbol.name(self.file.endian, self.file.symbols.strings())?;
        str::from_utf8(name)
            .ok()
            .read_error("Non UTF-8 ELF COMDAT name")
    }

    fn sections(&self) -> Self::SectionIterator {
        ElfComdatSectionIterator {
            file: self.file,
            data: self.data,
        }
    }
}

/// An iterator over the sections in a COMDAT section group of an `ElfFile32`.
pub type ElfComdatSectionIterator32<'data, 'file, Endian = Endianness> =
    ElfComdatSectionIterator<'data, 'file, elf::FileHeader32<Endian>>;
/// An iterator over the sections in a COMDAT section group of an `ElfFile64`.
pub type ElfComdatSectionIterator64<'data, 'file, Endian = Endianness> =
    ElfComdatSectionIterator<'data, 'file, elf::FileHeader64<Endian>>;

/// An iterator over the sections in a COMDAT section group of an `ElfFile`.
#[derive(Debug)]
pub struct ElfComdatSectionIterator<'data, 'file, Elf>
where
    'data: 'file,
    Elf: FileHeader,
{
    file: &'file ElfFile<'data, Elf>,
    data: Bytes<'data>,
}

impl<'data, 'file, Elf: FileHeader> Iterator for ElfComdatSectionIterator<'data, 'file, Elf> {
    type Item = SectionIndex;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.is_empty() {
            None
        } else {
            let index = self.data.read::<U32Bytes<_>>().ok()?;
            Some(SectionIndex(index.get(self.file.endian) as usize))
        }
    }
}
