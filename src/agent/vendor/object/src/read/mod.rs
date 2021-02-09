//! Interface for reading object files.

use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::{fmt, result};

use crate::common::*;
use crate::Bytes;

mod util;
pub use util::StringTable;

mod any;
pub use any::*;

#[cfg(feature = "archive")]
pub mod archive;

#[cfg(feature = "coff")]
pub mod coff;

#[cfg(feature = "elf")]
pub mod elf;

#[cfg(feature = "macho")]
pub mod macho;

#[cfg(feature = "pe")]
pub mod pe;

mod traits;
pub use traits::*;

#[cfg(feature = "wasm")]
pub mod wasm;

mod private {
    pub trait Sealed {}
}

/// The error type used within the read module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error(&'static str);

impl fmt::Display for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

/// The result type used within the read module.
pub type Result<T> = result::Result<T, Error>;

trait ReadError<T> {
    fn read_error(self, error: &'static str) -> Result<T>;
}

impl<T> ReadError<T> for result::Result<T, ()> {
    fn read_error(self, error: &'static str) -> Result<T> {
        self.map_err(|()| Error(error))
    }
}

impl<T> ReadError<T> for Option<T> {
    fn read_error(self, error: &'static str) -> Result<T> {
        self.ok_or(Error(error))
    }
}

/// The native executable file for the target platform.
#[cfg(all(
    unix,
    not(target_os = "macos"),
    target_pointer_width = "32",
    feature = "elf"
))]
pub type NativeFile<'data> = elf::ElfFile32<'data>;

/// The native executable file for the target platform.
#[cfg(all(
    unix,
    not(target_os = "macos"),
    target_pointer_width = "64",
    feature = "elf"
))]
pub type NativeFile<'data> = elf::ElfFile64<'data>;

/// The native executable file for the target platform.
#[cfg(all(target_os = "macos", target_pointer_width = "32", feature = "macho"))]
pub type NativeFile<'data> = macho::MachOFile32<'data>;

/// The native executable file for the target platform.
#[cfg(all(target_os = "macos", target_pointer_width = "64", feature = "macho"))]
pub type NativeFile<'data> = macho::MachOFile64<'data>;

/// The native executable file for the target platform.
#[cfg(all(target_os = "windows", target_pointer_width = "32", feature = "pe"))]
pub type NativeFile<'data> = pe::PeFile32<'data>;

/// The native executable file for the target platform.
#[cfg(all(target_os = "windows", target_pointer_width = "64", feature = "pe"))]
pub type NativeFile<'data> = pe::PeFile64<'data>;

/// The native executable file for the target platform.
#[cfg(all(feature = "wasm", target_arch = "wasm32", feature = "wasm"))]
pub type NativeFile<'data> = wasm::WasmFile<'data>;

/// The index used to identify a section of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SectionIndex(pub usize);

/// The index used to identify a symbol of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolIndex(pub usize);

/// The section where a symbol is defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolSection {
    /// The section is unknown.
    Unknown,
    /// The section is not applicable for this symbol (such as file symbols).
    None,
    /// The symbol is undefined.
    Undefined,
    /// The symbol has an absolute value.
    Absolute,
    /// The symbol is a zero-initialized symbol that will be combined with duplicate definitions.
    Common,
    /// The symbol is defined in the given section.
    Section(SectionIndex),
}

impl SymbolSection {
    /// Returns the section index for the section where the symbol is defined.
    ///
    /// May return `None` if the symbol is not defined in a section.
    #[inline]
    pub fn index(self) -> Option<SectionIndex> {
        if let SymbolSection::Section(index) = self {
            Some(index)
        } else {
            None
        }
    }
}

/// An entry in a `SymbolMap`.
pub trait SymbolMapEntry {
    /// The symbol address.
    fn address(&self) -> u64;
}

/// A map from addresses to symbols.
#[derive(Debug, Default, Clone)]
pub struct SymbolMap<T: SymbolMapEntry> {
    symbols: Vec<T>,
}

impl<T: SymbolMapEntry> SymbolMap<T> {
    /// Construct a new symbol map.
    ///
    /// This function will sort the symbols by address.
    pub fn new(mut symbols: Vec<T>) -> Self {
        symbols.sort_unstable_by_key(|s| s.address());
        SymbolMap { symbols }
    }

    /// Get the symbol before the given address.
    pub fn get(&self, address: u64) -> Option<&T> {
        let index = match self
            .symbols
            .binary_search_by_key(&address, |symbol| symbol.address())
        {
            Ok(index) => index,
            Err(index) => index.checked_sub(1)?,
        };
        self.symbols.get(index)
    }

    /// Get all symbols in the map.
    #[inline]
    pub fn symbols(&self) -> &[T] {
        &self.symbols
    }
}

/// A `SymbolMap` entry for symbol names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolMapName<'data> {
    address: u64,
    name: &'data str,
}

impl<'data> SymbolMapName<'data> {
    /// Construct a `SymbolMapName`.
    pub fn new(address: u64, name: &'data str) -> Self {
        SymbolMapName { address, name }
    }

    /// The symbol address.
    #[inline]
    pub fn address(&self) -> u64 {
        self.address
    }

    /// The symbol name.
    #[inline]
    pub fn name(&self) -> &'data str {
        self.name
    }
}

impl<'data> SymbolMapEntry for SymbolMapName<'data> {
    #[inline]
    fn address(&self) -> u64 {
        self.address
    }
}

