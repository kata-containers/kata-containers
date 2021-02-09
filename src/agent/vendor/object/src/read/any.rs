use alloc::fmt;

#[cfg(feature = "coff")]
use crate::read::coff;
#[cfg(feature = "elf")]
use crate::read::elf;
#[cfg(feature = "macho")]
use crate::read::macho;
#[cfg(feature = "pe")]
use crate::read::pe;
#[cfg(feature = "wasm")]
use crate::read::wasm;
use crate::read::{
    self, Architecture, BinaryFormat, ComdatKind, CompressedData, Error, FileFlags, Object,
    ObjectComdat, ObjectMap, ObjectSection, ObjectSegment, ObjectSymbol, ObjectSymbolTable,
    Relocation, Result, SectionFlags, SectionIndex, SectionKind, SymbolFlags, SymbolIndex,
    SymbolKind, SymbolMap, SymbolMapName, SymbolScope, SymbolSection,
};

/// Evaluate an expression on the contents of a file format enum.
///
/// This is a hack to avoid virtual calls.
macro_rules! with_inner {
    ($inner:expr, $enum:ident, | $var:ident | $body:expr) => {
        match $inner {
            #[cfg(feature = "coff")]
            $enum::Coff(ref $var) => $body,
            #[cfg(feature = "elf")]
            $enum::Elf32(ref $var) => $body,
            #[cfg(feature = "elf")]
            $enum::Elf64(ref $var) => $body,
            #[cfg(feature = "macho")]
            $enum::MachO32(ref $var) => $body,
            #[cfg(feature = "macho")]
            $enum::MachO64(ref $var) => $body,
            #[cfg(feature = "pe")]
            $enum::Pe32(ref $var) => $body,
            #[cfg(feature = "pe")]
            $enum::Pe64(ref $var) => $body,
            #[cfg(feature = "wasm")]
            $enum::Wasm(ref $var) => $body,
        }
    };
}

macro_rules! with_inner_mut {
    ($inner:expr, $enum:ident, | $var:ident | $body:expr) => {
        match $inner {
            #[cfg(feature = "coff")]
            $enum::Coff(ref mut $var) => $body,
            #[cfg(feature = "elf")]
            $enum::Elf32(ref mut $var) => $body,
            #[cfg(feature = "elf")]
            $enum::Elf64(ref mut $var) => $body,
            #[cfg(feature = "macho")]
            $enum::MachO32(ref mut $var) => $body,
            #[cfg(feature = "macho")]
            $enum::MachO64(ref mut $var) => $body,
            #[cfg(feature = "pe")]
            $enum::Pe32(ref mut $var) => $body,
            #[cfg(feature = "pe")]
            $enum::Pe64(ref mut $var) => $body,
            #[cfg(feature = "wasm")]
            $enum::Wasm(ref mut $var) => $body,
        }
    };
}

/// Like `with_inner!`, but wraps the result in another enum.
macro_rules! map_inner {
    ($inner:expr, $from:ident, $to:ident, | $var:ident | $body:expr) => {
        match $inner {
            #[cfg(feature = "coff")]
            $from::Coff(ref $var) => $to::Coff($body),
            #[cfg(feature = "elf")]
            $from::Elf32(ref $var) => $to::Elf32($body),
            #[cfg(feature = "elf")]
            $from::Elf64(ref $var) => $to::Elf64($body),
            #[cfg(feature = "macho")]
            $from::MachO32(ref $var) => $to::MachO32($body),
            #[cfg(feature = "macho")]
            $from::MachO64(ref $var) => $to::MachO64($body),
            #[cfg(feature = "pe")]
            $from::Pe32(ref $var) => $to::Pe32($body),
            #[cfg(feature = "pe")]
            $from::Pe64(ref $var) => $to::Pe64($body),
            #[cfg(feature = "wasm")]
            $from::Wasm(ref $var) => $to::Wasm($body),
        }
    };
}

