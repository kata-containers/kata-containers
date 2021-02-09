use alloc::fmt;
use alloc::vec::Vec;
use core::fmt::Debug;
use core::slice;
use core::str;

use crate::elf;
use crate::endian::{self, Endianness};
use crate::pod::{Bytes, Pod};
use crate::read::util::StringTable;
use crate::read::{
    self, ObjectSymbol, ObjectSymbolTable, ReadError, SectionIndex, SymbolFlags, SymbolIndex,
    SymbolKind, SymbolMap, SymbolMapEntry, SymbolScope, SymbolSection,
};

use super::{FileHeader, SectionHeader, SectionTable};

/// A table of symbol entries in an ELF file.
///
/// Also includes the string table used for the symbol names.
#[derive(Debug, Clone, Copy)]
pub struct SymbolTable<'data, Elf: FileHeader> {
    section: usize,
    symbols: &'data [Elf::Sym],
    strings: StringTable<'data>,
    shndx: &'data [u32],
}

impl<'data, Elf: FileHeader> Default for SymbolTable<'data, Elf> {
    fn default() -> Self {
        SymbolTable {
            section: 0,
            symbols: &[],
            strings: Default::default(),
            shndx: &[],
        }
    }
}

impl<'data, Elf: FileHeader> SymbolTable<'data, Elf> {
    /// Parse the symbol table of the given section type.
    ///
    /// Returns an empty symbol table if the symbol table does not exist.
    pub fn parse(
        endian: Elf::Endian,
        data: Bytes<'data>,
        sections: &SectionTable<Elf>,
        sh_type: u32,
    ) -> read::Result<SymbolTable<'data, Elf>> {
        debug_assert!(sh_type == elf::SHT_DYNSYM || sh_type == elf::SHT_SYMTAB);

        let (section, symtab) = match sections
            .iter()
            .enumerate()
            .find(|s| s.1.sh_type(endian) == sh_type)
        {
            Some(s) => s,
            None => return Ok(SymbolTable::default()),
        };
        let symbols = symtab
            .data_as_array(endian, data)
            .read_error("Invalid ELF symbol table data")?;

        let strtab = sections.section(symtab.sh_link(endian) as usize)?;
        let strtab_data = strtab
            .data(endian, data)
            .read_error("Invalid ELF string table data")?;
        let strings = StringTable::new(strtab_data);

        let shndx = sections
            .iter()
            .find(|s| {
                s.sh_type(endian) == elf::SHT_SYMTAB_SHNDX && s.sh_link(endian) as usize == section
            })
            .map(|section| {
                section
                    .data_as_array(endian, data)
                    .read_error("Invalid ELF symtab_shndx data")
            })
            .transpose()?
            .unwrap_or(&[]);

        Ok(SymbolTable {
            section,
            symbols,
            strings,
            shndx,
        })
    }

    /// Return the section index of this symbol table.
    #[inline]
    pub fn section(&self) -> usize {
        self.section
    }

    /// Return the string table used for the symbol names.
    #[inline]
    pub fn strings(&self) -> StringTable<'data> {
        self.strings
    }

    /// Iterate over the symbols.
    #[inline]
    pub fn iter(&self) -> slice::Iter<'data, Elf::Sym> {
        self.symbols.iter()
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

    /// Return the symbol at the given index.
    pub fn symbol(&self, index: usize) -> read::Result<&'data Elf::Sym> {
        self.symbols
            .get(index)
            .read_error("Invalid ELF symbol index")
    }

    /// Return the extended section index for the given symbol if present.
    #[inline]
    pub fn shndx(&self, index: usize) -> Option<u32> {
        self.shndx.get(index).cloned()
    }

    /// Construct a map from addresses to a user-defined map entry.
    pub fn map<Entry: SymbolMapEntry, F: Fn(&'data Elf::Sym) -> Option<Entry>>(
        &self,
        endian: Elf::Endian,
        f: F,
    ) -> SymbolMap<Entry> {
        let mut symbols = Vec::with_capacity(self.symbols.len());
        for symbol in self.symbols {
            if !symbol.is_definition(endian) {
                continue;
            }
            if let Some(entry) = f(symbol) {
                symbols.push(entry);
            }
        }
        SymbolMap::new(symbols)
    }
}

