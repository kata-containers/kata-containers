use alloc::vec::Vec;
use core::fmt::Debug;
use core::{mem, str};

use crate::read::{
    self, Architecture, ComdatKind, Error, FileFlags, Object, ObjectComdat, ObjectSection,
    ReadError, Result, SectionIndex, Symbol, SymbolFlags, SymbolIndex, SymbolKind, SymbolMap,
    SymbolScope, SymbolSection,
};
use crate::{endian, macho, BigEndian, Bytes, Endian, Endianness, Pod};

use super::{
    parse_symbol, MachOLoadCommandIterator, MachOSection, MachOSectionInternal,
    MachOSectionIterator, MachOSegment, MachOSegmentIterator, MachOSymbolIterator, Nlist, Section,
    Segment, SymbolTable,
};

/// A 32-bit Mach-O object file.
pub type MachOFile32<'data, Endian = Endianness> = MachOFile<'data, macho::MachHeader32<Endian>>;
/// A 64-bit Mach-O object file.
pub type MachOFile64<'data, Endian = Endianness> = MachOFile<'data, macho::MachHeader64<Endian>>;

/// A partially parsed Mach-O file.
///
/// Most of the functionality of this type is provided by the `Object` trait implementation.
#[derive(Debug)]
pub struct MachOFile<'data, Mach: MachHeader> {
    pub(super) endian: Mach::Endian,
    pub(super) data: Bytes<'data>,
    pub(super) header: &'data Mach,
    pub(super) sections: Vec<MachOSectionInternal<'data, Mach>>,
    pub(super) symbols: SymbolTable<'data, Mach>,
}

impl<'data, Mach: MachHeader> MachOFile<'data, Mach> {
    /// Parse the raw Mach-O file data.
    pub fn parse(data: &'data [u8]) -> Result<Self> {
        let data = Bytes(data);
        let header = Mach::parse(data)?;
        let endian = header.endian()?;

        let mut symbols = SymbolTable::default();
        // Build a list of sections to make some operations more efficient.
        let mut sections = Vec::new();
        if let Ok(mut commands) = header.load_commands(endian, data) {
            while let Ok(Some(command)) = commands.next() {
                if let Some((segment, section_data)) = Mach::Segment::from_command(command)? {
                    for section in segment.sections(endian, section_data)? {
                        let index = SectionIndex(sections.len() + 1);
                        sections.push(MachOSectionInternal::parse(index, section));
                    }
                } else if let Some(symtab) = command.symtab()? {
                    symbols = symtab.symbols(endian, data)?;
                }
            }
        }

        Ok(MachOFile {
            endian,
            header,
            sections,
            symbols,
            data,
        })
    }

    /// Return the section at the given index.
    #[inline]
    pub(super) fn section_internal(
        &self,
        index: SectionIndex,
    ) -> Result<&MachOSectionInternal<'data, Mach>> {
        index
            .0
            .checked_sub(1)
            .and_then(|index| self.sections.get(index))
            .read_error("Invalid Mach-O section index")
    }
}

impl<'data, Mach: MachHeader> read::private::Sealed for MachOFile<'data, Mach> {}

