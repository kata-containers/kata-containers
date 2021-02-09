//! Support for reading Wasm files.
//!
//! Provides `WasmFile` and related types which implement the `Object` trait.
//!
//! Currently implements the minimum required to access DWARF debugging information.
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::{slice, str};
use wasmparser as wp;

use crate::read::{
    self, Architecture, ComdatKind, CompressedData, Error, FileFlags, Object, ObjectComdat,
    ObjectSection, ObjectSegment, ReadError, Relocation, Result, SectionFlags, SectionIndex,
    SectionKind, Symbol, SymbolFlags, SymbolIndex, SymbolKind, SymbolMap, SymbolScope,
    SymbolSection,
};

const SECTION_CUSTOM: usize = 0;
const SECTION_TYPE: usize = 1;
const SECTION_IMPORT: usize = 2;
const SECTION_FUNCTION: usize = 3;
const SECTION_TABLE: usize = 4;
const SECTION_MEMORY: usize = 5;
const SECTION_GLOBAL: usize = 6;
const SECTION_EXPORT: usize = 7;
const SECTION_START: usize = 8;
const SECTION_ELEMENT: usize = 9;
const SECTION_CODE: usize = 10;
const SECTION_DATA: usize = 11;
const SECTION_DATA_COUNT: usize = 12;
// Update this constant when adding new section id:
const MAX_SECTION_ID: usize = SECTION_DATA_COUNT;

/// A WebAssembly object file.
#[derive(Debug, Default)]
pub struct WasmFile<'data> {
    // All sections, including custom sections.
    sections: Vec<wp::Section<'data>>,
    // Indices into `sections` of sections with a non-zero id.
    id_sections: Box<[Option<usize>; MAX_SECTION_ID + 1]>,
    // Whether the file has DWARF information.
    has_debug_symbols: bool,
    // Symbols collected from imports, exports, code and name sections.
    symbols: Vec<Symbol<'data>>,
    // Address of the function body for the entry point.
    entry: u64,
}

#[derive(Clone)]
enum LocalFunctionKind {
    Unknown,
    Exported { symbol_ids: Vec<u32> },
    Local { symbol_id: u32 },
}

impl<T> ReadError<T> for wasmparser::Result<T> {
    fn read_error(self, error: &'static str) -> Result<T> {
        self.map_err(|_| Error(error))
    }
}

