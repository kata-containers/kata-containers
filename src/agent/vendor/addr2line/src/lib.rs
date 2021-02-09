//! This crate provides a cross-platform library and binary for translating addresses into
//! function names, file names and line numbers. Given an address in an executable or an
//! offset in a section of a relocatable object, it uses the debugging information to
//! figure out which file name and line number are associated with it.
//!
//! When used as a library, files must first be loaded using the
//! [`object`](https://github.com/gimli-rs/object) crate.
//! A context can then be created with [`Context::new`](./struct.Context.html#method.new).
//! The context caches some of the parsed information so that multiple lookups are
//! efficient.
//! Location information is obtained with
//! [`Context::find_location`](./struct.Context.html#method.find_location).
//! Function information is obtained with
//! [`Context::find_frames`](./struct.Context.html#method.find_frames), which returns
//! a frame for each inline function. Each frame contains both name and location.
//!
//! The crate has an example CLI wrapper around the library which provides some of
//! the functionality of the `addr2line` command line tool distributed with [GNU
//! binutils](https://www.gnu.org/software/binutils/).
//!
//! Currently this library only provides information from the DWARF debugging information,
//! which is parsed using [`gimli`](https://github.com/gimli-rs/gimli).  The example CLI
//! wrapper also uses symbol table information provided by the `object` crate.
#![deny(missing_docs)]
#![no_std]

#[allow(unused_imports)]
#[macro_use]
extern crate alloc;

#[cfg(feature = "cpp_demangle")]
extern crate cpp_demangle;
#[cfg(feature = "fallible-iterator")]
pub extern crate fallible_iterator;
pub extern crate gimli;
#[cfg(feature = "object")]
pub extern crate object;
#[cfg(feature = "rustc-demangle")]
extern crate rustc_demangle;

use alloc::borrow::Cow;
use alloc::boxed::Box;
#[cfg(feature = "object")]
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use core::cmp::Ordering;
use core::iter;
use core::mem;
use core::u64;

use crate::lazy::LazyCell;

#[cfg(feature = "smallvec")]
mod maybe_small {
    pub type Vec<T> = smallvec::SmallVec<[T; 16]>;
    pub type IntoIter<T> = smallvec::IntoIter<[T; 16]>;
}
#[cfg(not(feature = "smallvec"))]
mod maybe_small {
    pub type Vec<T> = alloc::vec::Vec<T>;
    pub type IntoIter<T> = alloc::vec::IntoIter<T>;
}

mod lazy;

type Error = gimli::Error;

/// The state necessary to perform address to line translation.
///
/// Constructing a `Context` is somewhat costly, so users should aim to reuse `Context`s
/// when performing lookups for many addresses in the same executable.
pub struct Context<R>
where
    R: gimli::Reader,
{
    unit_ranges: Vec<UnitRange>,
    units: Vec<ResUnit<R>>,
    sections: gimli::Dwarf<R>,
}

struct UnitRange {
    unit_id: usize,
    max_end: u64,
    range: gimli::Range,
}

/// The type of `Context` that supports the `new` method.
#[cfg(feature = "std-object")]
pub type ObjectContext = Context<gimli::EndianRcSlice<gimli::RunTimeEndian>>;

#[cfg(feature = "std-object")]
impl Context<gimli::EndianRcSlice<gimli::RunTimeEndian>> {
    /// Construct a new `Context`.
    ///
    /// The resulting `Context` uses `gimli::EndianRcSlice<gimli::RunTimeEndian>`.
    /// This means it is not thread safe, has no lifetime constraints (since it copies
    /// the input data), and works for any endianity.
    ///
    /// Performance sensitive applications may want to use `Context::from_sections`
    /// with a more specialised `gimli::Reader` implementation.
    pub fn new<'data, 'file, O: object::Object<'data, 'file>>(
        file: &'file O,
    ) -> Result<Self, Error> {
        let endian = if file.is_little_endian() {
            gimli::RunTimeEndian::Little
        } else {
            gimli::RunTimeEndian::Big
        };

        fn load_section<'data, 'file, O, S, Endian>(file: &'file O, endian: Endian) -> S
        where
            O: object::Object<'data, 'file>,
            S: gimli::Section<gimli::EndianRcSlice<Endian>>,
            Endian: gimli::Endianity,
        {
            use object::ObjectSection;

            let data = file
                .section_by_name(S::section_name())
                .and_then(|section| section.uncompressed_data().ok())
                .unwrap_or(Cow::Borrowed(&[]));
            S::from(gimli::EndianRcSlice::new(Rc::from(&*data), endian))
        }

        let debug_abbrev: gimli::DebugAbbrev<_> = load_section(file, endian);
        let debug_addr: gimli::DebugAddr<_> = load_section(file, endian);
        let debug_info: gimli::DebugInfo<_> = load_section(file, endian);
        let debug_line: gimli::DebugLine<_> = load_section(file, endian);
        let debug_line_str: gimli::DebugLineStr<_> = load_section(file, endian);
        let debug_ranges: gimli::DebugRanges<_> = load_section(file, endian);
        let debug_rnglists: gimli::DebugRngLists<_> = load_section(file, endian);
        let debug_str: gimli::DebugStr<_> = load_section(file, endian);
        let debug_str_offsets: gimli::DebugStrOffsets<_> = load_section(file, endian);
        let default_section = gimli::EndianRcSlice::new(Rc::from(&[][..]), endian);

        Context::from_sections(
            debug_abbrev,
            debug_addr,
            debug_info,
            debug_line,
            debug_line_str,
            debug_ranges,
            debug_rnglists,
            debug_str,
            debug_str_offsets,
            default_section,
        )
    }
}

impl<R: gimli::Reader> Context<R> {
    /// Construct a new `Context` from DWARF sections.
    pub fn from_sections(
        debug_abbrev: gimli::DebugAbbrev<R>,
        debug_addr: gimli::DebugAddr<R>,
        debug_info: gimli::DebugInfo<R>,
        debug_line: gimli::DebugLine<R>,
        debug_line_str: gimli::DebugLineStr<R>,
        debug_ranges: gimli::DebugRanges<R>,
        debug_rnglists: gimli::DebugRngLists<R>,
        debug_str: gimli::DebugStr<R>,
        debug_str_offsets: gimli::DebugStrOffsets<R>,
        default_section: R,
    ) -> Result<Self, Error> {
        Self::from_dwarf(gimli::Dwarf {
            debug_abbrev,
            debug_addr,
            debug_info,
            debug_line,
            debug_line_str,
            debug_str,
            debug_str_offsets,
            debug_str_sup: default_section.clone().into(),
            debug_types: default_section.clone().into(),
            locations: gimli::LocationLists::new(
                default_section.clone().into(),
                default_section.clone().into(),
            ),
            ranges: gimli::RangeLists::new(debug_ranges, debug_rnglists),
        })
    }