/// A symbol table of an `ElfFile32`.
pub type ElfSymbolTable32<'data, 'file, Endian = Endianness> =
    ElfSymbolTable<'data, 'file, elf::FileHeader32<Endian>>;
/// A symbol table of an `ElfFile32`.
pub type ElfSymbolTable64<'data, 'file, Endian = Endianness> =
    ElfSymbolTable<'data, 'file, elf::FileHeader64<Endian>>;

/// A symbol table of an `ElfFile`.
#[derive(Debug, Clone, Copy)]
pub struct ElfSymbolTable<'data, 'file, Elf>
where
    'data: 'file,
    Elf: FileHeader,
{
    pub(super) endian: Elf::Endian,
    pub(super) symbols: &'file SymbolTable<'data, Elf>,
}

impl<'data, 'file, Elf: FileHeader> read::private::Sealed for ElfSymbolTable<'data, 'file, Elf> {}

impl<'data, 'file, Elf: FileHeader> ObjectSymbolTable<'data> for ElfSymbolTable<'data, 'file, Elf> {
    type Symbol = ElfSymbol<'data, 'file, Elf>;
    type SymbolIterator = ElfSymbolIterator<'data, 'file, Elf>;

    fn symbols(&self) -> Self::SymbolIterator {
        ElfSymbolIterator {
            endian: self.endian,
            symbols: self.symbols,
            index: 0,
        }
    }

    fn symbol_by_index(&self, index: SymbolIndex) -> read::Result<Self::Symbol> {
        let symbol = self.symbols.symbol(index.0)?;
        Ok(ElfSymbol {
            endian: self.endian,
            symbols: self.symbols,
            index,
            symbol,
        })
    }
}

/// An iterator over the symbols of an `ElfFile32`.
pub type ElfSymbolIterator32<'data, 'file, Endian = Endianness> =
    ElfSymbolIterator<'data, 'file, elf::FileHeader32<Endian>>;
/// An iterator over the symbols of an `ElfFile64`.
pub type ElfSymbolIterator64<'data, 'file, Endian = Endianness> =
    ElfSymbolIterator<'data, 'file, elf::FileHeader64<Endian>>;

/// An iterator over the symbols of an `ElfFile`.
pub struct ElfSymbolIterator<'data, 'file, Elf>
where
    'data: 'file,
    Elf: FileHeader,
{
    pub(super) endian: Elf::Endian,
    pub(super) symbols: &'file SymbolTable<'data, Elf>,
    pub(super) index: usize,
}

impl<'data, 'file, Elf: FileHeader> fmt::Debug for ElfSymbolIterator<'data, 'file, Elf> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElfSymbolIterator").finish()
    }
}

impl<'data, 'file, Elf: FileHeader> Iterator for ElfSymbolIterator<'data, 'file, Elf> {
    type Item = ElfSymbol<'data, 'file, Elf>;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index;
        let symbol = self.symbols.symbols.get(index)?;
        self.index += 1;
        Some(ElfSymbol {
            endian: self.endian,
            symbols: self.symbols,
            index: SymbolIndex(index),
            symbol,
        })
    }
}

/// A symbol of an `ElfFile32`.
pub type ElfSymbol32<'data, 'file, Endian = Endianness> =
    ElfSymbol<'data, 'file, elf::FileHeader32<Endian>>;
/// A symbol of an `ElfFile64`.
pub type ElfSymbol64<'data, 'file, Endian = Endianness> =
    ElfSymbol<'data, 'file, elf::FileHeader64<Endian>>;

