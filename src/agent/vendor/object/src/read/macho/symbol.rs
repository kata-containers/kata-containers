use core::fmt::Debug;
use core::{fmt, slice, str};

use crate::endian::{self, Endianness};
use crate::macho;
use crate::pod::Pod;
use crate::read::util::StringTable;
use crate::read::{
    ReadError, Result, SectionIndex, SectionKind, Symbol, SymbolFlags, SymbolIndex, SymbolKind,
    SymbolScope, SymbolSection,
};

use super::{MachHeader, MachOFile};

/// A table of symbol entries in a Mach-O file.
///
/// Also includes the string table used for the symbol names.
#[derive(Debug, Clone, Copy)]
pub struct SymbolTable<'data, Mach: MachHeader> {
    symbols: &'data [Mach::Nlist],
    strings: StringTable<'data>,
}

impl<'data, Mach: MachHeader> Default for SymbolTable<'data, Mach> {
    fn default() -> Self {
        SymbolTable {
            symbols: &[],
            strings: Default::default(),
        }
    }
}

impl<'data, Mach: MachHeader> SymbolTable<'data, Mach> {
    #[inline]
    pub(super) fn new(symbols: &'data [Mach::Nlist], strings: StringTable<'data>) -> Self {
        SymbolTable { symbols, strings }
    }

    /// Return the string table used for the symbol names.
    #[inline]
    pub fn strings(&self) -> StringTable<'data> {
        self.strings
    }

    /// Iterate over the symbols.
    #[inline]
    pub fn iter(&self) -> slice::Iter<'data, Mach::Nlist> {
        self.symbols.iter()
    }

    /// Return true if the symbol table is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// Return the symbol at the given index.
    pub fn symbol(&self, index: usize) -> Result<&'data Mach::Nlist> {
        self.symbols
            .get(index)
            .read_error("Invalid Mach-O symbol index")
    }
}

/// An iterator over the symbols of a `MachOFile32`.
pub type MachOSymbolIterator32<'data, 'file, Endian = Endianness> =
    MachOSymbolIterator<'data, 'file, macho::MachHeader32<Endian>>;
/// An iterator over the symbols of a `MachOFile64`.
pub type MachOSymbolIterator64<'data, 'file, Endian = Endianness> =
    MachOSymbolIterator<'data, 'file, macho::MachHeader64<Endian>>;

/// An iterator over the symbols of a `MachOFile`.
pub struct MachOSymbolIterator<'data, 'file, Mach: MachHeader> {
    pub(super) file: &'file MachOFile<'data, Mach>,
    pub(super) symbols: SymbolTable<'data, Mach>,
    pub(super) index: usize,
}

impl<'data, 'file, Mach: MachHeader> fmt::Debug for MachOSymbolIterator<'data, 'file, Mach> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MachOSymbolIterator").finish()
    }
}

impl<'data, 'file, Mach: MachHeader> Iterator for MachOSymbolIterator<'data, 'file, Mach> {
    type Item = (SymbolIndex, Symbol<'data>);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let index = self.index;
            let nlist = self.symbols.symbols.get(index)?;
            self.index += 1;
            if let Some(symbol) = parse_symbol(self.file, nlist, self.symbols.strings) {
                return Some((SymbolIndex(index), symbol));
            }
        }
    }
}