/// Like `map_inner!`, but the result is a Result or Option.
macro_rules! map_inner_option {
    ($inner:expr, $from:ident, $to:ident, | $var:ident | $body:expr) => {
        match $inner {
            #[cfg(feature = "coff")]
            $from::Coff(ref $var) => $body.map($to::Coff),
            #[cfg(feature = "elf")]
            $from::Elf32(ref $var) => $body.map($to::Elf32),
            #[cfg(feature = "elf")]
            $from::Elf64(ref $var) => $body.map($to::Elf64),
            #[cfg(feature = "macho")]
            $from::MachO32(ref $var) => $body.map($to::MachO32),
            #[cfg(feature = "macho")]
            $from::MachO64(ref $var) => $body.map($to::MachO64),
            #[cfg(feature = "pe")]
            $from::Pe32(ref $var) => $body.map($to::Pe32),
            #[cfg(feature = "pe")]
            $from::Pe64(ref $var) => $body.map($to::Pe64),
            #[cfg(feature = "wasm")]
            $from::Wasm(ref $var) => $body.map($to::Wasm),
        }
    };
}

/// Call `next` for a file format iterator.
macro_rules! next_inner {
    ($inner:expr, $from:ident, $to:ident) => {
        match $inner {
            #[cfg(feature = "coff")]
            $from::Coff(ref mut iter) => iter.next().map($to::Coff),
            #[cfg(feature = "elf")]
            $from::Elf32(ref mut iter) => iter.next().map($to::Elf32),
            #[cfg(feature = "elf")]
            $from::Elf64(ref mut iter) => iter.next().map($to::Elf64),
            #[cfg(feature = "macho")]
            $from::MachO32(ref mut iter) => iter.next().map($to::MachO32),
            #[cfg(feature = "macho")]
            $from::MachO64(ref mut iter) => iter.next().map($to::MachO64),
            #[cfg(feature = "pe")]
            $from::Pe32(ref mut iter) => iter.next().map($to::Pe32),
            #[cfg(feature = "pe")]
            $from::Pe64(ref mut iter) => iter.next().map($to::Pe64),
            #[cfg(feature = "wasm")]
            $from::Wasm(ref mut iter) => iter.next().map($to::Wasm),
        }
    };
}

/// An object file.
///
/// Most functionality is provided by the `Object` trait implementation.
#[derive(Debug)]
pub struct File<'data> {
    inner: FileInternal<'data>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