/// A map from addresses to symbol names and object files.
///
/// This is derived from STAB entries in Mach-O files.
#[derive(Debug, Default, Clone)]
pub struct ObjectMap<'data> {
    symbols: SymbolMap<ObjectMapEntry<'data>>,
    objects: Vec<&'data [u8]>,
}

impl<'data> ObjectMap<'data> {
    /// Get the entry containing the given address.
    pub fn get(&self, address: u64) -> Option<&ObjectMapEntry<'data>> {
        self.symbols
            .get(address)
            .filter(|entry| entry.size == 0 || address.wrapping_sub(entry.address) < entry.size)
    }

    /// Get all symbols in the map.
    #[inline]
    pub fn symbols(&self) -> &[ObjectMapEntry<'data>] {
        self.symbols.symbols()
    }

    /// Get all objects in the map.
    #[inline]
    pub fn objects(&self) -> &[&'data [u8]] {
        &self.objects
    }
}

/// A `ObjectMap` entry.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectMapEntry<'data> {
    address: u64,
    size: u64,
    name: &'data [u8],
    object: usize,
}

impl<'data> ObjectMapEntry<'data> {
    /// Get the symbol address.
    #[inline]
    pub fn address(&self) -> u64 {
        self.address
    }

    /// Get the symbol size.
    ///
    /// This may be 0 if the size is unknown.
    #[inline]
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Get the symbol name.
    #[inline]
    pub fn name(&self) -> &'data [u8] {
        self.name
    }

    /// Get the index of the object file name.
    #[inline]
    pub fn object_index(&self) -> usize {
        self.object
    }

    /// Get the object file name.
    #[inline]
    pub fn object(&self, map: &ObjectMap<'data>) -> &'data [u8] {
        map.objects[self.object]
    }
}

impl<'data> SymbolMapEntry for ObjectMapEntry<'data> {
    #[inline]
    fn address(&self) -> u64 {
        self.address
    }
}

/// The target referenced by a relocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelocationTarget {
    /// The target is a symbol.
    Symbol(SymbolIndex),
    /// The target is a section.
    Section(SectionIndex),
}

/// A relocation entry.
#[derive(Debug)]
pub struct Relocation {
    kind: RelocationKind,
    encoding: RelocationEncoding,
    size: u8,
    target: RelocationTarget,
    addend: i64,
    implicit_addend: bool,
}

impl Relocation {
    /// The operation used to calculate the result of the relocation.
    #[inline]
    pub fn kind(&self) -> RelocationKind {
        self.kind
    }

    /// Information about how the result of the relocation operation is encoded in the place.
    #[inline]
    pub fn encoding(&self) -> RelocationEncoding {
        self.encoding
    }

    /// The size in bits of the place of the relocation.
    ///
    /// If 0, then the size is determined by the relocation kind.
    #[inline]
    pub fn size(&self) -> u8 {
        self.size
    }

    /// The target of the relocation.
    #[inline]
    pub fn target(&self) -> RelocationTarget {
        self.target
    }

    /// The addend to use in the relocation calculation.
    #[inline]
    pub fn addend(&self) -> i64 {
        self.addend
    }

    /// Set the addend to use in the relocation calculation.
    #[inline]
    pub fn set_addend(&mut self, addend: i64) {
        self.addend = addend
    }

    /// Returns true if there is an implicit addend stored in the data at the offset
    /// to be relocated.
    #[inline]
    pub fn has_implicit_addend(&self) -> bool {
        self.implicit_addend
    }
}

fn data_range(data: Bytes, data_address: u64, range_address: u64, size: u64) -> Option<&[u8]> {
    let offset = range_address.checked_sub(data_address)?;
    let data = data.read_bytes_at(offset as usize, size as usize).ok()?;
    Some(data.0)
}

/// Data that may be compressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompressedData<'data> {
    /// The data compression format.
    pub format: CompressionFormat,
    /// The compressed data.
    pub data: &'data [u8],
    /// The uncompressed data size.
    pub uncompressed_size: usize,
}

/// A data compression format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompressionFormat {
    /// The data is uncompressed.
    None,
    /// The data is compressed, but the compression format is unknown.
    Unknown,
    /// ZLIB/DEFLATE.
    ///
    /// Used for ELF compression and GNU compressed debug information.
    Zlib,
}

impl<'data> CompressedData<'data> {
    /// Data that is uncompressed.
    #[inline]
    pub fn none(data: &'data [u8]) -> Self {
        CompressedData {
            format: CompressionFormat::None,
            data,
            uncompressed_size: data.len(),
        }
    }

    /// Return the uncompressed data.
    ///
    /// Returns an error for invalid data or unsupported compression.
    /// This includes if the data is compressed but the `compression` feature
    /// for this crate is disabled.
    pub fn decompress(self) -> Result<Cow<'data, [u8]>> {
        match self.format {
            CompressionFormat::None => Ok(Cow::Borrowed(self.data)),
            #[cfg(feature = "compression")]
            CompressionFormat::Zlib => {
                let mut decompressed = Vec::with_capacity(self.uncompressed_size);
                let mut decompress = flate2::Decompress::new(true);
                decompress
                    .decompress_vec(
                        self.data,
                        &mut decompressed,
                        flate2::FlushDecompress::Finish,
                    )
                    .ok()
                    .read_error("Invalid zlib compressed data")?;
                Ok(Cow::Owned(decompressed))
            }
            _ => Err(Error("Unsupported compressed data.")),
        }
    }
}
