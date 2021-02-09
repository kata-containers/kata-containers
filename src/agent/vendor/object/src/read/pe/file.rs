use alloc::vec::Vec;
use core::fmt::Debug;
use core::{mem, str};

use crate::read::coff::{parse_symbol, CoffSymbolIterator, SymbolTable};
use crate::read::{
    self, Architecture, ComdatKind, Error, FileFlags, Object, ObjectComdat, ReadError, Result,
    SectionIndex, Symbol, SymbolIndex, SymbolMap,
};
use crate::{pe, Bytes, LittleEndian as LE, Pod};

use super::{PeSection, PeSectionIterator, PeSegment, PeSegmentIterator, SectionTable};

/// A PE32 (32-bit) image file.
pub type PeFile32<'data> = PeFile<'data, pe::ImageNtHeaders32>;
/// A PE32+ (64-bit) image file.
pub type PeFile64<'data> = PeFile<'data, pe::ImageNtHeaders64>;

/// A PE object file.
#[derive(Debug)]
pub struct PeFile<'data, Pe: ImageNtHeaders> {
    pub(super) dos_header: &'data pe::ImageDosHeader,
    pub(super) nt_headers: &'data Pe,
    pub(super) data_directories: &'data [pe::ImageDataDirectory],
    pub(super) sections: SectionTable<'data>,
    pub(super) symbols: SymbolTable<'data>,
    pub(super) data: Bytes<'data>,
}

impl<'data, Pe: ImageNtHeaders> PeFile<'data, Pe> {
    /// Find the optional header and read the `optional_header.magic`.
    pub fn optional_header_magic(data: &'data [u8]) -> Result<u16> {
        let data = Bytes(data);
        let dos_header = pe::ImageDosHeader::parse(data)?;
        // NT headers are at an offset specified in the DOS header.
        let nt_headers = data
            .read_at::<Pe>(dos_header.e_lfanew.get(LE) as usize)
            .read_error("Invalid NT headers offset, size, or alignment")?;
        if nt_headers.signature() != pe::IMAGE_NT_SIGNATURE {
            return Err(Error("Invalid PE magic"));
        }
        Ok(nt_headers.optional_header().magic())
    }

    /// Parse the raw PE file data.
    pub fn parse(data: &'data [u8]) -> Result<Self> {
        let data = Bytes(data);
        let dos_header = pe::ImageDosHeader::parse(data)?;
        let (nt_headers, data_directories, nt_tail) = dos_header.nt_headers::<Pe>(data)?;
        let sections = nt_headers.sections(nt_tail)?;
        let symbols = nt_headers.symbols(data)?;

        Ok(PeFile {
            dos_header,
            nt_headers,
            data_directories,
            sections,
            symbols,
            data,
        })
    }

    pub(super) fn section_alignment(&self) -> u64 {
        u64::from(self.nt_headers.optional_header().section_alignment())
    }
}

impl<'data, Pe: ImageNtHeaders> read::private::Sealed for PeFile<'data, Pe> {}