impl<'data, 'file, Mach> Object<'data, 'file> for MachOFile<'data, Mach>
where
    'data: 'file,
    Mach: MachHeader,
{
    type Segment = MachOSegment<'data, 'file, Mach>;
    type SegmentIterator = MachOSegmentIterator<'data, 'file, Mach>;
    type Section = MachOSection<'data, 'file, Mach>;
    type SectionIterator = MachOSectionIterator<'data, 'file, Mach>;
    type Comdat = MachOComdat<'data, 'file, Mach>;
    type ComdatIterator = MachOComdatIterator<'data, 'file, Mach>;
    type SymbolIterator = MachOSymbolIterator<'data, 'file, Mach>;

    fn architecture(&self) -> Architecture {
        match self.header.cputype(self.endian) {
            macho::CPU_TYPE_ARM => Architecture::Arm,
            macho::CPU_TYPE_ARM64 => Architecture::Aarch64,
            macho::CPU_TYPE_X86 => Architecture::I386,
            macho::CPU_TYPE_X86_64 => Architecture::X86_64,
            macho::CPU_TYPE_MIPS => Architecture::Mips,
            _ => Architecture::Unknown,
        }
    }

    #[inline]
    fn is_little_endian(&self) -> bool {
        self.header.is_little_endian()
    }

    #[inline]
    fn is_64(&self) -> bool {
        self.header.is_type_64()
    }

    fn segments(&'file self) -> MachOSegmentIterator<'data, 'file, Mach> {
        MachOSegmentIterator {
            file: self,
            commands: self
                .header
                .load_commands(self.endian, self.data)
                .ok()
                .unwrap_or_else(Default::default),
        }
    }

    fn section_by_name(
        &'file self,
        section_name: &str,
    ) -> Option<MachOSection<'data, 'file, Mach>> {
        // Translate the "." prefix to the "__" prefix used by OSX/Mach-O, eg
        // ".debug_info" to "__debug_info".
        let system_section = section_name.starts_with('.');
        let cmp_section_name = |section: &MachOSection<Mach>| {
            section
                .name()
                .map(|name| {
                    section_name == name
                        || (system_section
                            && name.starts_with("__")
                            && section_name[1..] == name[2..])
                })
                .unwrap_or(false)
        };

        self.sections().find(cmp_section_name)
    }

    fn section_by_index(
        &'file self,
        index: SectionIndex,
    ) -> Result<MachOSection<'data, 'file, Mach>> {
        let internal = *self.section_internal(index)?;
        Ok(MachOSection {
            file: self,
            internal,
        })
    }

    fn sections(&'file self) -> MachOSectionIterator<'data, 'file, Mach> {
        MachOSectionIterator {
            file: self,
            iter: self.sections.iter(),
        }
    }

    fn comdats(&'file self) -> MachOComdatIterator<'data, 'file, Mach> {
        MachOComdatIterator { file: self }
    }

    fn symbol_by_index(&self, index: SymbolIndex) -> Result<Symbol<'data>> {
        let nlist = self.symbols.symbol(index.0)?;
        parse_symbol(self, nlist, self.symbols.strings())
            .read_error("Unsupported Mach-O symbol index")
    }

    fn symbols(&'file self) -> MachOSymbolIterator<'data, 'file, Mach> {
        MachOSymbolIterator {
            file: self,
            symbols: self.symbols,
            index: 0,
        }
    }

    fn dynamic_symbols(&'file self) -> MachOSymbolIterator<'data, 'file, Mach> {
        // The LC_DYSYMTAB command contains indices into the same symbol
        // table as the LC_SYMTAB command, so return all of them.
        self.symbols()
    }

    fn symbol_map(&self) -> SymbolMap<'data> {
        let mut symbols: Vec<_> = self.symbols().map(|(_, s)| s).collect();

        // Add symbols for the end of each section.
        for section in self.sections() {
            symbols.push(Symbol {
                name: None,
                address: section.address() + section.size(),
                size: 0,
                kind: SymbolKind::Section,
                section: SymbolSection::Undefined,
                weak: false,
                scope: SymbolScope::Compilation,
                flags: SymbolFlags::None,
            });
        }

        // Calculate symbol sizes by sorting and finding the next symbol.
        symbols.sort_by(|a, b| {
            a.address.cmp(&b.address).then_with(|| {
                // Place the end of section symbols last.
                (a.kind == SymbolKind::Section).cmp(&(b.kind == SymbolKind::Section))
            })
        });

        for i in 0..symbols.len() {
            let (before, after) = symbols.split_at_mut(i + 1);
            let symbol = &mut before[i];
            if symbol.kind != SymbolKind::Section {
                if let Some(next) = after
                    .iter()
                    .skip_while(|x| x.kind != SymbolKind::Section && x.address == symbol.address)
                    .next()
                {
                    symbol.size = next.address - symbol.address;
                }
            }
        }

        symbols.retain(SymbolMap::filter);
        SymbolMap { symbols }
    }

    fn has_debug_symbols(&self) -> bool {
        self.section_by_name(".debug_info").is_some()
    }

    fn mach_uuid(&self) -> Result<Option<[u8; 16]>> {
        self.header.uuid(self.endian, self.data)
    }

    fn entry(&self) -> u64 {
        if let Ok(mut commands) = self.header.load_commands(self.endian, self.data) {
            while let Ok(Some(command)) = commands.next() {
                if let Ok(Some(command)) = command.entry_point() {
                    return command.entryoff.get(self.endian);
                }
            }
        }
        0
    }

    fn flags(&self) -> FileFlags {
        FileFlags::MachO {
            flags: self.header.flags(self.endian),
        }
    }
}