    /// Construct a new `Context` from an existing [`gimli::Dwarf`] object.
    pub fn from_dwarf(sections: gimli::Dwarf<R>) -> Result<Self, Error> {
        let mut unit_ranges = Vec::new();
        let mut res_units = Vec::new();
        let mut units = sections.units();
        while let Some(header) = units.next()? {
            let unit_id = res_units.len();
            let offset = header.offset();
            let dw_unit = match sections.unit(header) {
                Ok(dw_unit) => dw_unit,
                Err(_) => continue,
            };

            let mut lang = None;
            {
                let mut entries = dw_unit.entries_raw(None)?;

                let abbrev = match entries.read_abbreviation()? {
                    Some(abbrev) if abbrev.tag() == gimli::DW_TAG_compile_unit => abbrev,
                    _ => continue, // wtf?
                };

                let mut ranges = RangeAttributes::default();
                for spec in abbrev.attributes() {
                    let attr = entries.read_attribute(*spec)?;
                    match attr.name() {
                        gimli::DW_AT_low_pc => {
                            if let gimli::AttributeValue::Addr(val) = attr.value() {
                                ranges.low_pc = Some(val);
                            }
                        }
                        gimli::DW_AT_high_pc => match attr.value() {
                            gimli::AttributeValue::Addr(val) => ranges.high_pc = Some(val),
                            gimli::AttributeValue::Udata(val) => ranges.size = Some(val),
                            _ => {}
                        },
                        gimli::DW_AT_ranges => {
                            ranges.ranges_offset =
                                sections.attr_ranges_offset(&dw_unit, attr.value())?;
                        }
                        gimli::DW_AT_language => {
                            if let gimli::AttributeValue::Language(val) = attr.value() {
                                lang = Some(val);
                            }
                        }
                        _ => {}
                    }
                }

                ranges.for_each_range(&sections, &dw_unit, |range| {
                    unit_ranges.push(UnitRange {
                        range,
                        unit_id,
                        max_end: 0,
                    });
                })?;
            }

            res_units.push(ResUnit {
                offset,
                dw_unit,
                lang,
                lines: LazyCell::new(),
                funcs: LazyCell::new(),
            });
        }

        // Sort this for faster lookup in `find_unit_and_address` below.
        unit_ranges.sort_by_key(|i| i.range.begin);

        // Calculate the `max_end` field now that we've determined the order of
        // CUs.
        let mut max = 0;
        for i in unit_ranges.iter_mut() {
            max = max.max(i.range.end);
            i.max_end = max;
        }

        Ok(Context {
            units: res_units,
            unit_ranges,
            sections,
        })
    }

    /// The dwarf sections associated with this `Context`.
    pub fn dwarf(&self) -> &gimli::Dwarf<R> {
        &self.sections
    }

    // Finds the CU for the function address given.
    // The index in the CU's function address table (`Functions::addresses`)
    // is also returned because this is calculated here to ensure the function
    // address actually resides in the functions of the CU.
    fn find_unit_and_address(&self, probe: u64) -> Option<(&ResUnit<R>, usize)> {
        // First up find the position in the array which could have our function
        // address.
        let pos = match self
            .unit_ranges
            .binary_search_by_key(&probe, |i| i.range.begin)
        {
            // Although unlikely, we could find an exact match.
            Ok(i) => i + 1,
            // No exact match was found, but this probe would fit at slot `i`.
            // This means that slot `i` is bigger than `probe`, along with all
            // indices greater than `i`, so we need to search all previous
            // entries.
            Err(i) => i,
        };

        // Once we have our index we iterate backwards from that position
        // looking for a matching CU.
        for i in self.unit_ranges[..pos].iter().rev() {
            // We know that this CU's start is beneath the probe already because
            // of our sorted array.
            debug_assert!(i.range.begin <= probe);

            // Each entry keeps track of the maximum end address seen so far,
            // starting from the beginning of the array of unit ranges. We're
            // iterating in reverse so if our probe is beyond the maximum range
            // of this entry, then it's guaranteed to not fit in any prior
            // entries, so we break out.
            if probe > i.max_end {
                break;
            }

            // If this CU doesn't actually contain this address, move to the
            // next CU.
            if probe > i.range.end {
                continue;
            }

            // There might be multiple CUs whose range contains this address.
            // Weak symbols have shown up in the wild which cause this to happen
            // but otherwise this happened in rust-lang/backtrace-rs#327 too. In
            // any case we assume that might happen, and as a result we need to
            // find a CU which actually contains this function.
            //
            // Consequently we consult the function address table here, and only
            // if there's actually a function in this CU which contains this
            // address do we return this unit.
            let unit = &self.units[i.unit_id];
            let funcs = match unit.parse_functions(&self.sections) {
                Ok(func) => func,
                Err(_) => continue,
            };
            if let Some(addr) = funcs.find_address(probe) {
                return Some((unit, addr));
            }
        }

        None
    }