impl<'data> WasmFile<'data> {
    /// Parse the raw wasm data.
    pub fn parse(data: &'data [u8]) -> Result<Self> {
        let module = wp::ModuleReader::new(data).read_error("Invalid Wasm header")?;

        let mut file = WasmFile::default();

        let mut main_file_symbol = Some(Symbol {
            name: None,
            address: 0,
            size: 0,
            kind: SymbolKind::File,
            section: SymbolSection::None,
            weak: false,
            scope: SymbolScope::Compilation,
            flags: SymbolFlags::None,
        });

        let mut imported_funcs_count = 0;
        let mut local_func_kinds = Vec::new();
        let mut entry_func_id = None;

        for section in module {
            let section = section.read_error("Invalid Wasm section header")?;

            match section.code {
                wp::SectionCode::Import => {
                    let mut last_module_name = None;

                    for import in section
                        .get_import_section_reader()
                        .read_error("Couldn't read header of the import section")?
                    {
                        let import = import.read_error("Couldn't read an import item")?;
                        let module_name = Some(import.module);

                        if last_module_name != module_name {
                            file.symbols.push(Symbol {
                                name: module_name,
                                address: 0,
                                size: 0,
                                kind: SymbolKind::File,
                                section: SymbolSection::None,
                                weak: false,
                                scope: SymbolScope::Dynamic,
                                flags: SymbolFlags::None,
                            });
                            last_module_name = module_name;
                        }

                        let kind = match import.ty {
                            wp::ImportSectionEntryType::Function(_) => {
                                imported_funcs_count += 1;
                                SymbolKind::Text
                            }
                            wp::ImportSectionEntryType::Table(_)
                            | wp::ImportSectionEntryType::Memory(_)
                            | wp::ImportSectionEntryType::Global(_) => SymbolKind::Data,
                        };

                        file.symbols.push(Symbol {
                            name: Some(import.field),
                            address: 0,
                            size: 0,
                            kind,
                            section: SymbolSection::Undefined,
                            weak: false,
                            scope: SymbolScope::Dynamic,
                            flags: SymbolFlags::None,
                        });
                    }
                }
                wp::SectionCode::Function => {
                    local_func_kinds = vec![
                        LocalFunctionKind::Unknown;
                        section
                            .get_function_section_reader()
                            .read_error("Couldn't read header of the function section")?
                            .get_count() as usize
                    ];
                }
                wp::SectionCode::Export => {
                    if let Some(main_file_symbol) = main_file_symbol.take() {
                        file.symbols.push(main_file_symbol);
                    }

                    for export in section
                        .get_export_section_reader()
                        .read_error("Couldn't read header of the export section")?
                    {
                        let export = export.read_error("Couldn't read an export item")?;

                        let (kind, section_idx) = match export.kind {
                            wp::ExternalKind::Function => {
                                if let Some(local_func_id) =
                                    export.index.checked_sub(imported_funcs_count)
                                {
                                    let local_func_kind =
                                        &mut local_func_kinds[local_func_id as usize];
                                    if let LocalFunctionKind::Unknown = local_func_kind {
                                        *local_func_kind = LocalFunctionKind::Exported {
                                            symbol_ids: Vec::new(),
                                        };
                                    }
                                    let symbol_ids = match local_func_kind {
                                        LocalFunctionKind::Exported { symbol_ids } => symbol_ids,
                                        _ => unreachable!(),
                                    };
                                    symbol_ids.push(file.symbols.len() as u32);
                                }
                                (SymbolKind::Text, SECTION_CODE)
                            }
                            wp::ExternalKind::Table
                            | wp::ExternalKind::Memory
                            | wp::ExternalKind::Global => (SymbolKind::Data, SECTION_DATA),
                        };

                        file.symbols.push(Symbol {
                            name: Some(export.field),
                            address: 0,
                            size: 0,
                            kind,
                            section: SymbolSection::Section(SectionIndex(section_idx)),
                            weak: false,
                            scope: SymbolScope::Dynamic,
                            flags: SymbolFlags::None,
                        });
                    }
                }
                wp::SectionCode::Start => {
                    entry_func_id = Some(
                        section
                            .get_start_section_content()
                            .read_error("Couldn't read contents of the start section")?,
                    );
                }
                wp::SectionCode::Code => {
                    if let Some(main_file_symbol) = main_file_symbol.take() {
                        file.symbols.push(main_file_symbol);
                    }

                    for (i, (body, local_func_kind)) in section
                        .get_code_section_reader()
                        .read_error("Couldn't read header of the code section")?
                        .into_iter()
                        .zip(&mut local_func_kinds)
                        .enumerate()
                    {
                        let body = body.read_error("Couldn't read a function body")?;
                        let range = body.range();

                        let address = range.start as u64 - section.range().start as u64;
                        let size = (range.end - range.start) as u64;

                        if entry_func_id == Some(i as u32) {
                            file.entry = address;
                        }

                        match local_func_kind {
                            LocalFunctionKind::Unknown => {
                                *local_func_kind = LocalFunctionKind::Local {
                                    symbol_id: file.symbols.len() as u32,
                                };
                                file.symbols.push(Symbol {
                                    section: SymbolSection::Section(SectionIndex(SECTION_CODE)),
                                    address,
                                    size,
                                    kind: SymbolKind::Text,
                                    name: None,
                                    weak: false,
                                    scope: SymbolScope::Compilation,
                                    flags: SymbolFlags::None,
                                });
                            }
                            LocalFunctionKind::Exported { symbol_ids } => {
                                for symbol_id in core::mem::replace(symbol_ids, Vec::new()) {
                                    let export_symbol = &mut file.symbols[symbol_id as usize];
                                    export_symbol.address = address;
                                    export_symbol.size = size;
                                }
                            }
                            _ => unreachable!(),
                        }
                    }
                }
                wp::SectionCode::Custom {
                    kind: wp::CustomSectionKind::Name,
                    ..
                } => {
                    for name in section
                        .get_name_section_reader()
                        .read_error("Couldn't read header of the name section")?
                    {
                        let name =
                            match name.read_error("Couldn't read header of a name subsection")? {
                                wp::Name::Function(name) => name,
                                _ => continue,
                            };
                        let mut name_map = name
                            .get_map()
                            .read_error("Couldn't read header of the function name subsection")?;
                        for _ in 0..name_map.get_count() {
                            let naming = name_map
                                .read()
                                .read_error("Couldn't read a function name")?;
                            if let Some(local_index) =
                                naming.index.checked_sub(imported_funcs_count)
                            {
                                if let LocalFunctionKind::Local { symbol_id } =
                                    local_func_kinds[local_index as usize]
                                {
                                    file.symbols[symbol_id as usize].name = Some(naming.name);
                                }
                            }
                        }
                    }
                }
                wp::SectionCode::Custom { name, .. } if name.starts_with(".debug_") => {
                    file.has_debug_symbols = true;
                }
                _ => {}
            }

            let id = section_code_to_id(section.code);
            file.id_sections[id] = Some(file.sections.len());

            file.sections.push(section);
        }

        Ok(file)
    }
}