enum FileInternal<'data> {
    #[cfg(feature = "coff")]
    Coff(coff::CoffFile<'data>),
    #[cfg(feature = "elf")]
    Elf32(elf::ElfFile32<'data>),
    #[cfg(feature = "elf")]
    Elf64(elf::ElfFile64<'data>),
    #[cfg(feature = "macho")]
    MachO32(macho::MachOFile32<'data>),
    #[cfg(feature = "macho")]
    MachO64(macho::MachOFile64<'data>),
    #[cfg(feature = "pe")]
    Pe32(pe::PeFile32<'data>),
    #[cfg(feature = "pe")]
    Pe64(pe::PeFile64<'data>),
    #[cfg(feature = "wasm")]
    Wasm(wasm::WasmFile<'data>),
}

impl<'data> File<'data> {
    /// Parse the raw file data.
    pub fn parse(data: &'data [u8]) -> Result<Self> {
        if data.len() < 16 {
            return Err(Error("File too short"));
        }

        let inner = match [data[0], data[1], data[2], data[3], data[4]] {
            // 32-bit ELF
            #[cfg(feature = "elf")]
            [0x7f, b'E', b'L', b'F', 1] => FileInternal::Elf32(elf::ElfFile32::parse(data)?),
            // 64-bit ELF
            #[cfg(feature = "elf")]
            [0x7f, b'E', b'L', b'F', 2] => FileInternal::Elf64(elf::ElfFile64::parse(data)?),
            // 32-bit Mach-O
            #[cfg(feature = "macho")]
            [0xfe, 0xed, 0xfa, 0xce, _]
            | [0xce, 0xfa, 0xed, 0xfe, _] => FileInternal::MachO32(macho::MachOFile32::parse(data)?),
            // 64-bit Mach-O
            #[cfg(feature = "macho")]
            | [0xfe, 0xed, 0xfa, 0xcf, _]
            | [0xcf, 0xfa, 0xed, 0xfe, _] => FileInternal::MachO64(macho::MachOFile64::parse(data)?),
            // WASM
            #[cfg(feature = "wasm")]
            [0x00, b'a', b's', b'm', _] => FileInternal::Wasm(wasm::WasmFile::parse(data)?),
            // MS-DOS, assume stub for Windows PE32 or PE32+
            #[cfg(feature = "pe")]
            [b'M', b'Z', _, _, _] => {
                // `optional_header_magic` doesn't care if it's `PeFile32` and `PeFile64`.
                match pe::PeFile64::optional_header_magic(data) {
                    Ok(crate::pe::IMAGE_NT_OPTIONAL_HDR32_MAGIC) => {
                        FileInternal::Pe32(pe::PeFile32::parse(data)?)
                    }
                    Ok(crate::pe::IMAGE_NT_OPTIONAL_HDR64_MAGIC) => {
                        FileInternal::Pe64(pe::PeFile64::parse(data)?)
                    }
                    _ => return Err(Error("Unknown MS-DOS file")),
                }
            }
            // TODO: more COFF machines
            #[cfg(feature = "coff")]
            // COFF x86
            [0x4c, 0x01, _, _, _]
            // COFF x86-64
            | [0x64, 0x86, _, _, _] => FileInternal::Coff(coff::CoffFile::parse(data)?),
            _ => return Err(Error("Unknown file magic")),
        };
        Ok(File { inner })
    }

    /// Return the file format.
    pub fn format(&self) -> BinaryFormat {
        match self.inner {
            #[cfg(feature = "coff")]
            FileInternal::Coff(_) => BinaryFormat::Coff,
            #[cfg(feature = "elf")]
            FileInternal::Elf32(_) | FileInternal::Elf64(_) => BinaryFormat::Elf,
            #[cfg(feature = "macho")]
            FileInternal::MachO32(_) | FileInternal::MachO64(_) => BinaryFormat::MachO,
            #[cfg(feature = "pe")]
            FileInternal::Pe32(_) | FileInternal::Pe64(_) => BinaryFormat::Pe,
            #[cfg(feature = "wasm")]
            FileInternal::Wasm(_) => BinaryFormat::Wasm,
        }
    }
}

impl<'data> read::private::Sealed for File<'data> {}

impl<'data, 'file> Object<'data, 'file> for File<'data>
where
    'data: 'file,
{
    type Segment = Segment<'data, 'file>;
    type SegmentIterator = SegmentIterator<'data, 'file>;
    type Section = Section<'data, 'file>;
    type SectionIterator = SectionIterator<'data, 'file>;
    type Comdat = Comdat<'data, 'file>;
    type ComdatIterator = ComdatIterator<'data, 'file>;
    type Symbol = Symbol<'data, 'file>;
    type SymbolIterator = SymbolIterator<'data, 'file>;
    type SymbolTable = SymbolTable<'data, 'file>;

    fn architecture(&self) -> Architecture {
        with_inner!(self.inner, FileInternal, |x| x.architecture())
    }

    fn is_little_endian(&self) -> bool {
        with_inner!(self.inner, FileInternal, |x| x.is_little_endian())
    }

    fn is_64(&self) -> bool {
        with_inner!(self.inner, FileInternal, |x| x.is_64())
    }

    fn segments(&'file self) -> SegmentIterator<'data, 'file> {
        SegmentIterator {
            inner: map_inner!(self.inner, FileInternal, SegmentIteratorInternal, |x| x
                .segments()),
        }
    }

    fn section_by_name(&'file self, section_name: &str) -> Option<Section<'data, 'file>> {
        map_inner_option!(self.inner, FileInternal, SectionInternal, |x| x
            .section_by_name(section_name))
        .map(|inner| Section { inner })
    }

    fn section_by_index(&'file self, index: SectionIndex) -> Result<Section<'data, 'file>> {
        map_inner_option!(self.inner, FileInternal, SectionInternal, |x| x
            .section_by_index(index))
        .map(|inner| Section { inner })
    }

    fn sections(&'file self) -> SectionIterator<'data, 'file> {
        SectionIterator {
            inner: map_inner!(self.inner, FileInternal, SectionIteratorInternal, |x| x
                .sections()),
        }
    }

    fn comdats(&'file self) -> ComdatIterator<'data, 'file> {
        ComdatIterator {
            inner: map_inner!(self.inner, FileInternal, ComdatIteratorInternal, |x| x
                .comdats()),
        }
    }

    fn symbol_by_index(&'file self, index: SymbolIndex) -> Result<Symbol<'data, 'file>> {
        map_inner_option!(self.inner, FileInternal, SymbolInternal, |x| x
            .symbol_by_index(index))
        .map(|inner| Symbol { inner })
    }

    fn symbols(&'file self) -> SymbolIterator<'data, 'file> {
        SymbolIterator {
            inner: map_inner!(self.inner, FileInternal, SymbolIteratorInternal, |x| x
                .symbols()),
        }
    }

    fn symbol_table(&'file self) -> Option<SymbolTable<'data, 'file>> {
        map_inner_option!(self.inner, FileInternal, SymbolTableInternal, |x| x
            .symbol_table())
        .map(|inner| SymbolTable { inner })
    }

    fn dynamic_symbols(&'file self) -> SymbolIterator<'data, 'file> {
        SymbolIterator {
            inner: map_inner!(self.inner, FileInternal, SymbolIteratorInternal, |x| x
                .dynamic_symbols()),
        }
    }

    fn dynamic_symbol_table(&'file self) -> Option<SymbolTable<'data, 'file>> {
        map_inner_option!(self.inner, FileInternal, SymbolTableInternal, |x| x
            .dynamic_symbol_table())
        .map(|inner| SymbolTable { inner })
    }

    fn symbol_map(&self) -> SymbolMap<SymbolMapName<'data>> {
        with_inner!(self.inner, FileInternal, |x| x.symbol_map())
    }

    fn object_map(&self) -> ObjectMap<'data> {
        with_inner!(self.inner, FileInternal, |x| x.object_map())
    }

    fn has_debug_symbols(&self) -> bool {
        with_inner!(self.inner, FileInternal, |x| x.has_debug_symbols())
    }

    #[inline]
    fn mach_uuid(&self) -> Result<Option<[u8; 16]>> {
        with_inner!(self.inner, FileInternal, |x| x.mach_uuid())
    }

    #[inline]
    fn build_id(&self) -> Result<Option<&'data [u8]>> {
        with_inner!(self.inner, FileInternal, |x| x.build_id())
    }

    #[inline]
    fn gnu_debuglink(&self) -> Result<Option<(&'data [u8], u32)>> {
        with_inner!(self.inner, FileInternal, |x| x.gnu_debuglink())
    }

    fn entry(&self) -> u64 {
        with_inner!(self.inner, FileInternal, |x| x.entry())
    }

    fn flags(&self) -> FileFlags {
        with_inner!(self.inner, FileInternal, |x| x.flags())
    }
}

/// An iterator over the segments of a `File`.
#[derive(Debug)]
pub struct SegmentIterator<'data, 'file>
where
    'data: 'file,
{
    inner: SegmentIteratorInternal<'data, 'file>,
}

#[derive(Debug)]
enum SegmentIteratorInternal<'data, 'file>
where
    'data: 'file,
{
    #[cfg(feature = "coff")]
    Coff(coff::CoffSegmentIterator<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf32(elf::ElfSegmentIterator32<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf64(elf::ElfSegmentIterator64<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO32(macho::MachOSegmentIterator32<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO64(macho::MachOSegmentIterator64<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe32(pe::PeSegmentIterator32<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe64(pe::PeSegmentIterator64<'data, 'file>),
    #[cfg(feature = "wasm")]
    Wasm(wasm::WasmSegmentIterator<'data, 'file>),
}

impl<'data, 'file> Iterator for SegmentIterator<'data, 'file> {
    type Item = Segment<'data, 'file>;

    fn next(&mut self) -> Option<Self::Item> {
        next_inner!(self.inner, SegmentIteratorInternal, SegmentInternal)
            .map(|inner| Segment { inner })
    }
}

/// A segment of a `File`.
pub struct Segment<'data, 'file>
where
    'data: 'file,
{
    inner: SegmentInternal<'data, 'file>,
}

#[derive(Debug)]
enum SegmentInternal<'data, 'file>
where
    'data: 'file,
{
    #[cfg(feature = "coff")]
    Coff(coff::CoffSegment<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf32(elf::ElfSegment32<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf64(elf::ElfSegment64<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO32(macho::MachOSegment32<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO64(macho::MachOSegment64<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe32(pe::PeSegment32<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe64(pe::PeSegment64<'data, 'file>),
    #[cfg(feature = "wasm")]
    Wasm(wasm::WasmSegment<'data, 'file>),
}

impl<'data, 'file> fmt::Debug for Segment<'data, 'file> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // It's painful to do much better than this
        let mut s = f.debug_struct("Segment");
        match self.name() {
            Ok(Some(ref name)) => {
                s.field("name", name);
            }
            Ok(None) => {}
            Err(_) => {
                s.field("name", &"<invalid>");
            }
        }
        s.field("address", &self.address())
            .field("size", &self.size())
            .finish()
    }
}

impl<'data, 'file> read::private::Sealed for Segment<'data, 'file> {}

impl<'data, 'file> ObjectSegment<'data> for Segment<'data, 'file> {
    fn address(&self) -> u64 {
        with_inner!(self.inner, SegmentInternal, |x| x.address())
    }

    fn size(&self) -> u64 {
        with_inner!(self.inner, SegmentInternal, |x| x.size())
    }

    fn align(&self) -> u64 {
        with_inner!(self.inner, SegmentInternal, |x| x.align())
    }

    fn file_range(&self) -> (u64, u64) {
        with_inner!(self.inner, SegmentInternal, |x| x.file_range())
    }

    fn data(&self) -> Result<&'data [u8]> {
        with_inner!(self.inner, SegmentInternal, |x| x.data())
    }

    fn data_range(&self, address: u64, size: u64) -> Result<Option<&'data [u8]>> {
        with_inner!(self.inner, SegmentInternal, |x| x.data_range(address, size))
    }

    fn name(&self) -> Result<Option<&str>> {
        with_inner!(self.inner, SegmentInternal, |x| x.name())
    }
}

/// An iterator of the sections of a `File`.
#[derive(Debug)]
pub struct SectionIterator<'data, 'file>
where
    'data: 'file,
{
    inner: SectionIteratorInternal<'data, 'file>,
}

// we wrap our enums in a struct so that they are kept private.
#[derive(Debug)]
enum SectionIteratorInternal<'data, 'file>
where
    'data: 'file,
{
    #[cfg(feature = "coff")]
    Coff(coff::CoffSectionIterator<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf32(elf::ElfSectionIterator32<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf64(elf::ElfSectionIterator64<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO32(macho::MachOSectionIterator32<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO64(macho::MachOSectionIterator64<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe32(pe::PeSectionIterator32<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe64(pe::PeSectionIterator64<'data, 'file>),
    #[cfg(feature = "wasm")]
    Wasm(wasm::WasmSectionIterator<'data, 'file>),
}

impl<'data, 'file> Iterator for SectionIterator<'data, 'file> {
    type Item = Section<'data, 'file>;

    fn next(&mut self) -> Option<Self::Item> {
        next_inner!(self.inner, SectionIteratorInternal, SectionInternal)
            .map(|inner| Section { inner })
    }
}

/// A Section of a File
pub struct Section<'data, 'file>
where
    'data: 'file,
{
    inner: SectionInternal<'data, 'file>,
}

enum SectionInternal<'data, 'file>
where
    'data: 'file,
{
    #[cfg(feature = "coff")]
    Coff(coff::CoffSection<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf32(elf::ElfSection32<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf64(elf::ElfSection64<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO32(macho::MachOSection32<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO64(macho::MachOSection64<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe32(pe::PeSection32<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe64(pe::PeSection64<'data, 'file>),
    #[cfg(feature = "wasm")]
    Wasm(wasm::WasmSection<'data, 'file>),
}

impl<'data, 'file> fmt::Debug for Section<'data, 'file> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // It's painful to do much better than this
        let mut s = f.debug_struct("Section");
        match self.segment_name() {
            Ok(Some(ref name)) => {
                s.field("segment", name);
            }
            Ok(None) => {}
            Err(_) => {
                s.field("segment", &"<invalid>");
            }
        }
        s.field("name", &self.name().unwrap_or("<invalid>"))
            .field("address", &self.address())
            .field("size", &self.size())
            .field("align", &self.align())
            .field("kind", &self.kind())
            .field("flags", &self.flags())
            .finish()
    }
}

impl<'data, 'file> read::private::Sealed for Section<'data, 'file> {}

impl<'data, 'file> ObjectSection<'data> for Section<'data, 'file> {
    type RelocationIterator = RelocationIterator<'data, 'file>;

    fn index(&self) -> SectionIndex {
        with_inner!(self.inner, SectionInternal, |x| x.index())
    }

    fn address(&self) -> u64 {
        with_inner!(self.inner, SectionInternal, |x| x.address())
    }

    fn size(&self) -> u64 {
        with_inner!(self.inner, SectionInternal, |x| x.size())
    }

    fn align(&self) -> u64 {
        with_inner!(self.inner, SectionInternal, |x| x.align())
    }

    fn file_range(&self) -> Option<(u64, u64)> {
        with_inner!(self.inner, SectionInternal, |x| x.file_range())
    }

    fn data(&self) -> Result<&'data [u8]> {
        with_inner!(self.inner, SectionInternal, |x| x.data())
    }

    fn data_range(&self, address: u64, size: u64) -> Result<Option<&'data [u8]>> {
        with_inner!(self.inner, SectionInternal, |x| x.data_range(address, size))
    }

    fn compressed_data(&self) -> Result<CompressedData<'data>> {
        with_inner!(self.inner, SectionInternal, |x| x.compressed_data())
    }

    fn name(&self) -> Result<&str> {
        with_inner!(self.inner, SectionInternal, |x| x.name())
    }

    fn segment_name(&self) -> Result<Option<&str>> {
        with_inner!(self.inner, SectionInternal, |x| x.segment_name())
    }

    fn kind(&self) -> SectionKind {
        with_inner!(self.inner, SectionInternal, |x| x.kind())
    }

    fn relocations(&self) -> RelocationIterator<'data, 'file> {
        RelocationIterator {
            inner: map_inner!(
                self.inner,
                SectionInternal,
                RelocationIteratorInternal,
                |x| x.relocations()
            ),
        }
    }

    fn flags(&self) -> SectionFlags {
        with_inner!(self.inner, SectionInternal, |x| x.flags())
    }
}

/// An iterator of the COMDAT section groups of a `File`.
#[derive(Debug)]
pub struct ComdatIterator<'data, 'file>
where
    'data: 'file,
{
    inner: ComdatIteratorInternal<'data, 'file>,
}

#[derive(Debug)]
enum ComdatIteratorInternal<'data, 'file>
where
    'data: 'file,
{
    #[cfg(feature = "coff")]
    Coff(coff::CoffComdatIterator<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf32(elf::ElfComdatIterator32<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf64(elf::ElfComdatIterator64<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO32(macho::MachOComdatIterator32<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO64(macho::MachOComdatIterator64<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe32(pe::PeComdatIterator32<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe64(pe::PeComdatIterator64<'data, 'file>),
    #[cfg(feature = "wasm")]
    Wasm(wasm::WasmComdatIterator<'data, 'file>),
}

impl<'data, 'file> Iterator for ComdatIterator<'data, 'file> {
    type Item = Comdat<'data, 'file>;

    fn next(&mut self) -> Option<Self::Item> {
        next_inner!(self.inner, ComdatIteratorInternal, ComdatInternal)
            .map(|inner| Comdat { inner })
    }
}

/// A COMDAT section group of a `File`.
pub struct Comdat<'data, 'file>
where
    'data: 'file,
{
    inner: ComdatInternal<'data, 'file>,
}

enum ComdatInternal<'data, 'file>
where
    'data: 'file,
{
    #[cfg(feature = "coff")]
    Coff(coff::CoffComdat<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf32(elf::ElfComdat32<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf64(elf::ElfComdat64<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO32(macho::MachOComdat32<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO64(macho::MachOComdat64<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe32(pe::PeComdat32<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe64(pe::PeComdat64<'data, 'file>),
    #[cfg(feature = "wasm")]
    Wasm(wasm::WasmComdat<'data, 'file>),
}

impl<'data, 'file> fmt::Debug for Comdat<'data, 'file> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("Comdat");
        s.field("symbol", &self.symbol())
            .field("name", &self.name().unwrap_or("<invalid>"))
            .field("kind", &self.kind())
            .finish()
    }
}

impl<'data, 'file> read::private::Sealed for Comdat<'data, 'file> {}

impl<'data, 'file> ObjectComdat<'data> for Comdat<'data, 'file> {
    type SectionIterator = ComdatSectionIterator<'data, 'file>;

    fn kind(&self) -> ComdatKind {
        with_inner!(self.inner, ComdatInternal, |x| x.kind())
    }

    fn symbol(&self) -> SymbolIndex {
        with_inner!(self.inner, ComdatInternal, |x| x.symbol())
    }

    fn name(&self) -> Result<&str> {
        with_inner!(self.inner, ComdatInternal, |x| x.name())
    }

    fn sections(&self) -> ComdatSectionIterator<'data, 'file> {
        ComdatSectionIterator {
            inner: map_inner!(
                self.inner,
                ComdatInternal,
                ComdatSectionIteratorInternal,
                |x| x.sections()
            ),
        }
    }
}

/// An iterator over COMDAT section entries.
#[derive(Debug)]
pub struct ComdatSectionIterator<'data, 'file>
where
    'data: 'file,
{
    inner: ComdatSectionIteratorInternal<'data, 'file>,
}

#[derive(Debug)]
enum ComdatSectionIteratorInternal<'data, 'file>
where
    'data: 'file,
{
    #[cfg(feature = "coff")]
    Coff(coff::CoffComdatSectionIterator<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf32(elf::ElfComdatSectionIterator32<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf64(elf::ElfComdatSectionIterator64<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO32(macho::MachOComdatSectionIterator32<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO64(macho::MachOComdatSectionIterator64<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe32(pe::PeComdatSectionIterator32<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe64(pe::PeComdatSectionIterator64<'data, 'file>),
    #[cfg(feature = "wasm")]
    Wasm(wasm::WasmComdatSectionIterator<'data, 'file>),
}

impl<'data, 'file> Iterator for ComdatSectionIterator<'data, 'file> {
    type Item = SectionIndex;

    fn next(&mut self) -> Option<Self::Item> {
        with_inner_mut!(self.inner, ComdatSectionIteratorInternal, |x| x.next())
    }
}

/// A symbol table.
#[derive(Debug)]
pub struct SymbolTable<'data, 'file>
where
    'data: 'file,
{
    inner: SymbolTableInternal<'data, 'file>,
}

#[derive(Debug)]
enum SymbolTableInternal<'data, 'file>
where
    'data: 'file,
{
    #[cfg(feature = "coff")]
    Coff(coff::CoffSymbolTable<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf32(elf::ElfSymbolTable32<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf64(elf::ElfSymbolTable64<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO32(macho::MachOSymbolTable32<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO64(macho::MachOSymbolTable64<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe32(coff::CoffSymbolTable<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe64(coff::CoffSymbolTable<'data, 'file>),
    #[cfg(feature = "wasm")]
    Wasm(wasm::WasmSymbolTable<'data, 'file>),
}

impl<'data, 'file> read::private::Sealed for SymbolTable<'data, 'file> {}

impl<'data, 'file> ObjectSymbolTable<'data> for SymbolTable<'data, 'file> {
    type Symbol = Symbol<'data, 'file>;
    type SymbolIterator = SymbolIterator<'data, 'file>;

    fn symbols(&self) -> Self::SymbolIterator {
        SymbolIterator {
            inner: map_inner!(
                self.inner,
                SymbolTableInternal,
                SymbolIteratorInternal,
                |x| x.symbols()
            ),
        }
    }

    fn symbol_by_index(&self, index: SymbolIndex) -> Result<Self::Symbol> {
        map_inner_option!(self.inner, SymbolTableInternal, SymbolInternal, |x| x
            .symbol_by_index(index))
        .map(|inner| Symbol { inner })
    }
}

/// An iterator over symbol table entries.
#[derive(Debug)]
pub struct SymbolIterator<'data, 'file>
where
    'data: 'file,
{
    inner: SymbolIteratorInternal<'data, 'file>,
}

#[derive(Debug)]
enum SymbolIteratorInternal<'data, 'file>
where
    'data: 'file,
{
    #[cfg(feature = "coff")]
    Coff(coff::CoffSymbolIterator<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf32(elf::ElfSymbolIterator32<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf64(elf::ElfSymbolIterator64<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO32(macho::MachOSymbolIterator32<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO64(macho::MachOSymbolIterator64<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe32(coff::CoffSymbolIterator<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe64(coff::CoffSymbolIterator<'data, 'file>),
    #[cfg(feature = "wasm")]
    Wasm(wasm::WasmSymbolIterator<'data, 'file>),
}

impl<'data, 'file> Iterator for SymbolIterator<'data, 'file> {
    type Item = Symbol<'data, 'file>;

    fn next(&mut self) -> Option<Self::Item> {
        next_inner!(self.inner, SymbolIteratorInternal, SymbolInternal)
            .map(|inner| Symbol { inner })
    }
}

/// A symbol table entry.
pub struct Symbol<'data, 'file>
where
    'data: 'file,
{
    inner: SymbolInternal<'data, 'file>,
}

enum SymbolInternal<'data, 'file>
where
    'data: 'file,
{
    #[cfg(feature = "coff")]
    Coff(coff::CoffSymbol<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf32(elf::ElfSymbol32<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf64(elf::ElfSymbol64<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO32(macho::MachOSymbol32<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO64(macho::MachOSymbol64<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe32(coff::CoffSymbol<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe64(coff::CoffSymbol<'data, 'file>),
    #[cfg(feature = "wasm")]
    Wasm(wasm::WasmSymbol<'data, 'file>),
}

impl<'data, 'file> fmt::Debug for Symbol<'data, 'file> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Symbol")
            .field("name", &self.name().unwrap_or("<invalid>"))
            .field("address", &self.address())
            .field("size", &self.size())
            .field("kind", &self.kind())
            .field("section", &self.section())
            .field("scope", &self.scope())
            .field("weak", &self.is_weak())
            .field("flags", &self.flags())
            .finish()
    }
}

impl<'data, 'file> read::private::Sealed for Symbol<'data, 'file> {}

impl<'data, 'file> ObjectSymbol<'data> for Symbol<'data, 'file> {
    fn index(&self) -> SymbolIndex {
        with_inner!(self.inner, SymbolInternal, |x| x.index())
    }

    fn name(&self) -> Result<&'data str> {
        with_inner!(self.inner, SymbolInternal, |x| x.name())
    }

    fn address(&self) -> u64 {
        with_inner!(self.inner, SymbolInternal, |x| x.address())
    }

    fn size(&self) -> u64 {
        with_inner!(self.inner, SymbolInternal, |x| x.size())
    }

    fn kind(&self) -> SymbolKind {
        with_inner!(self.inner, SymbolInternal, |x| x.kind())
    }

    fn section(&self) -> SymbolSection {
        with_inner!(self.inner, SymbolInternal, |x| x.section())
    }

    fn is_undefined(&self) -> bool {
        with_inner!(self.inner, SymbolInternal, |x| x.is_undefined())
    }

    fn is_definition(&self) -> bool {
        with_inner!(self.inner, SymbolInternal, |x| x.is_definition())
    }

    fn is_common(&self) -> bool {
        with_inner!(self.inner, SymbolInternal, |x| x.is_common())
    }

    fn is_weak(&self) -> bool {
        with_inner!(self.inner, SymbolInternal, |x| x.is_weak())
    }

    fn scope(&self) -> SymbolScope {
        with_inner!(self.inner, SymbolInternal, |x| x.scope())
    }

    fn is_global(&self) -> bool {
        with_inner!(self.inner, SymbolInternal, |x| x.is_global())
    }

    fn is_local(&self) -> bool {
        with_inner!(self.inner, SymbolInternal, |x| x.is_local())
    }

    fn flags(&self) -> SymbolFlags<SectionIndex> {
        with_inner!(self.inner, SymbolInternal, |x| x.flags())
    }
}

/// An iterator over relocation entries
#[derive(Debug)]
pub struct RelocationIterator<'data, 'file>
where
    'data: 'file,
{
    inner: RelocationIteratorInternal<'data, 'file>,
}

#[derive(Debug)]
enum RelocationIteratorInternal<'data, 'file>
where
    'data: 'file,
{
    #[cfg(feature = "coff")]
    Coff(coff::CoffRelocationIterator<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf32(elf::ElfRelocationIterator32<'data, 'file>),
    #[cfg(feature = "elf")]
    Elf64(elf::ElfRelocationIterator64<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO32(macho::MachORelocationIterator32<'data, 'file>),
    #[cfg(feature = "macho")]
    MachO64(macho::MachORelocationIterator64<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe32(pe::PeRelocationIterator<'data, 'file>),
    #[cfg(feature = "pe")]
    Pe64(pe::PeRelocationIterator<'data, 'file>),
    #[cfg(feature = "wasm")]
    Wasm(wasm::WasmRelocationIterator<'data, 'file>),
}

impl<'data, 'file> Iterator for RelocationIterator<'data, 'file> {
    type Item = (u64, Relocation);

    fn next(&mut self) -> Option<Self::Item> {
        with_inner_mut!(self.inner, RelocationIteratorInternal, |x| x.next())
    }
}