pub(super) fn parse_symbol<'data, Mach: MachHeader>(
    file: &MachOFile<'data, Mach>,
    nlist: &Mach::Nlist,
    strings: StringTable<'data>,
) -> Option<Symbol<'data>> {
    let endian = file.endian;
    let name = nlist
        .name(endian, strings)
        .ok()
        .and_then(|s| str::from_utf8(s).ok());
    let n_type = nlist.n_type();
    let n_desc = nlist.n_desc(endian);
    if n_type & macho::N_STAB != 0 {
        return None;
    }
    let section = match n_type & macho::N_TYPE {
        macho::N_UNDF => SymbolSection::Undefined,
        macho::N_ABS => SymbolSection::Absolute,
        macho::N_SECT => {
            let n_sect = nlist.n_sect();
            if n_sect != 0 {
                SymbolSection::Section(SectionIndex(n_sect as usize))
            } else {
                SymbolSection::Unknown
            }
        }
        _ => SymbolSection::Unknown,
    };
    let kind = section
        .index()
        .and_then(|index| file.section_internal(index).ok())
        .map(|section| match section.kind {
            SectionKind::Text => SymbolKind::Text,
            SectionKind::Data
            | SectionKind::ReadOnlyData
            | SectionKind::ReadOnlyString
            | SectionKind::UninitializedData
            | SectionKind::Common => SymbolKind::Data,
            SectionKind::Tls | SectionKind::UninitializedTls | SectionKind::TlsVariables => {
                SymbolKind::Tls
            }
            _ => SymbolKind::Unknown,
        })
        .unwrap_or(SymbolKind::Unknown);
    let weak = n_desc & (macho::N_WEAK_REF | macho::N_WEAK_DEF) != 0;
    let scope = if section == SymbolSection::Undefined {
        SymbolScope::Unknown
    } else if n_type & macho::N_EXT == 0 {
        SymbolScope::Compilation
    } else if n_type & macho::N_PEXT != 0 {
        SymbolScope::Linkage
    } else {
        SymbolScope::Dynamic
    };
    let flags = SymbolFlags::MachO { n_desc };
    Some(Symbol {
        name,
        address: nlist.n_value(endian).into(),
        // Only calculated for symbol maps
        size: 0,
        kind,
        section,
        weak,
        scope,
        flags,
    })
}

/// A trait for generic access to `Nlist32` and `Nlist64`.
#[allow(missing_docs)]
pub trait Nlist: Debug + Pod {
    type Word: Into<u64>;
    type Endian: endian::Endian;

    fn n_strx(&self, endian: Self::Endian) -> u32;
    fn n_type(&self) -> u8;
    fn n_sect(&self) -> u8;
    fn n_desc(&self, endian: Self::Endian) -> u16;
    fn n_value(&self, endian: Self::Endian) -> Self::Word;

    fn name<'data>(
        &self,
        endian: Self::Endian,
        strings: StringTable<'data>,
    ) -> Result<&'data [u8]> {
        strings
            .get(self.n_strx(endian))
            .read_error("Invalid Mach-O symbol name offset")
    }

    fn is_undefined(&self) -> bool {
        self.n_type() & macho::N_TYPE == macho::N_UNDF
    }
}

impl<Endian: endian::Endian> Nlist for macho::Nlist32<Endian> {
    type Word = u32;
    type Endian = Endian;

    fn n_strx(&self, endian: Self::Endian) -> u32 {
        self.n_strx.get(endian)
    }
    fn n_type(&self) -> u8 {
        self.n_type
    }
    fn n_sect(&self) -> u8 {
        self.n_sect
    }
    fn n_desc(&self, endian: Self::Endian) -> u16 {
        self.n_desc.get(endian)
    }
    fn n_value(&self, endian: Self::Endian) -> Self::Word {
        self.n_value.get(endian)
    }
}

impl<Endian: endian::Endian> Nlist for macho::Nlist64<Endian> {
    type Word = u64;
    type Endian = Endian;

    fn n_strx(&self, endian: Self::Endian) -> u32 {
        self.n_strx.get(endian)
    }
    fn n_type(&self) -> u8 {
        self.n_type
    }
    fn n_sect(&self) -> u8 {
        self.n_sect
    }
    fn n_desc(&self, endian: Self::Endian) -> u16 {
        self.n_desc.get(endian)
    }
    fn n_value(&self, endian: Self::Endian) -> Self::Word {
        self.n_value.get(endian)
    }
}