impl<'data, 'file, Pe> Object<'data, 'file> for PeFile<'data, Pe>
where
    'data: 'file,
    Pe: ImageNtHeaders,
{
    type Segment = PeSegment<'data, 'file, Pe>;
    type SegmentIterator = PeSegmentIterator<'data, 'file, Pe>;
    type Section = PeSection<'data, 'file, Pe>;
    type SectionIterator = PeSectionIterator<'data, 'file, Pe>;
    type Comdat = PeComdat<'data, 'file, Pe>;
    type ComdatIterator = PeComdatIterator<'data, 'file, Pe>;
    type SymbolIterator = CoffSymbolIterator<'data, 'file>;

    fn architecture(&self) -> Architecture {
        match self.nt_headers.file_header().machine.get(LE) {
            // TODO: Arm/Arm64
            pe::IMAGE_FILE_MACHINE_I386 => Architecture::I386,
            pe::IMAGE_FILE_MACHINE_AMD64 => Architecture::X86_64,
            _ => Architecture::Unknown,
        }
    }

    #[inline]
    fn is_little_endian(&self) -> bool {
        // Only little endian is supported.
        true
    }

    #[inline]
    fn is_64(&self) -> bool {
        self.nt_headers.is_type_64()
    }

    fn segments(&'file self) -> PeSegmentIterator<'data, 'file, Pe> {
        PeSegmentIterator {
            file: self,
            iter: self.sections.iter(),
        }
    }

    fn section_by_name(&'file self, section_name: &str) -> Option<PeSection<'data, 'file, Pe>> {
        self.sections
            .section_by_name(self.symbols.strings(), section_name.as_bytes())
            .map(|(index, section)| PeSection {
                file: self,
                index: SectionIndex(index),
                section,
            })
    }

    fn section_by_index(&'file self, index: SectionIndex) -> Result<PeSection<'data, 'file, Pe>> {
        let section = self.sections.section(index.0)?;
        Ok(PeSection {
            file: self,
            index,
            section,
        })
    }

    fn sections(&'file self) -> PeSectionIterator<'data, 'file, Pe> {
        PeSectionIterator {
            file: self,
            iter: self.sections.iter().enumerate(),
        }
    }

    fn comdats(&'file self) -> PeComdatIterator<'data, 'file, Pe> {
        PeComdatIterator { file: self }
    }

    fn symbol_by_index(&self, index: SymbolIndex) -> Result<Symbol<'data>> {
        let symbol = self
            .symbols
            .get(index.0)
            .read_error("Invalid PE symbol index")?;
        Ok(parse_symbol(&self.symbols, index.0, symbol))
    }

    fn symbols(&'file self) -> CoffSymbolIterator<'data, 'file> {
        CoffSymbolIterator {
            symbols: &self.symbols,
            index: 0,
        }
    }

    fn dynamic_symbols(&'file self) -> CoffSymbolIterator<'data, 'file> {
        // TODO: return exports/imports
        CoffSymbolIterator {
            symbols: &self.symbols,
            // Hack: don't return any.
            index: self.symbols.len(),
        }
    }

    fn symbol_map(&self) -> SymbolMap<'data> {
        // TODO: untested
        let mut symbols: Vec<_> = self
            .symbols()
            .map(|(_, s)| s)
            .filter(SymbolMap::filter)
            .collect();
        symbols.sort_by_key(|x| x.address);
        SymbolMap { symbols }
    }

    fn has_debug_symbols(&self) -> bool {
        self.section_by_name(".debug_info").is_some()
    }

    fn entry(&self) -> u64 {
        u64::from(self.nt_headers.optional_header().address_of_entry_point())
    }

    fn flags(&self) -> FileFlags {
        FileFlags::Coff {
            characteristics: self.nt_headers.file_header().characteristics.get(LE),
        }
    }
}

/// An iterator over the COMDAT section groups of a `PeFile32`.
pub type PeComdatIterator32<'data, 'file> = PeComdatIterator<'data, 'file, pe::ImageNtHeaders32>;
/// An iterator over the COMDAT section groups of a `PeFile64`.
pub type PeComdatIterator64<'data, 'file> = PeComdatIterator<'data, 'file, pe::ImageNtHeaders64>;

/// An iterator over the COMDAT section groups of a `PeFile`.
#[derive(Debug)]
pub struct PeComdatIterator<'data, 'file, Pe: ImageNtHeaders> {
    file: &'file PeFile<'data, Pe>,
}