    /// Find the DWARF unit corresponding to the given virtual memory address.
    pub fn find_dwarf_unit(&self, probe: u64) -> Option<&gimli::Unit<R>> {
        self.find_unit_and_address(probe)
            .map(|(unit, _)| &unit.dw_unit)
    }

    /// Find the source file and line corresponding to the given virtual memory address.
    pub fn find_location(&self, probe: u64) -> Result<Option<Location<'_>>, Error> {
        match self.find_unit_and_address(probe) {
            Some((unit, _)) => unit.find_location(probe, &self.sections),
            None => Ok(None),
        }
    }

    /// Return an iterator for the function frames corresponding to the given virtual
    /// memory address.
    ///
    /// If the probe address is not for an inline function then only one frame is
    /// returned.
    ///
    /// If the probe address is for an inline function then the first frame corresponds
    /// to the innermost inline function.  Subsequent frames contain the caller and call
    /// location, until an non-inline caller is reached.
    pub fn find_frames(&self, probe: u64) -> Result<FrameIter<R>, Error> {
        let (unit, address) = match self.find_unit_and_address(probe) {
            Some(x) => x,
            None => return Ok(FrameIter(FrameIterState::Empty)),
        };

        let loc = unit.find_location(probe, &self.sections)?;
        let functions = unit.parse_functions(&self.sections)?;
        let function_index = functions.addresses[address].function;
        let function = &functions.functions[function_index];
        let function = function
            .1
            .borrow_with(|| Function::parse(function.0, &unit.dw_unit, &self.sections, &self.units))
            .as_ref()
            .map_err(Error::clone)?;

        // Build the list of inlined functions that contain `probe`.
        // `inlined_functions` is ordered from outside to inside.
        let mut inlined_functions = maybe_small::Vec::new();
        let mut inlined_addresses = &function.inlined_addresses[..];
        loop {
            let current_depth = inlined_functions.len();
            // Look up (probe, current_depth) in inline_ranges.
            // `inlined_addresses` is sorted in "breadth-first traversal order", i.e.
            // by `call_depth` first, and then by `range.begin`. See the comment at
            // the sort call for more information about why.
            let search = inlined_addresses.binary_search_by(|range| {
                if range.call_depth > current_depth {
                    Ordering::Greater
                } else if range.call_depth < current_depth {
                    Ordering::Less
                } else if range.range.begin > probe {
                    Ordering::Greater
                } else if range.range.end <= probe {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            });
            if let Ok(index) = search {
                let function_index = inlined_addresses[index].function;
                inlined_functions.push(&function.inlined_functions[function_index]);
                inlined_addresses = &inlined_addresses[index + 1..];
            } else {
                break;
            }
        }

        Ok(FrameIter(FrameIterState::Frames(FrameIterFrames {
            unit,
            sections: &self.sections,
            function,
            inlined_functions: inlined_functions.into_iter().rev(),
            next: loc,
        })))
    }

    /// Initialize all line data structures. This is used for benchmarks.
    #[doc(hidden)]
    pub fn parse_lines(&self) -> Result<(), Error> {
        for unit in &self.units {
            unit.parse_lines(&self.sections)?;
        }
        Ok(())
    }

    /// Initialize all function data structures. This is used for benchmarks.
    #[doc(hidden)]
    pub fn parse_functions(&self) -> Result<(), Error> {
        for unit in &self.units {
            unit.parse_functions(&self.sections)?;
        }
        Ok(())
    }

    /// Initialize all inlined function data structures. This is used for benchmarks.
    #[doc(hidden)]
    pub fn parse_inlined_functions(&self) -> Result<(), Error> {
        for unit in &self.units {
            unit.parse_inlined_functions(&self.sections, &self.units)?;
        }
        Ok(())
    }
}

struct Lines {
    files: Box<[String]>,
    sequences: Box<[LineSequence]>,
}

struct LineSequence {
    start: u64,
    end: u64,
    rows: Box<[LineRow]>,
}

struct LineRow {
    address: u64,
    file_index: u64,
    line: u32,
    column: u32,
}

struct ResUnit<R>
where
    R: gimli::Reader,
{
    offset: gimli::DebugInfoOffset<R::Offset>,
    dw_unit: gimli::Unit<R>,
    lang: Option<gimli::DwLang>,
    lines: LazyCell<Result<Lines, Error>>,
    funcs: LazyCell<Result<Functions<R>, Error>>,
}

impl<R> ResUnit<R>
where
    R: gimli::Reader,
{
    fn parse_lines(&self, sections: &gimli::Dwarf<R>) -> Result<Option<&Lines>, Error> {
        let ilnp = match self.dw_unit.line_program {
            Some(ref ilnp) => ilnp,
            None => return Ok(None),
        };
        self.lines
            .borrow_with(|| {
                let mut sequences = Vec::new();
                let mut sequence_rows = Vec::<LineRow>::new();
                let mut rows = ilnp.clone().rows();
                while let Some((_, row)) = rows.next_row()? {
                    if row.end_sequence() {
                        if let Some(start) = sequence_rows.first().map(|x| x.address) {
                            let end = row.address();
                            let mut rows = Vec::new();
                            mem::swap(&mut rows, &mut sequence_rows);
                            sequences.push(LineSequence {
                                start,
                                end,
                                rows: rows.into_boxed_slice(),
                            });
                        }
                        continue;
                    }

                    let address = row.address();
                    let file_index = row.file_index();
                    let line = row.line().unwrap_or(0) as u32;
                    let column = match row.column() {
                        gimli::ColumnType::LeftEdge => 0,
                        gimli::ColumnType::Column(x) => x as u32,
                    };

                    if let Some(last_row) = sequence_rows.last_mut() {
                        if last_row.address == address {
                            last_row.file_index = file_index;
                            last_row.line = line;
                            last_row.column = column;
                            continue;
                        }
                    }

                    sequence_rows.push(LineRow {
                        address,
                        file_index,
                        line,
                        column,
                    });
                }
                sequences.sort_by_key(|x| x.start);

                let mut files = Vec::new();
                let mut index = 0;
                let header = ilnp.header();
                while let Some(file) = header.file(index) {
                    files.push(self.render_file(file, header, sections)?);
                    index += 1;
                }

                Ok(Lines {
                    files: files.into_boxed_slice(),
                    sequences: sequences.into_boxed_slice(),
                })
            })
            .as_ref()
            .map(Some)
            .map_err(Error::clone)
    }

    fn parse_functions(&self, sections: &gimli::Dwarf<R>) -> Result<&Functions<R>, Error> {
        self.funcs
            .borrow_with(|| Functions::parse(&self.dw_unit, sections))
            .as_ref()
            .map_err(Error::clone)
    }

    fn parse_inlined_functions(
        &self,
        sections: &gimli::Dwarf<R>,
        units: &[ResUnit<R>],
    ) -> Result<(), Error> {
        self.funcs
            .borrow_with(|| Functions::parse(&self.dw_unit, sections))
            .as_ref()
            .map_err(Error::clone)?
            .parse_inlined_functions(&self.dw_unit, sections, units)
    }

    fn find_location(
        &self,
        probe: u64,
        sections: &gimli::Dwarf<R>,
    ) -> Result<Option<Location<'_>>, Error> {
        let lines = match self.parse_lines(sections)? {
            Some(lines) => lines,
            None => return Ok(None),
        };

        let idx = lines.sequences.binary_search_by(|sequence| {
            if probe < sequence.start {
                Ordering::Greater
            } else if probe >= sequence.end {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        });
        let idx = match idx {
            Ok(x) => x,
            Err(_) => return Ok(None),
        };
        let sequence = &lines.sequences[idx];

        let idx = sequence
            .rows
            .binary_search_by(|row| row.address.cmp(&probe));
        let idx = match idx {
            Ok(x) => x,
            Err(0) => return Ok(None),
            Err(x) => x - 1,
        };
        let row = &sequence.rows[idx];

        let file = lines.files.get(row.file_index as usize).map(String::as_str);
        Ok(Some(Location {
            file,
            line: if row.line != 0 { Some(row.line) } else { None },
            column: if row.column != 0 {
                Some(row.column)
            } else {
                None
            },
        }))
    }

    fn render_file(
        &self,
        file: &gimli::FileEntry<R, R::Offset>,
        header: &gimli::LineProgramHeader<R, R::Offset>,
        sections: &gimli::Dwarf<R>,
    ) -> Result<String, gimli::Error> {
        let mut path = if let Some(ref comp_dir) = self.dw_unit.comp_dir {
            comp_dir.to_string_lossy()?.into_owned()
        } else {
            String::new()
        };

        if let Some(directory) = file.directory(header) {
            path_push(
                &mut path,
                sections
                    .attr_string(&self.dw_unit, directory)?
                    .to_string_lossy()?
                    .as_ref(),
            );
        }

        path_push(
            &mut path,
            sections
                .attr_string(&self.dw_unit, file.path_name())?
                .to_string_lossy()?
                .as_ref(),
        );

        Ok(path)
    }
}

fn path_push(path: &mut String, p: &str) {
    if p.starts_with('/') {
        *path = p.to_string();
    } else {
        if !path.ends_with('/') {
            path.push('/');
        }
        *path += p;
    }
}

fn name_attr<'abbrev, 'unit, R>(
    attr: gimli::AttributeValue<R>,
    unit: &gimli::Unit<R>,
    sections: &gimli::Dwarf<R>,
    units: &[ResUnit<R>],
    recursion_limit: usize,
) -> Result<Option<R>, Error>
where
    R: gimli::Reader,
{
    if recursion_limit == 0 {
        return Ok(None);
    }

    let mut entries = match attr {
        gimli::AttributeValue::UnitRef(offset) => unit.entries_raw(Some(offset))?,
        gimli::AttributeValue::DebugInfoRef(dr) => {
            let unit = match units.binary_search_by_key(&dr.0, |unit| unit.offset.0) {
                // There is never a DIE at the unit offset or before the first unit.
                Ok(_) | Err(0) => return Err(gimli::Error::NoEntryAtGivenOffset),
                Err(i) => &units[i - 1],
            };
            unit.dw_unit
                .entries_raw(Some(gimli::UnitOffset(dr.0 - unit.offset.0)))?
        }
        _ => return Ok(None),
    };

    let abbrev = if let Some(abbrev) = entries.read_abbreviation()? {
        abbrev
    } else {
        return Err(gimli::Error::NoEntryAtGivenOffset);
    };

    let mut name = None;
    let mut next = None;
    for spec in abbrev.attributes() {
        match entries.read_attribute(*spec) {
            Ok(ref attr) => match attr.name() {
                gimli::DW_AT_linkage_name | gimli::DW_AT_MIPS_linkage_name => {
                    if let Ok(val) = sections.attr_string(unit, attr.value()) {
                        return Ok(Some(val));
                    }
                }
                gimli::DW_AT_name => {
                    if let Ok(val) = sections.attr_string(unit, attr.value()) {
                        name = Some(val);
                    }
                }
                gimli::DW_AT_abstract_origin | gimli::DW_AT_specification => {
                    next = Some(attr.value());
                }
                _ => {}
            },
            Err(e) => return Err(e),
        }
    }

    if name.is_some() {
        return Ok(name);
    }

    if let Some(next) = next {
        return name_attr(next, unit, sections, units, recursion_limit - 1);
    }

    Ok(None)
}

struct Functions<R: gimli::Reader> {
    /// List of all `DW_TAG_subprogram` details in the unit.
    functions: Box<
        [(
            gimli::UnitOffset<R::Offset>,
            LazyCell<Result<Function<R>, Error>>,
        )],
    >,
    /// List of `DW_TAG_subprogram` address ranges in the unit.
    addresses: Box<[FunctionAddress]>,
}

/// A single address range for a function.
///
/// It is possible for a function to have multiple address ranges; this
/// is handled by having multiple `FunctionAddress` entries with the same
/// `function` field.
struct FunctionAddress {
    range: gimli::Range,
    /// An index into `Functions::functions`.
    function: usize,
}

struct Function<R: gimli::Reader> {
    dw_die_offset: gimli::UnitOffset<R::Offset>,
    name: Option<R>,
    /// List of all `DW_TAG_inlined_subroutine` details in this function.
    inlined_functions: Box<[InlinedFunction<R>]>,
    /// List of `DW_TAG_inlined_subroutine` address ranges in this function.
    inlined_addresses: Box<[InlinedFunctionAddress]>,
}

struct InlinedFunctionAddress {
    range: gimli::Range,
    call_depth: usize,
    /// An index into `Function::inlined_functions`.
    function: usize,
}

struct InlinedFunction<R: gimli::Reader> {
    dw_die_offset: gimli::UnitOffset<R::Offset>,
    name: Option<R>,
    call_file: u64,
    call_line: u32,
    call_column: u32,
}

impl<R: gimli::Reader> Functions<R> {
    fn parse(unit: &gimli::Unit<R>, sections: &gimli::Dwarf<R>) -> Result<Functions<R>, Error> {
        let mut functions = Vec::new();
        let mut addresses = Vec::new();
        let mut entries = unit.entries_raw(None)?;
        while !entries.is_empty() {
            let dw_die_offset = entries.next_offset();
            if let Some(abbrev) = entries.read_abbreviation()? {
                if abbrev.tag() == gimli::DW_TAG_subprogram {
                    let mut ranges = RangeAttributes::default();
                    for spec in abbrev.attributes() {
                        match entries.read_attribute(*spec) {
                            Ok(ref attr) => {
                                match attr.name() {
                                    gimli::DW_AT_low_pc => {
                                        if let gimli::AttributeValue::Addr(val) = attr.value() {
                                            ranges.low_pc = Some(val);
                                        }
                                    }
                                    gimli::DW_AT_high_pc => match attr.value() {
                                        gimli::AttributeValue::Addr(val) => {
                                            ranges.high_pc = Some(val)
                                        }
                                        gimli::AttributeValue::Udata(val) => {
                                            ranges.size = Some(val)
                                        }
                                        _ => {}
                                    },
                                    gimli::DW_AT_ranges => {
                                        ranges.ranges_offset =
                                            sections.attr_ranges_offset(unit, attr.value())?;
                                    }
                                    _ => {}
                                };
                            }
                            Err(e) => return Err(e),
                        }
                    }

                    let function_index = functions.len();
                    if ranges.for_each_range(sections, unit, |range| {
                        addresses.push(FunctionAddress {
                            range,
                            function: function_index,
                        });
                    })? {
                        functions.push((dw_die_offset, LazyCell::new()));
                    }
                } else {
                    for spec in abbrev.attributes() {
                        match entries.read_attribute(*spec) {
                            Ok(_) => {}
                            Err(e) => return Err(e),
                        }
                    }
                }
            }
        }

        // The binary search requires the addresses to be sorted.
        //
        // It also requires them to be non-overlapping.  In practice, overlapping
        // function ranges are unlikely, so we don't try to handle that yet.
        //
        // It's possible for multiple functions to have the same address range if the
        // compiler can detect and remove functions with identical code.  In that case
        // we'll nondeterministically return one of them.
        addresses.sort_by_key(|x| x.range.begin);

        Ok(Functions {
            functions: functions.into_boxed_slice(),
            addresses: addresses.into_boxed_slice(),
        })
    }