/// A symbol of an `ElfFile`.
#[derive(Debug, Clone, Copy)]
pub struct ElfSymbol<'data, 'file, Elf>
where
    'data: 'file,
    Elf: FileHeader,
{
    pub(super) endian: Elf::Endian,
    pub(super) symbols: &'file SymbolTable<'data, Elf>,
    pub(super) index: SymbolIndex,
    pub(super) symbol: &'data Elf::Sym,
}

impl<'data, 'file, Elf: FileHeader> read::private::Sealed for ElfSymbol<'data, 'file, Elf> {}

impl<'data, 'file, Elf: FileHeader> ObjectSymbol<'data> for ElfSymbol<'data, 'file, Elf> {
    #[inline]
    fn index(&self) -> SymbolIndex {
        self.index
    }

    fn name(&self) -> read::Result<&'data str> {
        let name = self.symbol.name(self.endian, self.symbols.strings())?;
        str::from_utf8(name)
            .ok()
            .read_error("Non UTF-8 ELF symbol name")
    }

    #[inline]
    fn address(&self) -> u64 {
        self.symbol.st_value(self.endian).into()
    }

    #[inline]
    fn size(&self) -> u64 {
        self.symbol.st_size(self.endian).into()
    }

    fn kind(&self) -> SymbolKind {
        match self.symbol.st_type() {
            elf::STT_NOTYPE if self.index.0 == 0 => SymbolKind::Null,
            elf::STT_OBJECT | elf::STT_COMMON => SymbolKind::Data,
            elf::STT_FUNC => SymbolKind::Text,
            elf::STT_SECTION => SymbolKind::Section,
            elf::STT_FILE => SymbolKind::File,
            elf::STT_TLS => SymbolKind::Tls,
            _ => SymbolKind::Unknown,
        }
    }

    fn section(&self) -> SymbolSection {
        match self.symbol.st_shndx(self.endian) {
            elf::SHN_UNDEF => SymbolSection::Undefined,
            elf::SHN_ABS => {
                if self.symbol.st_type() == elf::STT_FILE {
                    SymbolSection::None
                } else {
                    SymbolSection::Absolute
                }
            }
            elf::SHN_COMMON => SymbolSection::Common,
            elf::SHN_XINDEX => match self.symbols.shndx(self.index.0) {
                Some(index) => SymbolSection::Section(SectionIndex(index as usize)),
                None => SymbolSection::Unknown,
            },
            index if index < elf::SHN_LORESERVE => {
                SymbolSection::Section(SectionIndex(index as usize))
            }
            _ => SymbolSection::Unknown,
        }
    }

    #[inline]
    fn is_undefined(&self) -> bool {
        self.symbol.st_shndx(self.endian) == elf::SHN_UNDEF
    }

    #[inline]
    fn is_definition(&self) -> bool {
        self.symbol.is_definition(self.endian)
    }

    #[inline]
    fn is_common(&self) -> bool {
        self.symbol.st_shndx(self.endian) == elf::SHN_COMMON
    }

    #[inline]
    fn is_weak(&self) -> bool {
        self.symbol.st_bind() == elf::STB_WEAK
    }

    fn scope(&self) -> SymbolScope {
        if self.symbol.st_shndx(self.endian) == elf::SHN_UNDEF {
            SymbolScope::Unknown
        } else {
            match self.symbol.st_bind() {
                elf::STB_LOCAL => SymbolScope::Compilation,
                elf::STB_GLOBAL | elf::STB_WEAK => {
                    if self.symbol.st_visibility() == elf::STV_HIDDEN {
                        SymbolScope::Linkage
                    } else {
                        SymbolScope::Dynamic
                    }
                }
                _ => SymbolScope::Unknown,
            }
        }
    }

    #[inline]
    fn is_global(&self) -> bool {
        self.symbol.st_bind() != elf::STB_LOCAL
    }

    #[inline]
    fn is_local(&self) -> bool {
        self.symbol.st_bind() == elf::STB_LOCAL
    }

    #[inline]
    fn flags(&self) -> SymbolFlags<SectionIndex> {
        SymbolFlags::Elf {
            st_info: self.symbol.st_info(),
            st_other: self.symbol.st_other(),
        }
    }
}