/// An iterator over the COMDAT section groups of a `MachOFile64`.
pub type MachOComdatIterator32<'data, 'file, Endian = Endianness> =
    MachOComdatIterator<'data, 'file, macho::MachHeader32<Endian>>;
/// An iterator over the COMDAT section groups of a `MachOFile64`.
pub type MachOComdatIterator64<'data, 'file, Endian = Endianness> =
    MachOComdatIterator<'data, 'file, macho::MachHeader64<Endian>>;

/// An iterator over the COMDAT section groups of a `MachOFile`.
#[derive(Debug)]
pub struct MachOComdatIterator<'data, 'file, Mach: MachHeader> {
    file: &'file MachOFile<'data, Mach>,
}

impl<'data, 'file, Mach: MachHeader> Iterator for MachOComdatIterator<'data, 'file, Mach> {
    type Item = MachOComdat<'data, 'file, Mach>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

/// A COMDAT section group of a `MachOFile32`.
pub type MachOComdat32<'data, 'file, Endian = Endianness> =
    MachOComdat<'data, 'file, macho::MachHeader32<Endian>>;

/// A COMDAT section group of a `MachOFile64`.
pub type MachOComdat64<'data, 'file, Endian = Endianness> =
    MachOComdat<'data, 'file, macho::MachHeader64<Endian>>;

/// A COMDAT section group of a `MachOFile`.
#[derive(Debug)]
pub struct MachOComdat<'data, 'file, Mach: MachHeader> {
    file: &'file MachOFile<'data, Mach>,
}

impl<'data, 'file, Mach: MachHeader> read::private::Sealed for MachOComdat<'data, 'file, Mach> {}

impl<'data, 'file, Mach: MachHeader> ObjectComdat<'data> for MachOComdat<'data, 'file, Mach> {
    type SectionIterator = MachOComdatSectionIterator<'data, 'file, Mach>;

    #[inline]
    fn kind(&self) -> ComdatKind {
        unreachable!();
    }

    #[inline]
    fn symbol(&self) -> SymbolIndex {
        unreachable!();
    }

    #[inline]
    fn name(&self) -> Result<&str> {
        unreachable!();
    }

    #[inline]
    fn sections(&self) -> Self::SectionIterator {
        unreachable!();
    }
}

/// An iterator over the sections in a COMDAT section group of a `MachOFile32`.
pub type MachOComdatSectionIterator32<'data, 'file, Endian = Endianness> =
    MachOComdatSectionIterator<'data, 'file, macho::MachHeader32<Endian>>;
/// An iterator over the sections in a COMDAT section group of a `MachOFile64`.
pub type MachOComdatSectionIterator64<'data, 'file, Endian = Endianness> =
    MachOComdatSectionIterator<'data, 'file, macho::MachHeader64<Endian>>;

/// An iterator over the sections in a COMDAT section group of a `MachOFile`.
#[derive(Debug)]
pub struct MachOComdatSectionIterator<'data, 'file, Mach: MachHeader>
where
    'data: 'file,
{
    file: &'file MachOFile<'data, Mach>,
}

impl<'data, 'file, Mach: MachHeader> Iterator for MachOComdatSectionIterator<'data, 'file, Mach> {
    type Item = SectionIndex;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

/// A trait for generic access to `MachHeader32` and `MachHeader64`.
#[allow(missing_docs)]
pub trait MachHeader: Debug + Pod {
    type Word: Into<u64>;
    type Endian: endian::Endian;
    type Segment: Segment<Endian = Self::Endian, Section = Self::Section>;
    type Section: Section<Endian = Self::Endian>;
    type Nlist: Nlist<Endian = Self::Endian>;

    /// Return true if this type is a 64-bit header.
    ///
    /// This is a property of the type, not a value in the header data.
    fn is_type_64(&self) -> bool;

    /// Return true if the `magic` field signifies big-endian.
    fn is_big_endian(&self) -> bool;

    /// Return true if the `magic` field signifies little-endian.
    fn is_little_endian(&self) -> bool;

    fn magic(&self) -> u32;
    fn cputype(&self, endian: Self::Endian) -> u32;
    fn cpusubtype(&self, endian: Self::Endian) -> u32;
    fn filetype(&self, endian: Self::Endian) -> u32;
    fn ncmds(&self, endian: Self::Endian) -> u32;
    fn sizeofcmds(&self, endian: Self::Endian) -> u32;
    fn flags(&self, endian: Self::Endian) -> u32;

