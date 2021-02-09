use alloc::borrow::Cow;

use crate::read::{
    self, Architecture, ComdatKind, CompressedData, FileFlags, Relocation, Result, SectionFlags,
    SectionIndex, SectionKind, Symbol, SymbolIndex, SymbolMap,
};
use crate::Endianness;

/// An object file.
pub trait Object<'data, 'file>: read::private::Sealed {
    /// A segment in the object file.
    type Segment: ObjectSegment<'data>;

    /// An iterator over the segments in the object file.
    type SegmentIterator: Iterator<Item = Self::Segment>;

    /// A section in the object file.
    type Section: ObjectSection<'data>;

    /// An iterator over the sections in the object file.
    type SectionIterator: Iterator<Item = Self::Section>;

    /// A COMDAT section group in the object file.
    type Comdat: ObjectComdat<'data>;

    /// An iterator over the COMDAT section groups in the object file.
    type ComdatIterator: Iterator<Item = Self::Comdat>;

    /// An iterator over the symbols in the object file.
    type SymbolIterator: Iterator<Item = (SymbolIndex, Symbol<'data>)>;

    /// Get the architecture type of the file.
    fn architecture(&self) -> Architecture;

    /// Get the endianness of the file.
    #[inline]
    fn endianness(&self) -> Endianness {
        if self.is_little_endian() {
            Endianness::Little
        } else {
            Endianness::Big
        }
    }

    /// Return true if the file is little endian, false if it is big endian.
    fn is_little_endian(&self) -> bool;

    /// Return true if the file can contain 64-bit addresses.
    fn is_64(&self) -> bool;

    /// Get an iterator over the segments in the file.
    fn segments(&'file self) -> Self::SegmentIterator;

    /// Get the entry point address of the binary
    fn entry(&'file self) -> u64;

    /// Get the section named `section_name`, if such a section exists.
    ///
    /// If `section_name` starts with a '.' then it is treated as a system section name,
    /// and is compared using the conventions specific to the object file format. This
    /// includes:
    /// - if ".text" is requested for a Mach-O object file, then the actual
    /// section name that is searched for is "__text".
    /// - if ".debug_info" is requested for an ELF object file, then
    /// ".zdebug_info" may be returned (and similarly for other debug sections).
    ///
    /// For some object files, multiple segments may contain sections with the same
    /// name. In this case, the first matching section will be used.
    ///
    /// This method skips over sections with invalid names.
    fn section_by_name(&'file self, section_name: &str) -> Option<Self::Section>;

    /// Get the section at the given index.
    ///
    /// The meaning of the index depends on the object file.
    ///
    /// For some object files, this requires iterating through all sections.
    ///
    /// Returns an error if the index is invalid.
    fn section_by_index(&'file self, index: SectionIndex) -> Result<Self::Section>;

    /// Get an iterator over the sections in the file.
    fn sections(&'file self) -> Self::SectionIterator;

    /// Get an iterator over the COMDAT section groups in the file.
    fn comdats(&'file self) -> Self::ComdatIterator;

    /// Get the debugging symbol at the given index.
    ///
    /// The meaning of the index depends on the object file.
    ///
    /// Returns an error if the index is invalid.
    fn symbol_by_index(&self, index: SymbolIndex) -> Result<Symbol<'data>>;

    /// Get an iterator over the debugging symbols in the file.
    ///
    /// This may skip over symbols that are malformed or unsupported.
    fn symbols(&'file self) -> Self::SymbolIterator;

    /// Get the data for the given symbol.
    ///
    /// This may iterate over segments.
    ///
    /// Returns `Ok(None)` for undefined symbols or if the data could not be found.
    fn symbol_data(&'file self, symbol: &Symbol<'data>) -> Result<Option<&'data [u8]>> {
        if symbol.is_undefined() {
            return Ok(None);
        }
        let address = symbol.address();
        let size = symbol.size();
        if let Some(index) = symbol.section_index() {
            let section = self.section_by_index(index)?;
            section.data_range(address, size)
        } else {
            for segment in self.segments() {
                if let Some(data) = segment.data_range(address, size)? {
                    return Ok(Some(data));
                }
            }
            Ok(None)
        }
    }

    /// Get an iterator over the dynamic linking symbols in the file.
    ///
    /// This may skip over symbols that are malformed or unsupported.
    fn dynamic_symbols(&'file self) -> Self::SymbolIterator;

    /// Construct a map from addresses to symbols.
    fn symbol_map(&self) -> SymbolMap<'data>;

    /// Return true if the file contains debug information sections, false if not.
    fn has_debug_symbols(&self) -> bool;

    /// The UUID from a Mach-O `LC_UUID` load command.
    #[inline]
    fn mach_uuid(&self) -> Result<Option<[u8; 16]>> {
        Ok(None)
    }

    /// The build ID from an ELF `NT_GNU_BUILD_ID` note.
    #[inline]
    fn build_id(&self) -> Result<Option<&'data [u8]>> {
        Ok(None)
    }

    /// The filename and CRC from a `.gnu_debuglink` section.
    #[inline]
    fn gnu_debuglink(&self) -> Result<Option<(&'data [u8], u32)>> {
        Ok(None)
    }

    /// File flags that are specific to each file format.
    fn flags(&self) -> FileFlags;
}

/// A loadable segment defined in an object file.
///
/// For ELF, this is a program header with type `PT_LOAD`.
/// For Mach-O, this is a load command with type `LC_SEGMENT` or `LC_SEGMENT_64`.
pub trait ObjectSegment<'data>: read::private::Sealed {
    /// Returns the virtual address of the segment.
    fn address(&self) -> u64;

    /// Returns the size of the segment in memory.
    fn size(&self) -> u64;

    /// Returns the alignment of the segment in memory.
    fn align(&self) -> u64;

    /// Returns the offset and size of the segment in the file.
    fn file_range(&self) -> (u64, u64);

    /// Returns a reference to the file contents of the segment.
    ///
    /// The length of this data may be different from the size of the
    /// segment in memory.
    fn data(&self) -> Result<&'data [u8]>;

    /// Return the segment data in the given range.
    ///
    /// Returns `Ok(None)` if the segment does not contain the given range.
    fn data_range(&self, address: u64, size: u64) -> Result<Option<&'data [u8]>>;

    /// Returns the name of the segment.
    fn name(&self) -> Result<Option<&str>>;
}

/// A section defined in an object file.
pub trait ObjectSection<'data>: read::private::Sealed {
    /// An iterator over the relocations for a section.
    ///
    /// The first field in the item tuple is the section offset
    /// that the relocation applies to.
    type RelocationIterator: Iterator<Item = (u64, Relocation)>;

    /// Returns the section index.
    fn index(&self) -> SectionIndex;

    /// Returns the address of the section.
    fn address(&self) -> u64;

    /// Returns the size of the section in memory.
    fn size(&self) -> u64;

    /// Returns the alignment of the section in memory.
    fn align(&self) -> u64;

    /// Returns offset and size of on-disk segment (if any).
    fn file_range(&self) -> Option<(u64, u64)>;

    /// Returns the raw contents of the section.
    ///
    /// The length of this data may be different from the size of the
    /// section in memory.
    ///
    /// This does not do any decompression.
    fn data(&self) -> Result<&'data [u8]>;

    /// Return the raw contents of the section data in the given range.
    ///
    /// This does not do any decompression.
    ///
    /// Returns `Ok(None)` if the section does not contain the given range.
    fn data_range(&self, address: u64, size: u64) -> Result<Option<&'data [u8]>>;

    /// Returns the potentially compressed contents of the section,
    /// along with information about the compression.
    fn compressed_data(&self) -> Result<CompressedData<'data>>;

    /// Returns the uncompressed contents of the section.
    ///
    /// The length of this data may be different from the size of the
    /// section in memory.
    ///
    /// If no compression is detected, then returns the data unchanged.
    /// Returns `Err` if decompression fails.
    fn uncompressed_data(&self) -> Result<Cow<'data, [u8]>> {
        self.compressed_data()?.decompress()
    }

    /// Returns the name of the section.
    fn name(&self) -> Result<&str>;

    /// Returns the name of the segment for this section.
    fn segment_name(&self) -> Result<Option<&str>>;

    /// Return the kind of this section.
    fn kind(&self) -> SectionKind;

    /// Get the relocations for this section.
    fn relocations(&self) -> Self::RelocationIterator;

    /// Section flags that are specific to each file format.
    fn flags(&self) -> SectionFlags;
}

/// A COMDAT section group defined in an object file.
pub trait ObjectComdat<'data>: read::private::Sealed {
    /// An iterator over the sections in the object file.
    type SectionIterator: Iterator<Item = SectionIndex>;

    /// Returns the COMDAT selection kind.
    fn kind(&self) -> ComdatKind;

    /// Returns the index of the symbol used for the name of COMDAT section group.
    fn symbol(&self) -> SymbolIndex;

    /// Returns the name of the COMDAT section group.
    fn name(&self) -> Result<&str>;

    /// Get the sections in this section group.
    fn sections(&self) -> Self::SectionIterator;
}