impl<'data, 'file, Pe: ImageNtHeaders> Iterator for PeComdatIterator<'data, 'file, Pe> {
    type Item = PeComdat<'data, 'file, Pe>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

/// A COMDAT section group of a `PeFile32`.
pub type PeComdat32<'data, 'file> = PeComdat<'data, 'file, pe::ImageNtHeaders32>;
/// A COMDAT section group of a `PeFile64`.
pub type PeComdat64<'data, 'file> = PeComdat<'data, 'file, pe::ImageNtHeaders64>;

/// A COMDAT section group of a `PeFile`.
#[derive(Debug)]
pub struct PeComdat<'data, 'file, Pe: ImageNtHeaders> {
    file: &'file PeFile<'data, Pe>,
}

impl<'data, 'file, Pe: ImageNtHeaders> read::private::Sealed for PeComdat<'data, 'file, Pe> {}

impl<'data, 'file, Pe: ImageNtHeaders> ObjectComdat<'data> for PeComdat<'data, 'file, Pe> {
    type SectionIterator = PeComdatSectionIterator<'data, 'file, Pe>;

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

/// An iterator over the sections in a COMDAT section group of a `PeFile32`.
pub type PeComdatSectionIterator32<'data, 'file> =
    PeComdatSectionIterator<'data, 'file, pe::ImageNtHeaders32>;
/// An iterator over the sections in a COMDAT section group of a `PeFile64`.
pub type PeComdatSectionIterator64<'data, 'file> =
    PeComdatSectionIterator<'data, 'file, pe::ImageNtHeaders64>;

/// An iterator over the sections in a COMDAT section group of a `PeFile`.
#[derive(Debug)]
pub struct PeComdatSectionIterator<'data, 'file, Pe: ImageNtHeaders>
where
    'data: 'file,
{
    file: &'file PeFile<'data, Pe>,
}

impl<'data, 'file, Pe: ImageNtHeaders> Iterator for PeComdatSectionIterator<'data, 'file, Pe> {
    type Item = SectionIndex;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

impl pe::ImageDosHeader {
    /// Read the DOS header.
    ///
    /// Also checks that the `e_magic` field in the header is valid.
    pub fn parse<'data>(data: Bytes<'data>) -> read::Result<&'data Self> {
        // DOS header comes first.
        let dos_header = data
            .read_at::<pe::ImageDosHeader>(0)
            .read_error("Invalid DOS header size or alignment")?;
        if dos_header.e_magic.get(LE) != pe::IMAGE_DOS_SIGNATURE {
            return Err(Error("Invalid DOS magic"));
        }
        Ok(dos_header)
    }

    /// Read the NT headers, including the data directories.
    ///
    /// The given data must be for the entire file.  Returns the data following the NT headers,
    /// which will contain the section headers.
    ///
    /// Also checks that the `signature` and `magic` fields in the headers are valid.
    #[inline]
    pub fn nt_headers<'data, Pe: ImageNtHeaders>(
        &self,
        data: Bytes<'data>,
    ) -> read::Result<(&'data Pe, &'data [pe::ImageDataDirectory], Bytes<'data>)> {
        Pe::parse(self, data)
    }
}

/// A trait for generic access to `ImageNtHeaders32` and `ImageNtHeaders64`.
#[allow(missing_docs)]
pub trait ImageNtHeaders: Debug + Pod {
    type ImageOptionalHeader: ImageOptionalHeader;

    /// Return true if this type is a 64-bit header.
    ///
    /// This is a property of the type, not a value in the header data.
    fn is_type_64(&self) -> bool;

    /// Return true if the magic field in the optional header is valid.
    fn is_valid_optional_magic(&self) -> bool;

    /// Return the signature
    fn signature(&self) -> u32;

    /// Return the file header.
    fn file_header(&self) -> &pe::ImageFileHeader;

    /// Return the optional header.
    fn optional_header(&self) -> &Self::ImageOptionalHeader;

    // Provided methods.