    fn find_address(&self, probe: u64) -> Option<usize> {
        self.addresses
            .binary_search_by(|address| {
                if probe < address.range.begin {
                    Ordering::Greater
                } else if probe >= address.range.end {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            })
            .ok()
    }

    fn parse_inlined_functions(
        &self,
        unit: &gimli::Unit<R>,
        sections: &gimli::Dwarf<R>,
        units: &[ResUnit<R>],
    ) -> Result<(), Error> {
        for function in &*self.functions {
            function
                .1
                .borrow_with(|| Function::parse(function.0, unit, sections, units))
                .as_ref()
                .map_err(Error::clone)?;
        }
        Ok(())
    }
}

impl<R: gimli::Reader> Function<R> {
    fn parse(
        dw_die_offset: gimli::UnitOffset<R::Offset>,
        unit: &gimli::Unit<R>,
        sections: &gimli::Dwarf<R>,
        units: &[ResUnit<R>],
    ) -> Result<Self, Error> {
        let mut entries = unit.entries_raw(Some(dw_die_offset))?;
        let depth = entries.next_depth();
        let abbrev = entries.read_abbreviation()?.unwrap();
        debug_assert_eq!(abbrev.tag(), gimli::DW_TAG_subprogram);

        let mut name = None;
        for spec in abbrev.attributes() {
            match entries.read_attribute(*spec) {
                Ok(ref attr) => {
                    match attr.name() {
                        gimli::DW_AT_linkage_name | gimli::DW_AT_MIPS_linkage_name => {
                            if let Ok(val) = sections.attr_string(unit, attr.value()) {
                                name = Some(val);
                            }
                        }
                        gimli::DW_AT_name => {
                            if name.is_none() {
                                name = sections.attr_string(unit, attr.value()).ok();
                            }
                        }
                        gimli::DW_AT_abstract_origin | gimli::DW_AT_specification => {
                            if name.is_none() {
                                name = name_attr(attr.value(), unit, sections, units, 16)?;
                            }
                        }
                        _ => {}
                    };
                }
                Err(e) => return Err(e),
            }
        }

        let mut inlined_functions = Vec::new();
        let mut inlined_addresses = Vec::new();
        Function::parse_children(
            &mut entries,
            depth,
            unit,
            sections,
            units,
            &mut inlined_functions,
            &mut inlined_addresses,
            0,
        )?;

        // Sort ranges in "breadth-first traversal order", i.e. first by call_depth
        // and then by range.begin. This allows finding the range containing an
        // address at a certain depth using binary search.
        // Note: Using DFS order, i.e. ordering by range.begin first and then by
        // call_depth, would not work! Consider the two examples
        // "[0..10 at depth 0], [0..2 at depth 1], [6..8 at depth 1]"  and
        // "[0..5 at depth 0], [0..2 at depth 1], [5..10 at depth 0], [6..8 at depth 1]".
        // In this example, if you want to look up address 7 at depth 0, and you
        // encounter [0..2 at depth 1], are you before or after the target range?
        // You don't know.
        inlined_addresses.sort_by(|r1, r2| {
            if r1.call_depth < r2.call_depth {
                Ordering::Less
            } else if r1.call_depth > r2.call_depth {
                Ordering::Greater
            } else if r1.range.begin < r2.range.begin {
                Ordering::Less
            } else if r1.range.begin > r2.range.begin {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        });

        Ok(Function {
            dw_die_offset,
            name,
            inlined_functions: inlined_functions.into_boxed_slice(),
            inlined_addresses: inlined_addresses.into_boxed_slice(),
        })
    }

    fn parse_children(
        entries: &mut gimli::EntriesRaw<R>,
        depth: isize,
        unit: &gimli::Unit<R>,
        sections: &gimli::Dwarf<R>,
        units: &[ResUnit<R>],
        inlined_functions: &mut Vec<InlinedFunction<R>>,
        inlined_addresses: &mut Vec<InlinedFunctionAddress>,
        inlined_depth: usize,
    ) -> Result<(), Error> {
        loop {
            let dw_die_offset = entries.next_offset();
            let next_depth = entries.next_depth();
            if next_depth <= depth {
                return Ok(());
            }
            if let Some(abbrev) = entries.read_abbreviation()? {
                match abbrev.tag() {
                    gimli::DW_TAG_subprogram => {
                        Function::skip(entries, abbrev, next_depth)?;
                    }
                    gimli::DW_TAG_inlined_subroutine => {
                        InlinedFunction::parse(
                            dw_die_offset,
                            entries,
                            abbrev,
                            next_depth,
                            unit,
                            sections,
                            units,
                            inlined_functions,
                            inlined_addresses,
                            inlined_depth,
                        )?;
                    }
                    _ => {
                        for spec in abbrev.attributes() {
                            match entries.read_attribute(*spec) {
                                Ok(_) => {}
                                Err(e) => return Err(e),
                            }
                        }
                    }
                }
            }
        }
    }

    fn skip(
        entries: &mut gimli::EntriesRaw<R>,
        abbrev: &gimli::Abbreviation,
        depth: isize,
    ) -> Result<(), Error> {
        // TODO: use DW_AT_sibling
        for spec in abbrev.attributes() {
            match entries.read_attribute(*spec) {
                Ok(_) => {}
                Err(e) => return Err(e),
            }
        }
        while entries.next_depth() > depth {
            if let Some(abbrev) = entries.read_abbreviation()? {
                for spec in abbrev.attributes() {
                    match entries.read_attribute(*spec) {
                        Ok(_) => {}
                        Err(e) => return Err(e),
                    }
                }
            }
        }
        Ok(())
    }
}

impl<R: gimli::Reader> InlinedFunction<R> {
    fn parse(
        dw_die_offset: gimli::UnitOffset<R::Offset>,
        entries: &mut gimli::EntriesRaw<R>,
        abbrev: &gimli::Abbreviation,
        depth: isize,
        unit: &gimli::Unit<R>,
        sections: &gimli::Dwarf<R>,
        units: &[ResUnit<R>],
        inlined_functions: &mut Vec<InlinedFunction<R>>,
        inlined_addresses: &mut Vec<InlinedFunctionAddress>,
        inlined_depth: usize,
    ) -> Result<(), Error> {
        let mut ranges = RangeAttributes::default();
        let mut name = None;
        let mut call_file = 0;
        let mut call_line = 0;
        let mut call_column = 0;
        for spec in abbrev.attributes() {
            match entries.read_attribute(*spec) {
                Ok(ref attr) => match attr.name() {
                    gimli::DW_AT_low_pc => {
                        if let gimli::AttributeValue::Addr(val) = attr.value() {
                            ranges.low_pc = Some(val);
                        }
                    }
                    gimli::DW_AT_high_pc => match attr.value() {
                        gimli::AttributeValue::Addr(val) => ranges.high_pc = Some(val),
                        gimli::AttributeValue::Udata(val) => ranges.size = Some(val),
                        _ => {}
                    },
                    gimli::DW_AT_ranges => {
                        ranges.ranges_offset = sections.attr_ranges_offset(unit, attr.value())?;
                    }
                    gimli::DW_AT_linkage_name | gimli::DW_AT_MIPS_linkage_name => {
                        if let Ok(val) = sections.attr_string(unit, attr.value()) {
                            name = Some(val);
                        }
                    }
                    gimli::DW_AT_name => {
                        if name.is_none() {
                            name = sections.attr_string(unit, attr.value()).ok();
                        }
                    }
                    gimli::DW_AT_abstract_origin | gimli::DW_AT_specification => {
                        if name.is_none() {
                            name = name_attr(attr.value(), unit, sections, units, 16)?;
                        }
                    }
                    gimli::DW_AT_call_file => {
                        if let gimli::AttributeValue::FileIndex(fi) = attr.value() {
                            call_file = fi;
                        }
                    }
                    gimli::DW_AT_call_line => {
                        call_line = attr.udata_value().unwrap_or(0) as u32;
                    }
                    gimli::DW_AT_call_column => {
                        call_column = attr.udata_value().unwrap_or(0) as u32;
                    }
                    _ => {}
                },
                Err(e) => return Err(e),
            }
        }

        let function_index = inlined_functions.len();
        inlined_functions.push(InlinedFunction {
            dw_die_offset,
            name,
            call_file,
            call_line,
            call_column,
        });

        ranges.for_each_range(sections, unit, |range| {
            inlined_addresses.push(InlinedFunctionAddress {
                range,
                call_depth: inlined_depth,
                function: function_index,
            });
        })?;

        Function::parse_children(
            entries,
            depth,
            unit,
            sections,
            units,
            inlined_functions,
            inlined_addresses,
            inlined_depth + 1,
        )
    }
}

struct RangeAttributes<R: gimli::Reader> {
    low_pc: Option<u64>,
    high_pc: Option<u64>,
    size: Option<u64>,
    ranges_offset: Option<gimli::RangeListsOffset<<R as gimli::Reader>::Offset>>,
}

impl<R: gimli::Reader> Default for RangeAttributes<R> {
    fn default() -> Self {
        RangeAttributes {
            low_pc: None,
            high_pc: None,
            size: None,
            ranges_offset: None,
        }
    }
}

impl<R: gimli::Reader> RangeAttributes<R> {
    fn for_each_range<F: FnMut(gimli::Range)>(
        &self,
        sections: &gimli::Dwarf<R>,
        unit: &gimli::Unit<R>,
        mut f: F,
    ) -> Result<bool, Error> {
        let mut added_any = false;
        let mut add_range = |range: gimli::Range| {
            if range.begin < range.end {
                f(range);
                added_any = true
            }
        };
        if let Some(ranges_offset) = self.ranges_offset {
            let mut range_list = sections.ranges(unit, ranges_offset)?;
            while let Some(range) = range_list.next()? {
                add_range(range);
            }
        } else if let (Some(begin), Some(end)) = (self.low_pc, self.high_pc) {
            add_range(gimli::Range { begin, end });
        } else if let (Some(begin), Some(size)) = (self.low_pc, self.size) {
            add_range(gimli::Range {
                begin,
                end: begin + size,
            });
        }
        Ok(added_any)
    }
}

/// An iterator over function frames.
pub struct FrameIter<'ctx, R>(FrameIterState<'ctx, R>)
where
    R: gimli::Reader + 'ctx;

enum FrameIterState<'ctx, R>
where
    R: gimli::Reader + 'ctx,
{
    Empty,
    Frames(FrameIterFrames<'ctx, R>),
}

struct FrameIterFrames<'ctx, R>
where
    R: gimli::Reader + 'ctx,
{
    unit: &'ctx ResUnit<R>,
    sections: &'ctx gimli::Dwarf<R>,
    function: &'ctx Function<R>,
    inlined_functions: iter::Rev<maybe_small::IntoIter<&'ctx InlinedFunction<R>>>,
    next: Option<Location<'ctx>>,
}

impl<'ctx, R> FrameIter<'ctx, R>
where
    R: gimli::Reader + 'ctx,
{
    /// Advances the iterator and returns the next frame.
    pub fn next(&mut self) -> Result<Option<Frame<'ctx, R>>, Error> {
        let frames = match &mut self.0 {
            FrameIterState::Empty => return Ok(None),
            FrameIterState::Frames(frames) => frames,
        };

        let loc = frames.next.take();
        let func = match frames.inlined_functions.next() {
            Some(func) => func,
            None => {
                let frame = Frame {
                    dw_die_offset: Some(frames.function.dw_die_offset),
                    function: frames.function.name.clone().map(|name| FunctionName {
                        name,
                        language: frames.unit.lang,
                    }),
                    location: loc,
                };
                self.0 = FrameIterState::Empty;
                return Ok(Some(frame));
            }
        };

        let mut next = Location {
            file: None,
            line: if func.call_line != 0 {
                Some(func.call_line)
            } else {
                None
            },
            column: if func.call_column != 0 {
                Some(func.call_column)
            } else {
                None
            },
        };
        if func.call_file != 0 {
            if let Some(lines) = frames.unit.parse_lines(frames.sections)? {
                next.file = lines.files.get(func.call_file as usize).map(String::as_str);
            }
        }
        frames.next = Some(next);

        Ok(Some(Frame {
            dw_die_offset: Some(func.dw_die_offset),
            function: func.name.clone().map(|name| FunctionName {
                name,
                language: frames.unit.lang,
            }),
            location: loc,
        }))
    }
}

#[cfg(feature = "fallible-iterator")]
impl<'ctx, R> fallible_iterator::FallibleIterator for FrameIter<'ctx, R>
where
    R: gimli::Reader + 'ctx,
{
    type Item = Frame<'ctx, R>;
    type Error = Error;

    #[inline]
    fn next(&mut self) -> Result<Option<Frame<'ctx, R>>, Error> {
        self.next()
    }
}

/// A function frame.
pub struct Frame<'ctx, R: gimli::Reader> {
    /// The DWARF unit offset corresponding to the DIE of the function.
    pub dw_die_offset: Option<gimli::UnitOffset<R::Offset>>,
    /// The name of the function.
    pub function: Option<FunctionName<R>>,
    /// The source location corresponding to this frame.
    pub location: Option<Location<'ctx>>,
}