impl<'data> read::private::Sealed for WasmFile<'data> {}

impl<'data, 'file> Object<'data, 'file> for WasmFile<'data>
where
    'data: 'file,
{
    type Segment = WasmSegment<'data, 'file>;
    type SegmentIterator = WasmSegmentIterator<'data, 'file>;
    type Section = WasmSection<'data, 'file>;
    type SectionIterator = WasmSectionIterator<'data, 'file>;
    type Comdat = WasmComdat<'data, 'file>;
    type ComdatIterator = WasmComdatIterator<'data, 'file>;
    type SymbolIterator = WasmSymbolIterator<'data, 'file>;

    #[inline]
    fn architecture(&self) -> Architecture {
        Architecture::Wasm32
    }

    #[inline]
    fn is_little_endian(&self) -> bool {
        true
    }

    #[inline]
    fn is_64(&self) -> bool {
        false
    }

    fn segments(&'file self) -> Self::SegmentIterator {
        WasmSegmentIterator { file: self }
    }

    #[inline]
    fn entry(&'file self) -> u64 {
        self.entry
    }

    fn section_by_name(&'file self, section_name: &str) -> Option<WasmSection<'data, 'file>> {
        self.sections()
            .find(|section| section.name() == Ok(section_name))
    }

    fn section_by_index(&'file self, index: SectionIndex) -> Result<WasmSection<'data, 'file>> {
        // TODO: Missing sections should return an empty section.
        let id_section = self
            .id_sections
            .get(index.0)
            .and_then(|x| *x)
            .read_error("Invalid Wasm section index")?;
        let section = self.sections.get(id_section).unwrap();
        Ok(WasmSection { section })
    }

    fn sections(&'file self) -> Self::SectionIterator {
        WasmSectionIterator {
            sections: self.sections.iter(),
        }
    }

    fn comdats(&'file self) -> Self::ComdatIterator {
        WasmComdatIterator { file: self }
    }

    #[inline]
    fn symbol_by_index(&self, index: SymbolIndex) -> Result<Symbol<'data>> {
        self.symbols
            .get(index.0)
            .cloned()
            .read_error("Invalid Wasm symbol index")
    }

    fn symbols(&'file self) -> Self::SymbolIterator {
        WasmSymbolIterator {
            symbols: self.symbols.iter().enumerate(),
        }
    }

    fn dynamic_symbols(&'file self) -> Self::SymbolIterator {
        WasmSymbolIterator {
            symbols: [].iter().enumerate(),
        }
    }

    fn symbol_map(&self) -> SymbolMap<'data> {
        SymbolMap {
            symbols: self.symbols.clone(),
        }
    }

    fn has_debug_symbols(&self) -> bool {
        self.has_debug_symbols
    }

    #[inline]
    fn flags(&self) -> FileFlags {
        FileFlags::None
    }
}

/// An iterator over the segments of a `WasmFile`.
#[derive(Debug)]
pub struct WasmSegmentIterator<'data, 'file> {
    file: &'file WasmFile<'data>,
}

