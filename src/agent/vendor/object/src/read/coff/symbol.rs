use alloc::fmt;
use core::convert::TryInto;
use core::str;

use crate::endian::{LittleEndian as LE, U32Bytes};
use crate::pe;
use crate::pod::{Bytes, Pod};
use crate::read::util::StringTable;
use crate::read::{
    ReadError, Result, SectionIndex, Symbol, SymbolFlags, SymbolIndex, SymbolKind, SymbolScope,
    SymbolSection,
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
    pub fn symbol(&self, index: usize) -> Option<&'data pe::ImageSymbol> {
        self.get::<pe::ImageSymbol>(index)
    }

    /// Return the symbol table entry or auxiliary record at the given index.
    pub fn get<T: Pod>(&self, index: usize) -> Option<&'data T> {
        let bytes = self.symbols.get(index)?;
        Bytes(&bytes.0[..]).read().ok()
    }
}

impl pe::ImageSymbol {
    /// Parse a COFF symbol name.
    ///
    /// `strings` must be the string table used for symbols names.
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
}

/// An iterator over the symbols of a `CoffFile`.
pub struct CoffSymbolIterator<'data, 'file>
where
    'data: 'file,
{
    pub(crate) symbols: &'file SymbolTable<'data>,
    pub(crate) index: usize,
}

impl<'data, 'file> fmt::Debug for CoffSymbolIterator<'data, 'file> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoffSymbolIterator").finish()
    }
}

impl<'data, 'file> Iterator for CoffSymbolIterator<'data, 'file> {
    type Item = (SymbolIndex, Symbol<'data>);

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index;
        let symbol = self.symbols.get::<pe::ImageSymbol>(index)?;
        self.index += 1 + symbol.number_of_aux_symbols as usize;
        Some((
            SymbolIndex(index),
            parse_symbol(self.symbols, index, symbol),
        ))
    }
}

pub(crate) fn parse_symbol<'data>(
    symbols: &SymbolTable<'data>,
    index: usize,
    symbol: &'data pe::ImageSymbol,
) -> Symbol<'data> {
    let value = symbol.value.get(LE);
    let section_number = symbol.section_number.get(LE);

    let name = if symbol.storage_class == pe::IMAGE_SYM_CLASS_FILE {
        // The file name is in the following auxiliary symbol.
        if symbol.number_of_aux_symbols > 0 {
            symbols.symbols.get(index + 1).map(|s| {
                // The name is padded with nulls.
                match s.0.iter().position(|&x| x == 0) {
                    Some(end) => &s.0[..end],
                    None => &s.0[..],
                }
            })
        } else {
            None
        }
    } else {
        symbol.name(symbols.strings()).ok()
    };
    let name = name.and_then(|s| str::from_utf8(s).ok());

    let derived_kind = if symbol.derived_type() == pe::IMAGE_SYM_DTYPE_FUNCTION {
        SymbolKind::Text
    } else {
        SymbolKind::Data
    };
    let mut flags = SymbolFlags::None;
    // FIXME: symbol.value is a section offset for non-absolute symbols, not an address
    let (kind, address, size) = match symbol.storage_class {
        pe::IMAGE_SYM_CLASS_STATIC => {
            if value == 0 && symbol.number_of_aux_symbols > 0 {
                let mut size = 0;
                if let Some(aux) = symbols.get::<pe::ImageAuxSymbolSection>(index + 1) {
                    size = u64::from(aux.length.get(LE));
                    // TODO: use high_number for bigobj
                    let number = aux.number.get(LE) as usize;
                    flags = SymbolFlags::CoffSection {
                        selection: aux.selection,
                        associative_section: if number == 0 {
                            None
                        } else {
                            Some(SectionIndex(number))
                        },
                    };
                }
                (SymbolKind::Section, 0, size)
            } else {
                (derived_kind, u64::from(value), 0)
            }
        }
        pe::IMAGE_SYM_CLASS_EXTERNAL => {
            if section_number == pe::IMAGE_SYM_UNDEFINED {
                // Common data: symbol.value is the size.
                (derived_kind, 0, u64::from(value))
            } else if symbol.derived_type() == pe::IMAGE_SYM_DTYPE_FUNCTION
                && symbol.number_of_aux_symbols > 0
            {
                let mut size = 0;
                if let Some(aux) = symbols.get::<pe::ImageAuxSymbolFunction>(index + 1) {
                    size = u64::from(aux.total_size.get(LE));
                }
                (derived_kind, u64::from(value), size)
            } else {
                (derived_kind, u64::from(value), 0)
            }
        }
        pe::IMAGE_SYM_CLASS_WEAK_EXTERNAL => (derived_kind, u64::from(value), 0),
        pe::IMAGE_SYM_CLASS_SECTION => (SymbolKind::Section, 0, 0),
        pe::IMAGE_SYM_CLASS_FILE => (SymbolKind::File, 0, 0),
        pe::IMAGE_SYM_CLASS_LABEL => (SymbolKind::Label, u64::from(value), 0),
        _ => {
            // No address because symbol.value could mean anything.
            (SymbolKind::Unknown, 0, 0)
        }
    };
    let section = match section_number {
        pe::IMAGE_SYM_UNDEFINED => {
            if symbol.storage_class == pe::IMAGE_SYM_CLASS_EXTERNAL && value != 0 {
                SymbolSection::Common
            } else {
                SymbolSection::Undefined
            }
        }
        pe::IMAGE_SYM_ABSOLUTE => SymbolSection::Absolute,
        pe::IMAGE_SYM_DEBUG => {
            if symbol.storage_class == pe::IMAGE_SYM_CLASS_FILE {
                SymbolSection::None
            } else {
                SymbolSection::Unknown
            }
        }
        index if index > 0 => SymbolSection::Section(SectionIndex(index as usize)),
        _ => SymbolSection::Unknown,
    };
    let weak = symbol.storage_class == pe::IMAGE_SYM_CLASS_WEAK_EXTERNAL;
    let scope = match symbol.storage_class {
        _ if section == SymbolSection::Undefined => SymbolScope::Unknown,
        pe::IMAGE_SYM_CLASS_EXTERNAL | pe::IMAGE_SYM_CLASS_WEAK_EXTERNAL => {
            // TODO: determine if symbol is exported
            SymbolScope::Linkage
        }
        _ => SymbolScope::Compilation,
    };
    Symbol {
        name,
        address,
        size,
        kind,
        section,
        weak,
        scope,
        flags,
    }
}