    /// Read the NT headers, including the data directories.
    ///
    /// The DOS header is required to determine the NT headers offset.
    ///
    /// The given data must be for the entire file.  Returns the data following the NT headers,
    /// which will contain the section headers.
    ///
    /// Also checks that the `signature` and `magic` fields in the headers are valid.
    fn parse<'data>(
        dos_header: &pe::ImageDosHeader,
        mut data: Bytes<'data>,
    ) -> read::Result<(&'data Self, &'data [pe::ImageDataDirectory], Bytes<'data>)> {
        data.skip(dos_header.e_lfanew.get(LE) as usize)
            .read_error("Invalid PE headers offset")?;
        // Note that this does not include the data directories in the optional header.
        let nt_headers = data
            .read::<Self>()
            .read_error("Invalid PE headers size or alignment")?;
        if nt_headers.signature() != pe::IMAGE_NT_SIGNATURE {
            return Err(Error("Invalid PE magic"));
        }
        if !nt_headers.is_valid_optional_magic() {
            return Err(Error("Invalid PE optional header magic"));
        }

        // Read the rest of the optional header, and then read the data directories from that.
        let optional_data_size = (nt_headers.file_header().size_of_optional_header.get(LE)
            as usize)
            .checked_sub(mem::size_of::<Self::ImageOptionalHeader>())
            .read_error("PE optional header size is too small")?;
        let mut optional_data = data
            .read_bytes(optional_data_size)
            .read_error("Invalid PE optional header size")?;
        let data_directories = optional_data
            .read_slice(nt_headers.optional_header().number_of_rva_and_sizes() as usize)
            .read_error("Invalid PE number of RVA and sizes")?;

        Ok((nt_headers, data_directories, data))
    }

    /// Read the section table.
    ///
    /// `nt_tail` must be the data following the NT headers.
    #[inline]
    fn sections<'data>(&self, nt_tail: Bytes<'data>) -> read::Result<SectionTable<'data>> {
        SectionTable::parse(self.file_header(), nt_tail)
    }

    /// Read the symbol table and string table.
    ///
    /// `data` must be the entire file data.
    #[inline]
    fn symbols<'data>(&self, data: Bytes<'data>) -> read::Result<SymbolTable<'data>> {
        SymbolTable::parse(self.file_header(), data)
    }
}

/// A trait for generic access to `ImageOptionalHeader32` and `ImageOptionalHeader64`.
#[allow(missing_docs)]
pub trait ImageOptionalHeader: Debug + Pod {
    // Standard fields.
    fn magic(&self) -> u16;
    fn major_linker_version(&self) -> u8;
    fn minor_linker_version(&self) -> u8;
    fn size_of_code(&self) -> u32;
    fn size_of_initialized_data(&self) -> u32;
    fn size_of_uninitialized_data(&self) -> u32;
    fn address_of_entry_point(&self) -> u32;
    fn base_of_code(&self) -> u32;

    // NT additional fields.
    fn image_base(&self) -> u64;
    fn section_alignment(&self) -> u32;
    fn file_alignment(&self) -> u32;
    fn major_operating_system_version(&self) -> u16;
    fn minor_operating_system_version(&self) -> u16;
    fn major_image_version(&self) -> u16;
    fn minor_image_version(&self) -> u16;
    fn major_subsystem_version(&self) -> u16;
    fn minor_subsystem_version(&self) -> u16;
    fn win32_version_value(&self) -> u32;
    fn size_of_image(&self) -> u32;
    fn size_of_headers(&self) -> u32;
    fn check_sum(&self) -> u32;
    fn subsystem(&self) -> u16;
    fn dll_characteristics(&self) -> u16;
    fn size_of_stack_reserve(&self) -> u64;
    fn size_of_stack_commit(&self) -> u64;
    fn size_of_heap_reserve(&self) -> u64;
    fn size_of_heap_commit(&self) -> u64;
    fn loader_flags(&self) -> u32;
    fn number_of_rva_and_sizes(&self) -> u32;
}

impl ImageNtHeaders for pe::ImageNtHeaders32 {
    type ImageOptionalHeader = pe::ImageOptionalHeader32;

    #[inline]
    fn is_type_64(&self) -> bool {
        false
    }

    #[inline]
    fn is_valid_optional_magic(&self) -> bool {
        self.optional_header.magic.get(LE) == pe::IMAGE_NT_OPTIONAL_HDR32_MAGIC
    }

    #[inline]
    fn signature(&self) -> u32 {
        self.signature.get(LE)
    }

    #[inline]
    fn file_header(&self) -> &pe::ImageFileHeader {
        &self.file_header
    }