/// A trait for generic access to `Sym32` and `Sym64`.
#[allow(missing_docs)]
pub trait Sym: Debug + Pod {
    type Word: Into<u64>;
    type Endian: endian::Endian;

    fn st_name(&self, endian: Self::Endian) -> u32;
    fn st_info(&self) -> u8;
    fn st_bind(&self) -> u8;
    fn st_type(&self) -> u8;
    fn st_other(&self) -> u8;
    fn st_visibility(&self) -> u8;
    fn st_shndx(&self, endian: Self::Endian) -> u16;
    fn st_value(&self, endian: Self::Endian) -> Self::Word;
    fn st_size(&self, endian: Self::Endian) -> Self::Word;

    /// Parse the symbol name from the string table.
    fn name<'data>(
        &self,
        endian: Self::Endian,
        strings: StringTable<'data>,
    ) -> read::Result<&'data [u8]> {
        strings
            .get(self.st_name(endian))
            .read_error("Invalid ELF symbol name offset")
    }

    /// Return true if the symbol is a definition of a function or data object.
    fn is_definition(&self, endian: Self::Endian) -> bool {
        let st_type = self.st_type();
        (st_type == elf::STT_NOTYPE || st_type == elf::STT_FUNC || st_type == elf::STT_OBJECT)
            && self.st_shndx(endian) != elf::SHN_UNDEF
    }
}

impl<Endian: endian::Endian> Sym for elf::Sym32<Endian> {
    type Word = u32;
    type Endian = Endian;

    #[inline]
    fn st_name(&self, endian: Self::Endian) -> u32 {
        self.st_name.get(endian)
    }

    #[inline]
    fn st_info(&self) -> u8 {
        self.st_info
    }

    #[inline]
    fn st_bind(&self) -> u8 {
        self.st_bind()
    }

    #[inline]
    fn st_type(&self) -> u8 {
        self.st_type()
    }

    #[inline]
    fn st_other(&self) -> u8 {
        self.st_other
    }

    #[inline]
    fn st_visibility(&self) -> u8 {
        self.st_visibility()
    }

    #[inline]
    fn st_shndx(&self, endian: Self::Endian) -> u16 {
        self.st_shndx.get(endian)
    }

    #[inline]
    fn st_value(&self, endian: Self::Endian) -> Self::Word {
        self.st_value.get(endian)
    }

    #[inline]
    fn st_size(&self, endian: Self::Endian) -> Self::Word {
        self.st_size.get(endian)
    }
}

impl<Endian: endian::Endian> Sym for elf::Sym64<Endian> {
    type Word = u64;
    type Endian = Endian;

    #[inline]
    fn st_name(&self, endian: Self::Endian) -> u32 {
        self.st_name.get(endian)
    }

    #[inline]
    fn st_info(&self) -> u8 {
        self.st_info
    }

    #[inline]
    fn st_bind(&self) -> u8 {
        self.st_bind()
    }

    #[inline]
    fn st_type(&self) -> u8 {
        self.st_type()
    }

    #[inline]
    fn st_other(&self) -> u8 {
        self.st_other
    }

    #[inline]
    fn st_visibility(&self) -> u8 {
        self.st_visibility()
    }

    #[inline]
    fn st_shndx(&self, endian: Self::Endian) -> u16 {
        self.st_shndx.get(endian)
    }

    #[inline]
    fn st_value(&self, endian: Self::Endian) -> Self::Word {
        self.st_value.get(endian)
    }

    #[inline]
    fn st_size(&self, endian: Self::Endian) -> Self::Word {
        self.st_size.get(endian)
    }
}