    // Provided methods.

    /// Read the file header.
    ///
    /// Also checks that the magic field in the file header is a supported format.
    fn parse<'data>(mut data: Bytes<'data>) -> read::Result<&'data Self> {
        let header = data
            .read::<Self>()
            .read_error("Invalid Mach-O header size or alignment")?;
        if !header.is_supported() {
            return Err(Error("Unsupported Mach-O header"));
        }
        Ok(header)
    }

    fn is_supported(&self) -> bool {
        self.is_little_endian() || self.is_big_endian()
    }

    fn endian(&self) -> Result<Self::Endian> {
        Self::Endian::from_big_endian(self.is_big_endian()).read_error("Unsupported Mach-O endian")
    }

    fn load_commands<'data>(
        &self,
        endian: Self::Endian,
        data: Bytes<'data>,
    ) -> Result<MachOLoadCommandIterator<'data, Self::Endian>> {
        let data = data
            .read_bytes_at(mem::size_of::<Self>(), self.sizeofcmds(endian) as usize)
            .read_error("Invalid Mach-O load command table size")?;
        Ok(MachOLoadCommandIterator::new(
            endian,
            data,
            self.ncmds(endian),
        ))
    }

    /// Return the UUID from the `LC_UUID` load command, if one is present.
    fn uuid<'data>(&self, endian: Self::Endian, data: Bytes<'data>) -> Result<Option<[u8; 16]>> {
        let mut commands = self.load_commands(endian, data)?;
        while let Some(command) = commands.next()? {
            if let Ok(Some(uuid)) = command.uuid() {
                return Ok(Some(uuid.uuid));
            }
        }
        Ok(None)
    }
}

impl<Endian: endian::Endian> MachHeader for macho::MachHeader32<Endian> {
    type Word = u32;
    type Endian = Endian;
    type Segment = macho::SegmentCommand32<Endian>;
    type Section = macho::Section32<Endian>;
    type Nlist = macho::Nlist32<Endian>;

    fn is_type_64(&self) -> bool {
        false
    }

    fn is_big_endian(&self) -> bool {
        self.magic() == macho::MH_MAGIC
    }

    fn is_little_endian(&self) -> bool {
        self.magic() == macho::MH_CIGAM
    }

    fn magic(&self) -> u32 {
        self.magic.get(BigEndian)
    }

    fn cputype(&self, endian: Self::Endian) -> u32 {
        self.cputype.get(endian)
    }

    fn cpusubtype(&self, endian: Self::Endian) -> u32 {
        self.cpusubtype.get(endian)
    }

    fn filetype(&self, endian: Self::Endian) -> u32 {
        self.filetype.get(endian)
    }

    fn ncmds(&self, endian: Self::Endian) -> u32 {
        self.ncmds.get(endian)
    }

    fn sizeofcmds(&self, endian: Self::Endian) -> u32 {
        self.sizeofcmds.get(endian)
    }

    fn flags(&self, endian: Self::Endian) -> u32 {
        self.flags.get(endian)
    }
}

impl<Endian: endian::Endian> MachHeader for macho::MachHeader64<Endian> {
    type Word = u64;
    type Endian = Endian;
    type Segment = macho::SegmentCommand64<Endian>;
    type Section = macho::Section64<Endian>;
    type Nlist = macho::Nlist64<Endian>;

    fn is_type_64(&self) -> bool {
        true
    }

    fn is_big_endian(&self) -> bool {
        self.magic() == macho::MH_MAGIC_64
    }

    fn is_little_endian(&self) -> bool {
        self.magic() == macho::MH_CIGAM_64
    }

    fn magic(&self) -> u32 {
        self.magic.get(BigEndian)
    }

    fn cputype(&self, endian: Self::Endian) -> u32 {
        self.cputype.get(endian)
    }

    fn cpusubtype(&self, endian: Self::Endian) -> u32 {
        self.cpusubtype.get(endian)
    }

    fn filetype(&self, endian: Self::Endian) -> u32 {
        self.filetype.get(endian)
    }

    fn ncmds(&self, endian: Self::Endian) -> u32 {
        self.ncmds.get(endian)
    }

    fn sizeofcmds(&self, endian: Self::Endian) -> u32 {
        self.sizeofcmds.get(endian)
    }

    fn flags(&self, endian: Self::Endian) -> u32 {
        self.flags.get(endian)
    }
}
