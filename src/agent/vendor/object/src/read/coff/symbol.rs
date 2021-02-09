use alloc::fmt;
use alloc::vec::Vec;
use core::convert::TryInto;
use core::str;

use super::{CoffCommon, SectionTable};
use crate::endian::{LittleEndian as LE, U32Bytes};
use crate::pe;
use crate::pod::{Bytes, Pod};
use crate::read::util::StringTable;
use crate::read::{
    self, ObjectSymbol, ObjectSymbolTable, ReadError, Result, SectionIndex, SymbolFlags,
    SymbolIndex, SymbolKind, SymbolMap, SymbolMapEntry, SymbolScope, SymbolSection,
};

/// A table of symbol entries in a COFF or PE file.
///
/// Also includes the string table used for the symbol names.
#[derive(Debug)]
pub struct SymbolTable<'data> {
    symbols: &'data [pe::ImageSymbolBytes],
    strings: StringTable<'data>,
}

impl<'data> SymbolTable<'data> {
    /// Read the symbol table.
    pub fn parse(header: &pe::ImageFileHeader, mut data: Bytes<'data>) -> Result<Self> {
        // The symbol table may not be present.
        let symbol_offset = header.pointer_to_symbol_table.get(LE) as usize;
        let (symbols, strings) = if symbol_offset != 0 {
            data.skip(symbol_offset)
                .read_error("Invalid COFF symbol table offset")?;
            let symbols = data
                .read_slice(header.number_of_symbols.get(LE) as usize)
                .read_error("Invalid COFF symbol table size")?;

            // Note: don't update data when reading length; the length includes itself.
            let length = data
                .read_at::<U32Bytes<_>>(0)
                .read_error("Missing COFF string table")?
                .get(LE);
            let strings = data
                .read_bytes(length as usize)
                .read_error("Invalid COFF string table length")?;

            (symbols, strings)
        } else {
            (&[][..], Bytes(&[]))
        };

        Ok(SymbolTable {
            symbols,
            strings: StringTable::new(strings),
        })
    }

    /// Return the string table used for the symbol names.
    #[inline]
    pub fn strings(&self) -> StringTable<'data> {
        self.strings
    }

    /// Return true if the symbol table is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// The number of symbols.
    #[inline]
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// Return the symbol table entry at the given index.
    #[inline]
    pub fn symbol(&self, index: usize) -> Result<&'data pe::ImageSymbol> {
        self.get::<pe::ImageSymbol>(index)
    }

    /// Return the symbol table entry or auxiliary record at the given index.
    pub fn get<T: Pod>(&self, index: usize) -> Result<&'data T> {
        let bytes = self
            .symbols
            .get(index)
            .read_error("Invalid COFF symbol index")?;
        Bytes(&bytes.0[..])
            .read()
            .read_error("Invalid COFF symbol data")
    }

    /// Construct a map from addresses to a user-defined map entry.
    pub fn map<Entry: SymbolMapEntry, F: Fn(&'data pe::ImageSymbol) -> Option<Entry>>(
        &self,
        f: F,
    ) -> SymbolMap<Entry> {
        let mut symbols = Vec::with_capacity(self.symbols.len());
        let mut i = 0;
        while let Ok(symbol) = self.symbol(i) {
            i += 1 + symbol.number_of_aux_symbols as usize;
            if !symbol.is_definition() {
                continue;
            }
            if let Some(entry) = f(symbol) {
                symbols.push(entry);
            }
        }
        SymbolMap::new(symbols)
    }
}

impl pe::ImageSymbol {
    /// Parse a COFF symbol name.
    ///
    /// `strings` must be the string table used for symbol names.
    pub fn name<'data>(&'data self, strings: StringTable<'data>) -> Result<&'data [u8]> {
        if self.name[0] == 0 {
            // If the name starts with 0 then the last 4 bytes are a string table offset.
            let offset = u32::from_le_bytes(self.name[4..8].try_into().unwrap());
            strings
                .get(offset)
                .read_error("Invalid COFF symbol name offset")
        } else {
            // The name is inline and padded with nulls.
            Ok(match self.name.iter().position(|&x| x == 0) {
                Some(end) => &self.name[..end],
                None => &self.name[..],
            })
        }
    }

    /// Return the symbol address.
    ///
    /// This takes into account the image base and the section address.
    pub fn address(&self, image_base: u64, sections: &SectionTable) -> Result<u64> {
        let section_number = self.section_number.get(LE) as usize;
        let section = sections.section(section_number)?;
        let virtual_address = u64::from(section.virtual_address.get(LE));
        let value = u64::from(self.value.get(LE));
        Ok(image_base + virtual_address + value)
    }

    /// Return true if the symbol is a definition of a function or data object.
    pub fn is_definition(&self) -> bool {
        let section_number = self.section_number.get(LE);
        if section_number == pe::IMAGE_SYM_UNDEFINED {
            return false;
        }
        match self.storage_class {
            pe::IMAGE_SYM_CLASS_STATIC => {
                if self.value.get(LE) == 0 && self.number_of_aux_symbols > 0 {
                    // This is a section symbol.
                    false
                } else {
                    true
                }
            }
            pe::IMAGE_SYM_CLASS_EXTERNAL | pe::IMAGE_SYM_CLASS_WEAK_EXTERNAL => true,
            _ => false,
        }
    }
}