impl<'data, 'file> Iterator for WasmSegmentIterator<'data, 'file> {
    type Item = WasmSegment<'data, 'file>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

/// A segment of a `WasmFile`.
#[derive(Debug)]
pub struct WasmSegment<'data, 'file> {
    file: &'file WasmFile<'data>,
}

impl<'data, 'file> read::private::Sealed for WasmSegment<'data, 'file> {}

impl<'data, 'file> ObjectSegment<'data> for WasmSegment<'data, 'file> {
    #[inline]
    fn address(&self) -> u64 {
        unreachable!()
    }

    #[inline]
    fn size(&self) -> u64 {
        unreachable!()
    }

    #[inline]
    fn align(&self) -> u64 {
        unreachable!()
    }

    #[inline]
    fn file_range(&self) -> (u64, u64) {
        unreachable!()
    }

    fn data(&self) -> Result<&'data [u8]> {
        unreachable!()
    }

    fn data_range(&self, _address: u64, _size: u64) -> Result<Option<&'data [u8]>> {
        unreachable!()
    }

    #[inline]
    fn name(&self) -> Result<Option<&str>> {
        unreachable!()
    }
}

/// An iterator over the sections of a `WasmFile`.
#[derive(Debug)]
pub struct WasmSectionIterator<'data, 'file> {
    sections: slice::Iter<'file, wp::Section<'data>>,
}

impl<'data, 'file> Iterator for WasmSectionIterator<'data, 'file> {
    type Item = WasmSection<'data, 'file>;

    fn next(&mut self) -> Option<Self::Item> {
        let section = self.sections.next()?;
        Some(WasmSection { section })
    }
}

/// A section of a `WasmFile`.
#[derive(Debug)]
pub struct WasmSection<'data, 'file> {
    section: &'file wp::Section<'data>,
}

impl<'data, 'file> read::private::Sealed for WasmSection<'data, 'file> {}

impl<'data, 'file> ObjectSection<'data> for WasmSection<'data, 'file> {
    type RelocationIterator = WasmRelocationIterator<'data, 'file>;

    #[inline]
    fn index(&self) -> SectionIndex {
        // Note that we treat all custom sections as index 0.
        // This is ok because they are never looked up by index.
        SectionIndex(section_code_to_id(self.section.code))
    }

    #[inline]
    fn address(&self) -> u64 {
        0
    }

    #[inline]
    fn size(&self) -> u64 {
        let range = self.section.range();
        (range.end - range.start) as u64
    }

    #[inline]
    fn align(&self) -> u64 {
        1
    }

    #[inline]
    fn file_range(&self) -> Option<(u64, u64)> {
        let range = self.section.range();
        Some((range.start as _, range.end as _))
    }

    #[inline]
    fn data(&self) -> Result<&'data [u8]> {
        let mut reader = self.section.get_binary_reader();
        // TODO: raise a feature request upstream to be able
        // to get remaining slice from a BinaryReader directly.
        Ok(reader.read_bytes(reader.bytes_remaining()).unwrap())
    }

    fn data_range(&self, _address: u64, _size: u64) -> Result<Option<&'data [u8]>> {
        unimplemented!()
    }

    #[inline]
    fn compressed_data(&self) -> Result<CompressedData<'data>> {
        self.data().map(CompressedData::none)
    }

    #[inline]
    fn name(&self) -> Result<&str> {
        Ok(match self.section.code {
            wp::SectionCode::Custom { name, .. } => name,
            wp::SectionCode::Type => "<type>",
            wp::SectionCode::Import => "<import>",
            wp::SectionCode::Function => "<function>",
            wp::SectionCode::Table => "<table>",
            wp::SectionCode::Memory => "<memory>",
            wp::SectionCode::Global => "<global>",
            wp::SectionCode::Export => "<export>",
            wp::SectionCode::Start => "<start>",
            wp::SectionCode::Element => "<element>",
            wp::SectionCode::Code => "<code>",
            wp::SectionCode::Data => "<data>",
            wp::SectionCode::DataCount => "<data_count>",
        })
    }

    #[inline]
    fn segment_name(&self) -> Result<Option<&str>> {
        Ok(None)
    }

    #[inline]
    fn kind(&self) -> SectionKind {
        match self.section.code {
            wp::SectionCode::Custom { kind, .. } => match kind {
                wp::CustomSectionKind::Reloc | wp::CustomSectionKind::Linking => {
                    SectionKind::Linker
                }
                _ => SectionKind::Other,
            },
            wp::SectionCode::Type => SectionKind::Metadata,
            wp::SectionCode::Import => SectionKind::Linker,
            wp::SectionCode::Function => SectionKind::Metadata,
            wp::SectionCode::Table => SectionKind::UninitializedData,
            wp::SectionCode::Memory => SectionKind::UninitializedData,
            wp::SectionCode::Global => SectionKind::Data,
            wp::SectionCode::Export => SectionKind::Linker,
            wp::SectionCode::Start => SectionKind::Linker,
            wp::SectionCode::Element => SectionKind::Data,
            wp::SectionCode::Code => SectionKind::Text,
            wp::SectionCode::Data => SectionKind::Data,
            wp::SectionCode::DataCount => SectionKind::UninitializedData,
        }
    }

    #[inline]
    fn relocations(&self) -> WasmRelocationIterator<'data, 'file> {
        WasmRelocationIterator::default()
    }

    #[inline]
    fn flags(&self) -> SectionFlags {
        SectionFlags::None
    }
}

