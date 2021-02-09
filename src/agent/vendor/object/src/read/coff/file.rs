use core::str;

use crate::read::{
    self, Architecture, FileFlags, Object, ObjectSection, ReadError, Result, SectionIndex,
    SymbolIndex,
};
use crate::{pe, Bytes, LittleEndian as LE};

use super::{
    CoffComdat, CoffComdatIterator, CoffSection, CoffSectionIterator, CoffSegment,
    CoffSegmentIterator, CoffSymbol, CoffSymbolIterator, CoffSymbolTable, SectionTable,
    SymbolTable,
};

/// The common parts of `PeFile` and `CoffFile`.
#[derive(Debug)]
pub(crate) struct CoffCommon<'data> {
    pub(crate) sections: SectionTable<'data>,
    // TODO: ImageSymbolExBytes
    pub(crate) symbols: SymbolTable<'data>,
    pub(crate) image_base: u64,
}

/// A COFF object file.
#[derive(Debug)]
pub struct CoffFile<'data> {
    pub(super) header: &'data pe::ImageFileHeader,
    pub(super) common: CoffCommon<'data>,
    pub(super) data: Bytes<'data>,
}

impl<'data> CoffFile<'data> {
    /// Parse the raw COFF file data.
    pub fn parse(data: &'data [u8]) -> Result<Self> {
        let data = Bytes(data);
        let (header, tail) = pe::ImageFileHeader::parse(data)?;
        let sections = header.sections(tail)?;
        let symbols = header.symbols(data)?;

        Ok(CoffFile {
            header,
            common: CoffCommon {
                sections,
                symbols,
                image_base: 0,
            },
            data,
        })
    }
}

impl<'data> read::private::Sealed for CoffFile<'data> {}

impl<'data, 'file> Object<'data, 'file> for CoffFile<'data>
where
    'data: 'file,
{
    type Segment = CoffSegment<'data, 'file>;
    type SegmentIterator = CoffSegmentIterator<'data, 'file>;
    type Section = CoffSection<'data, 'file>;
    type SectionIterator = CoffSectionIterator<'data, 'file>;
    type Comdat = CoffComdat<'data, 'file>;
    type ComdatIterator = CoffComdatIterator<'data, 'file>;
    type Symbol = CoffSymbol<'data, 'file>;
    type SymbolIterator = CoffSymbolIterator<'data, 'file>;
    type SymbolTable = CoffSymbolTable<'data, 'file>;

    fn architecture(&self) -> Architecture {
        match self.header.machine.get(LE) {
            pe::IMAGE_FILE_MACHINE_I386 => Architecture::I386,
            pe::IMAGE_FILE_MACHINE_AMD64 => Architecture::X86_64,
            _ => Architecture::Unknown,
        }
    }

    #[inline]
    fn is_little_endian(&self) -> bool {
        true
    }

    #[inline]
    fn is_64(&self) -> bool {
        // Windows COFF is always 32-bit, even for 64-bit architectures. This could be confusing.
        false
    }

    fn segments(&'file self) -> CoffSegmentIterator<'data, 'file> {
        CoffSegmentIterator {
            file: self,
            iter: self.common.sections.iter(),
        }
    }

    fn section_by_name(&'file self, section_name: &str) -> Option<CoffSection<'data, 'file>> {
        self.sections()
            .find(|section| section.name() == Ok(section_name))
    }

    fn section_by_index(&'file self, index: SectionIndex) -> Result<CoffSection<'data, 'file>> {
        let section = self.common.sections.section(index.0)?;
        Ok(CoffSection {
            file: self,
            index,
            section,
        })
    }

    fn sections(&'file self) -> CoffSectionIterator<'data, 'file> {
        CoffSectionIterator {
            file: self,
            iter: self.common.sections.iter().enumerate(),
        }
    }

    fn comdats(&'file self) -> CoffComdatIterator<'data, 'file> {
        CoffComdatIterator {
            file: self,
            index: 0,
        }
    }

    fn symbol_by_index(&'file self, index: SymbolIndex) -> Result<CoffSymbol<'data, 'file>> {
        let symbol = self.common.symbols.symbol(index.0)?;
        Ok(CoffSymbol {
            file: &self.common,
            index,
            symbol,
        })
    }

    fn symbols(&'file self) -> CoffSymbolIterator<'data, 'file> {
        CoffSymbolIterator {
            file: &self.common,
            index: 0,
        }
    }

    #[inline]
    fn symbol_table(&'file self) -> Option<CoffSymbolTable<'data, 'file>> {
        Some(CoffSymbolTable { file: &self.common })
    }

    fn dynamic_symbols(&'file self) -> CoffSymbolIterator<'data, 'file> {
        CoffSymbolIterator {
            file: &self.common,
            // Hack: don't return any.
            index: self.common.symbols.len(),
        }
    }

    #[inline]
    fn dynamic_symbol_table(&'file self) -> Option<CoffSymbolTable<'data, 'file>> {
        None
    }

    fn has_debug_symbols(&self) -> bool {
        self.section_by_name(".debug_info").is_some()
    }

    #[inline]
    fn entry(&self) -> u64 {
        0
    }

    fn flags(&self) -> FileFlags {
        FileFlags::Coff {
            characteristics: self.header.characteristics.get(LE),
        }
    }
}

impl pe::ImageFileHeader {
    /// Read the DOS header.
    ///
    /// The given data must be for the entire file.  Returns the data following the optional
    /// header, which will contain the section headers.
    pub fn parse<'data>(mut data: Bytes<'data>) -> read::Result<(&'data Self, Bytes<'data>)> {
        let header = data
            .read::<pe::ImageFileHeader>()
            .read_error("Invalid COFF file header size or alignment")?;

        // Skip over the optional header.
        data.skip(header.size_of_optional_header.get(LE) as usize)
            .read_error("Invalid COFF optional header size")?;

        // TODO: maybe validate that the machine is known?
        Ok((header, data))
    }

    /// Read the section table.
    ///
    /// `tail` must be the data following the optional header.
    #[inline]
    fn sections<'data>(&self, tail: Bytes<'data>) -> read::Result<SectionTable<'data>> {
        SectionTable::parse(self, tail)
    }

    /// Read the symbol table and string table.
    ///
    /// `data` must be the entire file data.
    #[inline]
    fn symbols<'data>(&self, data: Bytes<'data>) -> read::Result<SymbolTable<'data>> {
        SymbolTable::parse(self, data)
    }
}