/// A symbol table of a `CoffFile`.
#[derive(Debug, Clone, Copy)]
pub struct CoffSymbolTable<'data, 'file>
where
    'data: 'file,
{
    pub(crate) file: &'file CoffCommon<'data>,
}

impl<'data, 'file> read::private::Sealed for CoffSymbolTable<'data, 'file> {}

impl<'data, 'file> ObjectSymbolTable<'data> for CoffSymbolTable<'data, 'file> {
    type Symbol = CoffSymbol<'data, 'file>;
    type SymbolIterator = CoffSymbolIterator<'data, 'file>;

    fn symbols(&self) -> Self::SymbolIterator {
        CoffSymbolIterator {
            file: self.file,
            index: 0,
        }
    }

    fn symbol_by_index(&self, index: SymbolIndex) -> Result<Self::Symbol> {
        let symbol = self.file.symbols.symbol(index.0)?;
        Ok(CoffSymbol {
            file: self.file,
            index,
            symbol,
        })
    }
}

/// An iterator over the symbols of a `CoffFile`.
pub struct CoffSymbolIterator<'data, 'file>
where
    'data: 'file,
{
    pub(crate) file: &'file CoffCommon<'data>,
    pub(crate) index: usize,
}

impl<'data, 'file> fmt::Debug for CoffSymbolIterator<'data, 'file> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoffSymbolIterator").finish()
    }
}

impl<'data, 'file> Iterator for CoffSymbolIterator<'data, 'file> {
    type Item = CoffSymbol<'data, 'file>;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index;
        let symbol = self.file.symbols.symbol(index).ok()?;
        self.index += 1 + symbol.number_of_aux_symbols as usize;
        Some(CoffSymbol {
            file: self.file,
            index: SymbolIndex(index),
            symbol,
        })
    }
}

/// A symbol of a `CoffFile`.
#[derive(Debug, Clone, Copy)]
pub struct CoffSymbol<'data, 'file>
where
    'data: 'file,
{
    pub(crate) file: &'file CoffCommon<'data>,
    pub(crate) index: SymbolIndex,
    pub(crate) symbol: &'data pe::ImageSymbol,
}

impl<'data, 'file> read::private::Sealed for CoffSymbol<'data, 'file> {}