/// An iterator over the COMDAT section groups of a `WasmFile`.
#[derive(Debug)]
pub struct WasmComdatIterator<'data, 'file> {
    file: &'file WasmFile<'data>,
}

impl<'data, 'file> Iterator for WasmComdatIterator<'data, 'file> {
    type Item = WasmComdat<'data, 'file>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

/// A COMDAT section group of a `WasmFile`.
#[derive(Debug)]
pub struct WasmComdat<'data, 'file> {
    file: &'file WasmFile<'data>,
}

impl<'data, 'file> read::private::Sealed for WasmComdat<'data, 'file> {}

impl<'data, 'file> ObjectComdat<'data> for WasmComdat<'data, 'file> {
    type SectionIterator = WasmComdatSectionIterator<'data, 'file>;

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

/// An iterator over the sections in a COMDAT section group of a `WasmFile`.
#[derive(Debug)]
pub struct WasmComdatSectionIterator<'data, 'file>
where
    'data: 'file,
{
    file: &'file WasmFile<'data>,
}

impl<'data, 'file> Iterator for WasmComdatSectionIterator<'data, 'file> {
    type Item = SectionIndex;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

/// An iterator over the symbols of a `WasmFile`.
#[derive(Debug)]
pub struct WasmSymbolIterator<'data, 'file> {
    symbols: core::iter::Enumerate<slice::Iter<'file, Symbol<'data>>>,
}

impl<'data, 'file> Iterator for WasmSymbolIterator<'data, 'file> {
    type Item = (SymbolIndex, Symbol<'data>);

    fn next(&mut self) -> Option<Self::Item> {
        let (index, symbol) = self.symbols.next()?;
        Some((SymbolIndex(index), symbol.clone()))
    }
}

/// An iterator over the relocations in a `WasmSection`.
#[derive(Debug, Default)]
pub struct WasmRelocationIterator<'data, 'file>(PhantomData<(&'data (), &'file ())>);

impl<'data, 'file> Iterator for WasmRelocationIterator<'data, 'file> {
    type Item = (u64, Relocation);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

fn section_code_to_id(code: wp::SectionCode) -> usize {
    match code {
        wp::SectionCode::Custom { .. } => SECTION_CUSTOM,
        wp::SectionCode::Type => SECTION_TYPE,
        wp::SectionCode::Import => SECTION_IMPORT,
        wp::SectionCode::Function => SECTION_FUNCTION,
        wp::SectionCode::Table => SECTION_TABLE,
        wp::SectionCode::Memory => SECTION_MEMORY,
        wp::SectionCode::Global => SECTION_GLOBAL,
        wp::SectionCode::Export => SECTION_EXPORT,
        wp::SectionCode::Start => SECTION_START,
        wp::SectionCode::Element => SECTION_ELEMENT,
        wp::SectionCode::Code => SECTION_CODE,
        wp::SectionCode::Data => SECTION_DATA,
        wp::SectionCode::DataCount => SECTION_DATA_COUNT,
    }
}