    #[inline]
    fn optional_header(&self) -> &Self::ImageOptionalHeader {
        &self.optional_header
    }
}

impl ImageOptionalHeader for pe::ImageOptionalHeader32 {
    #[inline]
    fn magic(&self) -> u16 {
        self.magic.get(LE)
    }

    #[inline]
    fn major_linker_version(&self) -> u8 {
        self.major_linker_version
    }

    #[inline]
    fn minor_linker_version(&self) -> u8 {
        self.minor_linker_version
    }

    #[inline]
    fn size_of_code(&self) -> u32 {
        self.size_of_code.get(LE)
    }

    #[inline]
    fn size_of_initialized_data(&self) -> u32 {
        self.size_of_initialized_data.get(LE)
    }

    #[inline]
    fn size_of_uninitialized_data(&self) -> u32 {
        self.size_of_uninitialized_data.get(LE)
    }

    #[inline]
    fn address_of_entry_point(&self) -> u32 {
        self.address_of_entry_point.get(LE)
    }

    #[inline]
    fn base_of_code(&self) -> u32 {
        self.base_of_code.get(LE)
    }

    #[inline]
    fn image_base(&self) -> u64 {
        self.image_base.get(LE).into()
    }

    #[inline]
    fn section_alignment(&self) -> u32 {
        self.section_alignment.get(LE)
    }

    #[inline]
    fn file_alignment(&self) -> u32 {
        self.file_alignment.get(LE)
    }

    #[inline]
    fn major_operating_system_version(&self) -> u16 {
        self.major_operating_system_version.get(LE)
    }

    #[inline]
    fn minor_operating_system_version(&self) -> u16 {
        self.minor_operating_system_version.get(LE)
    }

    #[inline]
    fn major_image_version(&self) -> u16 {
        self.major_image_version.get(LE)
    }

    #[inline]
    fn minor_image_version(&self) -> u16 {
        self.minor_image_version.get(LE)
    }

    #[inline]
    fn major_subsystem_version(&self) -> u16 {
        self.major_subsystem_version.get(LE)
    }

    #[inline]
    fn minor_subsystem_version(&self) -> u16 {
        self.minor_subsystem_version.get(LE)
    }

    #[inline]
    fn win32_version_value(&self) -> u32 {
        self.win32_version_value.get(LE)
    }

    #[inline]
    fn size_of_image(&self) -> u32 {
        self.size_of_image.get(LE)
    }

    #[inline]
    fn size_of_headers(&self) -> u32 {
        self.size_of_headers.get(LE)
    }

    #[inline]
    fn check_sum(&self) -> u32 {
        self.check_sum.get(LE)
    }

    #[inline]
    fn subsystem(&self) -> u16 {
        self.subsystem.get(LE)
    }

    #[inline]
    fn dll_characteristics(&self) -> u16 {
        self.dll_characteristics.get(LE)
    }

    #[inline]
    fn size_of_stack_reserve(&self) -> u64 {
        self.size_of_stack_reserve.get(LE).into()
    }

    #[inline]
    fn size_of_stack_commit(&self) -> u64 {
        self.size_of_stack_commit.get(LE).into()
    }

    #[inline]
    fn size_of_heap_reserve(&self) -> u64 {
        self.size_of_heap_reserve.get(LE).into()
    }

    #[inline]
    fn size_of_heap_commit(&self) -> u64 {
        self.size_of_heap_commit.get(LE).into()
    }

    #[inline]
    fn loader_flags(&self) -> u32 {
        self.loader_flags.get(LE)
    }

    #[inline]
    fn number_of_rva_and_sizes(&self) -> u32 {
        self.number_of_rva_and_sizes.get(LE)
    }
}

impl ImageNtHeaders for pe::ImageNtHeaders64 {
    type ImageOptionalHeader = pe::ImageOptionalHeader64;

    #[inline]
    fn is_type_64(&self) -> bool {
        true
    }