impl<'data, 'file> ObjectSymbol<'data> for CoffSymbol<'data, 'file> {
    #[inline]
    fn index(&self) -> SymbolIndex {
        self.index
    }

    fn name(&self) -> read::Result<&'data str> {
        let name = if self.symbol.storage_class == pe::IMAGE_SYM_CLASS_FILE {
            // The file name is in the following auxiliary symbol.
            if self.symbol.number_of_aux_symbols > 0 {
                let s = self
                    .file
                    .symbols
                    .get::<pe::ImageSymbolBytes>(self.index.0 + 1)?;
                // The name is padded with nulls.
                match s.0.iter().position(|&x| x == 0) {
                    Some(end) => &s.0[..end],
                    None => &s.0[..],
                }
            } else {
                &[][..]
            }
        } else {
            self.symbol.name(self.file.symbols.strings())?
        };
        str::from_utf8(name)
            .ok()
            .read_error("Non UTF-8 COFF symbol name")
    }

    fn address(&self) -> u64 {
        // Only return an address for storage classes that we know use an address.
        match self.symbol.storage_class {
            pe::IMAGE_SYM_CLASS_STATIC
            | pe::IMAGE_SYM_CLASS_WEAK_EXTERNAL
            | pe::IMAGE_SYM_CLASS_LABEL => {}
            pe::IMAGE_SYM_CLASS_EXTERNAL => {
                if self.symbol.section_number.get(LE) == pe::IMAGE_SYM_UNDEFINED {
                    // Undefined or common data, neither of which have an address.
                    return 0;
                }
            }
            _ => return 0,
        }
        self.symbol
            .address(self.file.image_base, &self.file.sections)
            .unwrap_or(0)
    }

    fn size(&self) -> u64 {
        match self.symbol.storage_class {
            pe::IMAGE_SYM_CLASS_STATIC => {
                // Section symbols may duplicate the size from the section table.
                if self.symbol.value.get(LE) == 0 && self.symbol.number_of_aux_symbols > 0 {
                    if let Ok(aux) = self
                        .file
                        .symbols
                        .get::<pe::ImageAuxSymbolSection>(self.index.0 + 1)
                    {
                        u64::from(aux.length.get(LE))
                    } else {
                        0
                    }
                } else {
                    0
                }
            }
            pe::IMAGE_SYM_CLASS_EXTERNAL => {
                if self.symbol.section_number.get(LE) == pe::IMAGE_SYM_UNDEFINED {
                    // For undefined symbols, symbol.value is 0 and the size is 0.
                    // For common data, symbol.value is the size.
                    u64::from(self.symbol.value.get(LE))
                } else if self.symbol.derived_type() == pe::IMAGE_SYM_DTYPE_FUNCTION
                    && self.symbol.number_of_aux_symbols > 0
                {
                    // Function symbols may have a size.
                    if let Ok(aux) = self
                        .file
                        .symbols
                        .get::<pe::ImageAuxSymbolFunction>(self.index.0 + 1)
                    {
                        u64::from(aux.total_size.get(LE))
                    } else {
                        0
                    }
                } else {
                    0
                }
            }
            // Most symbols don't have sizes.
            _ => 0,
        }
    }

    fn kind(&self) -> SymbolKind {
        let derived_kind = if self.symbol.derived_type() == pe::IMAGE_SYM_DTYPE_FUNCTION {
            SymbolKind::Text
        } else {
            SymbolKind::Data
        };
        match self.symbol.storage_class {
            pe::IMAGE_SYM_CLASS_STATIC => {
                if self.symbol.value.get(LE) == 0 && self.symbol.number_of_aux_symbols > 0 {
                    SymbolKind::Section
                } else {
                    derived_kind
                }
            }
            pe::IMAGE_SYM_CLASS_EXTERNAL | pe::IMAGE_SYM_CLASS_WEAK_EXTERNAL => derived_kind,
            pe::IMAGE_SYM_CLASS_SECTION => SymbolKind::Section,
            pe::IMAGE_SYM_CLASS_FILE => SymbolKind::File,
            pe::IMAGE_SYM_CLASS_LABEL => SymbolKind::Label,
            _ => SymbolKind::Unknown,
        }
    }

    fn section(&self) -> SymbolSection {
        match self.symbol.section_number.get(LE) {
            pe::IMAGE_SYM_UNDEFINED => {
                if self.symbol.storage_class == pe::IMAGE_SYM_CLASS_EXTERNAL
                    && self.symbol.value.get(LE) == 0
                {
                    SymbolSection::Undefined
                } else {
                    SymbolSection::Common
                }
            }
            pe::IMAGE_SYM_ABSOLUTE => SymbolSection::Absolute,
            pe::IMAGE_SYM_DEBUG => {
                if self.symbol.storage_class == pe::IMAGE_SYM_CLASS_FILE {
                    SymbolSection::None
                } else {
                    SymbolSection::Unknown
                }
            }
            index if index > 0 => SymbolSection::Section(SectionIndex(index as usize)),
            _ => SymbolSection::Unknown,
        }
    }

    #[inline]
    fn is_undefined(&self) -> bool {
        self.symbol.storage_class == pe::IMAGE_SYM_CLASS_EXTERNAL
            && self.symbol.section_number.get(LE) == pe::IMAGE_SYM_UNDEFINED
            && self.symbol.value.get(LE) == 0
    }

    #[inline]
    fn is_definition(&self) -> bool {
        self.symbol.is_definition()
    }

    #[inline]
    fn is_common(&self) -> bool {
        self.symbol.storage_class == pe::IMAGE_SYM_CLASS_EXTERNAL
            && self.symbol.section_number.get(LE) == pe::IMAGE_SYM_UNDEFINED
            && self.symbol.value.get(LE) != 0
    }

    #[inline]
    fn is_weak(&self) -> bool {
        self.symbol.storage_class == pe::IMAGE_SYM_CLASS_WEAK_EXTERNAL
    }

    #[inline]
    fn scope(&self) -> SymbolScope {
        match self.symbol.storage_class {
            pe::IMAGE_SYM_CLASS_EXTERNAL | pe::IMAGE_SYM_CLASS_WEAK_EXTERNAL => {
                // TODO: determine if symbol is exported
                SymbolScope::Linkage
            }
            _ => SymbolScope::Compilation,
        }
    }

    #[inline]
    fn is_global(&self) -> bool {
        match self.symbol.storage_class {
            pe::IMAGE_SYM_CLASS_EXTERNAL | pe::IMAGE_SYM_CLASS_WEAK_EXTERNAL => true,
            _ => false,
        }
    }

    #[inline]
    fn is_local(&self) -> bool {
        !self.is_global()
    }

    fn flags(&self) -> SymbolFlags<SectionIndex> {
        if self.symbol.storage_class == pe::IMAGE_SYM_CLASS_STATIC
            && self.symbol.value.get(LE) == 0
            && self.symbol.number_of_aux_symbols > 0
        {
            if let Ok(aux) = self
                .file
                .symbols
                .get::<pe::ImageAuxSymbolSection>(self.index.0 + 1)
            {
                // TODO: use high_number for bigobj
                let number = aux.number.get(LE) as usize;
                return SymbolFlags::CoffSection {
                    selection: aux.selection,
                    associative_section: if number == 0 {
                        None
                    } else {
                        Some(SectionIndex(number))
                    },
                };
            }
        }
        SymbolFlags::None
    }
}