/// A function name.
pub struct FunctionName<R: gimli::Reader> {
    /// The name of the function.
    pub name: R,
    /// The language of the compilation unit containing this function.
    pub language: Option<gimli::DwLang>,
}

impl<R: gimli::Reader> FunctionName<R> {
    /// The raw name of this function before demangling.
    pub fn raw_name(&self) -> Result<Cow<str>, Error> {
        self.name.to_string_lossy()
    }

    /// The name of this function after demangling (if applicable).
    pub fn demangle(&self) -> Result<Cow<str>, Error> {
        self.raw_name().map(|x| demangle_auto(x, self.language))
    }
}

/// Demangle a symbol name using the demangling scheme for the given language.
///
/// Returns `None` if demangling failed or is not required.
#[allow(unused_variables)]
pub fn demangle(name: &str, language: gimli::DwLang) -> Option<String> {
    match language {
        #[cfg(feature = "rustc-demangle")]
        gimli::DW_LANG_Rust => rustc_demangle::try_demangle(name)
            .ok()
            .as_ref()
            .map(|x| format!("{:#}", x)),
        #[cfg(feature = "cpp_demangle")]
        gimli::DW_LANG_C_plus_plus
        | gimli::DW_LANG_C_plus_plus_03
        | gimli::DW_LANG_C_plus_plus_11
        | gimli::DW_LANG_C_plus_plus_14 => cpp_demangle::Symbol::new(name)
            .ok()
            .and_then(|x| x.demangle(&Default::default()).ok()),
        _ => None,
    }
}

/// Apply 'best effort' demangling of a symbol name.
///
/// If `language` is given, then only the demangling scheme for that language
/// is used.
///
/// If `language` is `None`, then heuristics are used to determine how to
/// demangle the name. Currently, these heuristics are very basic.
///
/// If demangling fails or is not required, then `name` is returned unchanged.
pub fn demangle_auto(name: Cow<str>, language: Option<gimli::DwLang>) -> Cow<str> {
    match language {
        Some(language) => demangle(name.as_ref(), language),
        None => demangle(name.as_ref(), gimli::DW_LANG_Rust)
            .or_else(|| demangle(name.as_ref(), gimli::DW_LANG_C_plus_plus)),
    }
    .map(Cow::from)
    .unwrap_or(name)
}

/// A source location.
pub struct Location<'a> {
    /// The file name.
    pub file: Option<&'a str>,
    /// The line number.
    pub line: Option<u32>,
    /// The column number.
    pub column: Option<u32>,
}