    #[inline]
    fn is_valid_optional_magic(&self) -> bool {
        self.optional_header.magic.get(LE) == pe::IMAGE_NT_OPTIONAL_HDR64_MAGIC
    }

    #[inline]
    fn signature(&self) -> u32 {
        self.signature.get(LE)
    }

    #[inline]
    fn file_header(&self) -> &pe::ImageFileHeader {
        &self.file_header
    }

    #[inline]
    fn optional_header(&self) -> &Self::ImageOptionalHeader {
        &self.optional_header
    }
}

impl ImageOptionalHeader for pe::ImageOptionalHeader64 {
    #[inline]
    fn magic(&self) -> u16 {
        self.magic.get(LE)
    }

    #[inline]
    fn major_linker_version(&self) -> u8 {
        self.major_linker_version
    }

    #[inline]
    fn minor_linker_version(&self) -> u8 {
        self.minor_linker_version
    }

    #[inline]
    fn size_of_code(&self) -> u32 {
        self.size_of_code.get(LE)
    }

    #[inline]
    fn size_of_initialized_data(&self) -> u32 {
        self.size_of_initialized_data.get(LE)
    }

    #[inline]
    fn size_of_uninitialized_data(&self) -> u32 {
        self.size_of_uninitialized_data.get(LE)
    }

    #[inline]
    fn address_of_entry_point(&self) -> u32 {
        self.address_of_entry_point.get(LE)
    }

    #[inline]
    fn base_of_code(&self) -> u32 {
        self.base_of_code.get(LE)
    }

    #[inline]
    fn image_base(&self) -> u64 {
        self.image_base.get(LE)
    }

    #[inline]
    fn section_alignment(&self) -> u32 {
        self.section_alignment.get(LE)
    }

    #[inline]
    fn file_alignment(&self) -> u32 {
        self.file_alignment.get(LE)
    }

    #[inline]
    fn major_operating_system_version(&self) -> u16 {
        self.major_operating_system_version.get(LE)
    }

    #[inline]
    fn minor_operating_system_version(&self) -> u16 {
        self.minor_operating_system_version.get(LE)
    }

    #[inline]
    fn major_image_version(&self) -> u16 {
        self.major_image_version.get(LE)
    }

    #[inline]
    fn minor_image_version(&self) -> u16 {
        self.minor_image_version.get(LE)
    }

    #[inline]
    fn major_subsystem_version(&self) -> u16 {
        self.major_subsystem_version.get(LE)
    }

    #[inline]
    fn minor_subsystem_version(&self) -> u16 {
        self.minor_subsystem_version.get(LE)
    }

    #[inline]
    fn win32_version_value(&self) -> u32 {
        self.win32_version_value.get(LE)
    }

    #[inline]
    fn size_of_image(&self) -> u32 {
        self.size_of_image.get(LE)
    }

    #[inline]
    fn size_of_headers(&self) -> u32 {
        self.size_of_headers.get(LE)
    }

    #[inline]
    fn check_sum(&self) -> u32 {
        self.check_sum.get(LE)
    }

    #[inline]
    fn subsystem(&self) -> u16 {
        self.subsystem.get(LE)
    }

    #[inline]
    fn dll_characteristics(&self) -> u16 {
        self.dll_characteristics.get(LE)
    }

    #[inline]
    fn size_of_stack_reserve(&self) -> u64 {
        self.size_of_stack_reserve.get(LE)
    }

    #[inline]
    fn size_of_stack_commit(&self) -> u64 {
        self.size_of_stack_commit.get(LE)
    }

    #[inline]
    fn size_of_heap_reserve(&self) -> u64 {
        self.size_of_heap_reserve.get(LE)
    }

    #[inline]
    fn size_of_heap_commit(&self) -> u64 {
        self.size_of_heap_commit.get(LE)
    }

    #[inline]
    fn loader_flags(&self) -> u32 {
        self.loader_flags.get(LE)
    }

    #[inline]
    fn number_of_rva_and_sizes(&self) -> u32 {
        self.number_of_rva_and_sizes.get(LE)
    }
}
