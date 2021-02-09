use alloc::string::String;

use crate::common::{
    DebugAddrBase, DebugAddrIndex, DebugInfoOffset, DebugLineStrOffset, DebugLocListsBase,
    DebugLocListsIndex, DebugRngListsBase, DebugRngListsIndex, DebugStrOffset, DebugStrOffsetsBase,
    DebugStrOffsetsIndex, DebugTypesOffset, Encoding, LocationListsOffset, RangeListsOffset,
    SectionId, UnitSectionOffset,
};
use crate::constants;
use crate::read::{
    Abbreviations, AttributeValue, CompilationUnitHeader, CompilationUnitHeadersIter, DebugAbbrev,
    DebugAddr, DebugInfo, DebugLine, DebugLineStr, DebugStr, DebugStrOffsets, DebugTypes,
    DebuggingInformationEntry, EntriesCursor, EntriesRaw, EntriesTree, Error,
    IncompleteLineProgram, LocListIter, LocationLists, Range, RangeLists, Reader, ReaderOffset,
    ReaderOffsetId, Result, RngListIter, Section, TypeUnitHeader, TypeUnitHeadersIter, UnitHeader,
    UnitOffset,
};

/// All of the commonly used DWARF sections, and other common information.
#[derive(Debug, Default)]
pub struct Dwarf<R> {
    /// The `.debug_abbrev` section.
    pub debug_abbrev: DebugAbbrev<R>,

    /// The `.debug_addr` section.
    pub debug_addr: DebugAddr<R>,

    /// The `.debug_info` section.
    pub debug_info: DebugInfo<R>,

    /// The `.debug_line` section.
    pub debug_line: DebugLine<R>,

    /// The `.debug_line_str` section.
    pub debug_line_str: DebugLineStr<R>,

    /// The `.debug_str` section.
    pub debug_str: DebugStr<R>,

    /// The `.debug_str_offsets` section.
    pub debug_str_offsets: DebugStrOffsets<R>,

    /// The `.debug_str` section for a supplementary object file.
    pub debug_str_sup: DebugStr<R>,

    /// The `.debug_types` section.
    pub debug_types: DebugTypes<R>,

    /// The location lists in the `.debug_loc` and `.debug_loclists` sections.
    pub locations: LocationLists<R>,

    /// The range lists in the `.debug_ranges` and `.debug_rnglists` sections.
    pub ranges: RangeLists<R>,
}

impl<T> Dwarf<T> {
    /// Try to load the DWARF sections using the given loader functions.
    ///
    /// `section` loads a DWARF section from the main object file.
    /// `sup` loads a DWARF sections from the supplementary object file.
    /// These functions should return an empty section if the section does not exist.
    ///
    /// The provided callback functions may either directly return a `Reader` instance
    /// (such as `EndianSlice`), or they may return some other type and then convert
    /// that type into a `Reader` using `Dwarf::borrow`.
    pub fn load<F1, F2, E>(mut section: F1, mut sup: F2) -> core::result::Result<Self, E>
    where
        F1: FnMut(SectionId) -> core::result::Result<T, E>,
        F2: FnMut(SectionId) -> core::result::Result<T, E>,
    {
        // Section types are inferred.
        let debug_loc = Section::load(&mut section)?;
        let debug_loclists = Section::load(&mut section)?;
        let debug_ranges = Section::load(&mut section)?;
        let debug_rnglists = Section::load(&mut section)?;
        Ok(Dwarf {
            debug_abbrev: Section::load(&mut section)?,
            debug_addr: Section::load(&mut section)?,
            debug_info: Section::load(&mut section)?,
            debug_line: Section::load(&mut section)?,
            debug_line_str: Section::load(&mut section)?,
            debug_str: Section::load(&mut section)?,
            debug_str_offsets: Section::load(&mut section)?,
            debug_str_sup: Section::load(&mut sup)?,
            debug_types: Section::load(&mut section)?,
            locations: LocationLists::new(debug_loc, debug_loclists),
            ranges: RangeLists::new(debug_ranges, debug_rnglists),
        })
    }

    /// Create a `Dwarf` structure that references the data in `self`.
    ///
    /// This is useful when `R` implements `Reader` but `T` does not.
    ///
    /// ## Example Usage
    ///
    /// It can be useful to load DWARF sections into owned data structures,
    /// such as `Vec`. However, we do not implement the `Reader` trait
    /// for `Vec`, because it would be very inefficient, but this trait
    /// is required for all of the methods that parse the DWARF data.
    /// So we first load the DWARF sections into `Vec`s, and then use
    /// `borrow` to create `Reader`s that reference the data.
    ///
    /// ```rust,no_run
    /// # fn example() -> Result<(), gimli::Error> {
    /// # let loader = |name| -> Result<_, gimli::Error> { unimplemented!() };
    /// # let sup_loader = |name| { unimplemented!() };
    /// // Read the DWARF sections into `Vec`s with whatever object loader you're using.
    /// let owned_dwarf: gimli::Dwarf<Vec<u8>> = gimli::Dwarf::load(loader, sup_loader)?;
    /// // Create references to the DWARF sections.
    /// let dwarf = owned_dwarf.borrow(|section| {
    ///     gimli::EndianSlice::new(&section, gimli::LittleEndian)
    /// });
    /// # unreachable!()
    /// # }
    /// ```
    pub fn borrow<'a, F, R>(&'a self, mut borrow: F) -> Dwarf<R>
    where
        F: FnMut(&'a T) -> R,
    {
        Dwarf {
            debug_abbrev: self.debug_abbrev.borrow(&mut borrow),
            debug_addr: self.debug_addr.borrow(&mut borrow),
            debug_info: self.debug_info.borrow(&mut borrow),
            debug_line: self.debug_line.borrow(&mut borrow),
            debug_line_str: self.debug_line_str.borrow(&mut borrow),
            debug_str: self.debug_str.borrow(&mut borrow),
            debug_str_offsets: self.debug_str_offsets.borrow(&mut borrow),
            debug_str_sup: self.debug_str_sup.borrow(&mut borrow),
            debug_types: self.debug_types.borrow(&mut borrow),
            locations: self.locations.borrow(&mut borrow),
            ranges: self.ranges.borrow(&mut borrow),
        }
    }
}

impl<R: Reader> Dwarf<R> {
    /// Iterate the compilation- and partial-unit headers in the
    /// `.debug_info` section.
    ///
    /// Can be [used with
    /// `FallibleIterator`](./index.html#using-with-fallibleiterator).
    #[inline]
    pub fn units(&self) -> CompilationUnitHeadersIter<R> {
        self.debug_info.units()
    }

    /// Construct a new `Unit` from the given compilation unit header.
    #[inline]
    pub fn unit(&self, header: CompilationUnitHeader<R>) -> Result<Unit<R>> {
        Unit::new(self, header)
    }

    /// Iterate the type-unit headers in the `.debug_types` section.
    ///
    /// Can be [used with
    /// `FallibleIterator`](./index.html#using-with-fallibleiterator).
    #[inline]
    pub fn type_units(&self) -> TypeUnitHeadersIter<R> {
        self.debug_types.units()
    }

    /// Construct a new `Unit` from the given type unit header.
    #[inline]
    pub fn type_unit(&self, header: TypeUnitHeader<R>) -> Result<Unit<R>> {
        Unit::new_type_unit(self, header)
    }

    /// Parse the abbreviations for a compilation unit.
    // TODO: provide caching of abbreviations
    #[inline]
    pub fn abbreviations(&self, unit: &CompilationUnitHeader<R>) -> Result<Abbreviations> {
        unit.abbreviations(&self.debug_abbrev)
    }

    /// Parse the abbreviations for a type unit.
    // TODO: provide caching of abbreviations
    #[inline]
    pub fn type_abbreviations(&self, unit: &TypeUnitHeader<R>) -> Result<Abbreviations> {
        unit.abbreviations(&self.debug_abbrev)
    }

    /// Return the string offset at the given index.
    #[inline]
    pub fn string_offset(
        &self,
        unit: &Unit<R>,
        index: DebugStrOffsetsIndex<R::Offset>,
    ) -> Result<DebugStrOffset<R::Offset>> {
        self.debug_str_offsets
            .get_str_offset(unit.header.format(), unit.str_offsets_base, index)
    }

    /// Return the string at the given offset in `.debug_str`.
    #[inline]
    pub fn string(&self, offset: DebugStrOffset<R::Offset>) -> Result<R> {
        self.debug_str.get_str(offset)
    }

    /// Return the string at the given offset in `.debug_line_str`.
    #[inline]
    pub fn line_string(&self, offset: DebugLineStrOffset<R::Offset>) -> Result<R> {
        self.debug_line_str.get_str(offset)
    }

    /// Return an attribute value as a string slice.
    ///
    /// If the attribute value is one of:
    ///
    /// - an inline `DW_FORM_string` string
    /// - a `DW_FORM_strp` reference to an offset into the `.debug_str` section
    /// - a `DW_FORM_strp_sup` reference to an offset into a supplementary
    /// object file
    /// - a `DW_FORM_line_strp` reference to an offset into the `.debug_line_str`
    /// section
    /// - a `DW_FORM_strx` index into the `.debug_str_offsets` entries for the unit
    ///
    /// then return the attribute's string value. Returns an error if the attribute
    /// value does not have a string form, or if a string form has an invalid value.
    pub fn attr_string(&self, unit: &Unit<R>, attr: AttributeValue<R>) -> Result<R> {
        match attr {
            AttributeValue::String(string) => Ok(string),
            AttributeValue::DebugStrRef(offset) => self.debug_str.get_str(offset),
            AttributeValue::DebugStrRefSup(offset) => self.debug_str_sup.get_str(offset),
            AttributeValue::DebugLineStrRef(offset) => self.debug_line_str.get_str(offset),
            AttributeValue::DebugStrOffsetsIndex(index) => {
                let offset = self.debug_str_offsets.get_str_offset(
                    unit.header.format(),
                    unit.str_offsets_base,
                    index,
                )?;
                self.debug_str.get_str(offset)
            }
            _ => Err(Error::ExpectedStringAttributeValue),
        }
    }

    /// Return the address at the given index.
    pub fn address(&self, unit: &Unit<R>, index: DebugAddrIndex<R::Offset>) -> Result<u64> {
        self.debug_addr
            .get_address(unit.encoding().address_size, unit.addr_base, index)
    }

    /// Return the range list offset at the given index.
    pub fn ranges_offset(
        &self,
        unit: &Unit<R>,
        index: DebugRngListsIndex<R::Offset>,
    ) -> Result<RangeListsOffset<R::Offset>> {
        self.ranges
            .get_offset(unit.encoding(), unit.rnglists_base, index)
    }

    /// Iterate over the `RangeListEntry`s starting at the given offset.
    pub fn ranges(
        &self,
        unit: &Unit<R>,
        offset: RangeListsOffset<R::Offset>,
    ) -> Result<RngListIter<R>> {
        self.ranges.ranges(
            offset,
            unit.encoding(),
            unit.low_pc,
            &self.debug_addr,
            unit.addr_base,
        )
    }

    /// Try to return an attribute value as a range list offset.
    ///
    /// If the attribute value is one of:
    ///
    /// - a `DW_FORM_sec_offset` reference to the `.debug_ranges` or `.debug_rnglists` sections
    /// - a `DW_FORM_rnglistx` index into the `.debug_rnglists` entries for the unit
    ///
    /// then return the range list offset of the range list.
    /// Returns `None` for other forms.
    pub fn attr_ranges_offset(
        &self,
        unit: &Unit<R>,
        attr: AttributeValue<R>,
    ) -> Result<Option<RangeListsOffset<R::Offset>>> {
        match attr {
            AttributeValue::RangeListsRef(offset) => Ok(Some(offset)),
            AttributeValue::DebugRngListsIndex(index) => self.ranges_offset(unit, index).map(Some),
            _ => Ok(None),
        }
    }

    /// Try to return an attribute value as a range list entry iterator.
    ///
    /// If the attribute value is one of:
    ///
    /// - a `DW_FORM_sec_offset` reference to the `.debug_ranges` or `.debug_rnglists` sections
    /// - a `DW_FORM_rnglistx` index into the `.debug_rnglists` entries for the unit
    ///
    /// then return an iterator over the entries in the range list.
    /// Returns `None` for other forms.
    pub fn attr_ranges(
        &self,
        unit: &Unit<R>,
        attr: AttributeValue<R>,
    ) -> Result<Option<RngListIter<R>>> {
        match self.attr_ranges_offset(unit, attr)? {
            Some(offset) => Ok(Some(self.ranges(unit, offset)?)),
            None => Ok(None),
        }
    }

    /// Return an iterator for the address ranges of a `DebuggingInformationEntry`.
    ///
    /// This uses `DW_AT_low_pc`, `DW_AT_high_pc` and `DW_AT_ranges`.
    pub fn die_ranges(
        &self,
        unit: &Unit<R>,
        entry: &DebuggingInformationEntry<R>,
    ) -> Result<RangeIter<R>> {
        let mut low_pc = None;
        let mut high_pc = None;
        let mut size = None;
        let mut attrs = entry.attrs();
        while let Some(attr) = attrs.next()? {
            match attr.name() {
                constants::DW_AT_low_pc => {
                    if let AttributeValue::Addr(val) = attr.value() {
                        low_pc = Some(val);
                    }
                }
                constants::DW_AT_high_pc => match attr.value() {
                    AttributeValue::Addr(val) => high_pc = Some(val),
                    AttributeValue::Udata(val) => size = Some(val),
                    _ => return Err(Error::UnsupportedAttributeForm),
                },
                constants::DW_AT_ranges => {
                    if let Some(list) = self.attr_ranges(unit, attr.value())? {
                        return Ok(RangeIter(RangeIterInner::List(list)));
                    }
                }
                _ => {}
            }
        }
        let range = low_pc.and_then(|begin| {
            let end = size.map(|size| begin + size).or(high_pc);
            // TODO: perhaps return an error if `end` is `None`
            end.map(|end| Range { begin, end })
        });
        Ok(RangeIter(RangeIterInner::Single(range)))
    }

    /// Return an iterator for the address ranges of a `Unit`.
    ///
    /// This uses `DW_AT_low_pc`, `DW_AT_high_pc` and `DW_AT_ranges` of the
    /// root `DebuggingInformationEntry`.
    pub fn unit_ranges(&self, unit: &Unit<R>) -> Result<RangeIter<R>> {
        let mut cursor = unit.header.entries(&unit.abbreviations);
        cursor.next_dfs()?;
        let root = cursor.current().ok_or(Error::MissingUnitDie)?;
        self.die_ranges(unit, root)
    }

    /// Return the location list offset at the given index.
    pub fn locations_offset(
        &self,
        unit: &Unit<R>,
        index: DebugLocListsIndex<R::Offset>,
    ) -> Result<LocationListsOffset<R::Offset>> {
        self.locations
            .get_offset(unit.encoding(), unit.loclists_base, index)
    }

    /// Iterate over the `LocationListEntry`s starting at the given offset.
    pub fn locations(
        &self,
        unit: &Unit<R>,
        offset: LocationListsOffset<R::Offset>,
    ) -> Result<LocListIter<R>> {
        self.locations.locations(
            offset,
            unit.encoding(),
            unit.low_pc,
            &self.debug_addr,
            unit.addr_base,
        )
    }

    /// Try to return an attribute value as a location list offset.
    ///
    /// If the attribute value is one of:
    ///
    /// - a `DW_FORM_sec_offset` reference to the `.debug_loc` or `.debug_loclists` sections
    /// - a `DW_FORM_loclistx` index into the `.debug_loclists` entries for the unit
    ///
    /// then return the location list offset of the location list.
    /// Returns `None` for other forms.
    pub fn attr_locations_offset(
        &self,
        unit: &Unit<R>,
        attr: AttributeValue<R>,
    ) -> Result<Option<LocationListsOffset<R::Offset>>> {
        match attr {
            AttributeValue::LocationListsRef(offset) => Ok(Some(offset)),
            AttributeValue::DebugLocListsIndex(index) => {
                self.locations_offset(unit, index).map(Some)
            }
            _ => Ok(None),
        }
    }

    /// Try to return an attribute value as a location list entry iterator.
    ///
    /// If the attribute value is one of:
    ///
    /// - a `DW_FORM_sec_offset` reference to the `.debug_loc` or `.debug_loclists` sections
    /// - a `DW_FORM_loclistx` index into the `.debug_loclists` entries for the unit
    ///
    /// then return an iterator over the entries in the location list.
    /// Returns `None` for other forms.
    pub fn attr_locations(
        &self,
        unit: &Unit<R>,
        attr: AttributeValue<R>,
    ) -> Result<Option<LocListIter<R>>> {
        match self.attr_locations_offset(unit, attr)? {
            Some(offset) => Ok(Some(self.locations(unit, offset)?)),
            None => Ok(None),
        }
    }

    /// Call `Reader::lookup_offset_id` for each section, and return the first match.
    ///
    /// The first element of the tuple is `true` for supplementary sections.
    pub fn lookup_offset_id(&self, id: ReaderOffsetId) -> Option<(bool, SectionId, R::Offset)> {
        None.or_else(|| self.debug_abbrev.lookup_offset_id(id))
            .or_else(|| self.debug_addr.lookup_offset_id(id))
            .or_else(|| self.debug_info.lookup_offset_id(id))
            .or_else(|| self.debug_line.lookup_offset_id(id))
            .or_else(|| self.debug_line_str.lookup_offset_id(id))
            .or_else(|| self.debug_str.lookup_offset_id(id))
            .or_else(|| self.debug_str_offsets.lookup_offset_id(id))
            .or_else(|| self.debug_types.lookup_offset_id(id))
            .or_else(|| self.locations.lookup_offset_id(id))
            .or_else(|| self.ranges.lookup_offset_id(id))
            .map(|(id, offset)| (false, id, offset))
            .or_else(|| {
                self.debug_str_sup
                    .lookup_offset_id(id)
                    .map(|(id, offset)| (true, id, offset))
            })
    }

    /// Returns a string representation of the given error.
    ///
    /// This uses information from the DWARF sections to provide more information in some cases.
    pub fn format_error(&self, err: Error) -> String {
        #[allow(clippy::single_match)]
        match err {
            Error::UnexpectedEof(id) => match self.lookup_offset_id(id) {
                Some((sup, section, offset)) => {
                    return format!(
                        "{} at {}{}+0x{:x}",
                        err,
                        section.name(),
                        if sup { "(sup)" } else { "" },
                        offset.into_u64(),
                    );
                }
                None => {}
            },
            _ => {}
        }
        err.description().into()
    }
}

/// All of the commonly used information for a unit in the `.debug_info` or `.debug_types`
/// sections.
#[derive(Debug)]
pub struct Unit<R, Offset = <R as Reader>::Offset>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    /// The section offset of the unit.
    pub offset: UnitSectionOffset<Offset>,

    /// The header of the unit.
    pub header: UnitHeader<R, Offset>,

    /// The parsed abbreviations for the unit.
    pub abbreviations: Abbreviations,

    /// The `DW_AT_name` attribute of the unit.
    pub name: Option<R>,

    /// The `DW_AT_comp_dir` attribute of the unit.
    pub comp_dir: Option<R>,

    /// The `DW_AT_low_pc` attribute of the unit. Defaults to 0.
    pub low_pc: u64,

    /// The `DW_AT_str_offsets_base` attribute of the unit. Defaults to 0.
    pub str_offsets_base: DebugStrOffsetsBase<Offset>,

    /// The `DW_AT_addr_base` attribute of the unit. Defaults to 0.
    pub addr_base: DebugAddrBase<Offset>,

    /// The `DW_AT_loclists_base` attribute of the unit. Defaults to 0.
    pub loclists_base: DebugLocListsBase<Offset>,

    /// The `DW_AT_rnglists_base` attribute of the unit. Defaults to 0.
    pub rnglists_base: DebugRngListsBase<Offset>,

    /// The line number program of the unit.
    pub line_program: Option<IncompleteLineProgram<R, Offset>>,
}

impl<R: Reader> Unit<R> {
    /// Construct a new `Unit` from the given compilation unit header.
    #[inline]
    pub fn new(dwarf: &Dwarf<R>, header: CompilationUnitHeader<R>) -> Result<Self> {
        Self::new_internal(
            dwarf,
            UnitSectionOffset::DebugInfoOffset(header.offset()),
            header.header(),
        )
    }

    /// Construct a new `Unit` from the given type unit header.
    #[inline]
    pub fn new_type_unit(dwarf: &Dwarf<R>, header: TypeUnitHeader<R>) -> Result<Self> {
        Self::new_internal(
            dwarf,
            UnitSectionOffset::DebugTypesOffset(header.offset()),
            header.header(),
        )
    }

    fn new_internal(
        dwarf: &Dwarf<R>,
        offset: UnitSectionOffset<R::Offset>,
        header: UnitHeader<R>,
    ) -> Result<Self> {
        let abbreviations = header.abbreviations(&dwarf.debug_abbrev)?;
        let mut unit = Unit {
            offset,
            header,
            abbreviations,
            name: None,
            comp_dir: None,
            low_pc: 0,
            // Defaults to 0 for GNU extensions.
            str_offsets_base: DebugStrOffsetsBase(R::Offset::from_u8(0)),
            addr_base: DebugAddrBase(R::Offset::from_u8(0)),
            loclists_base: DebugLocListsBase(R::Offset::from_u8(0)),
            rnglists_base: DebugRngListsBase(R::Offset::from_u8(0)),
            line_program: None,
        };
        let mut name = None;
        let mut comp_dir = None;
        let mut line_program_offset = None;

        {
            let mut cursor = unit.header.entries(&unit.abbreviations);
            cursor.next_dfs()?;
            let root = cursor.current().ok_or(Error::MissingUnitDie)?;
            let mut attrs = root.attrs();
            while let Some(attr) = attrs.next()? {
                match attr.name() {
                    constants::DW_AT_name => {
                        name = Some(attr.value());
                    }
                    constants::DW_AT_comp_dir => {
                        comp_dir = Some(attr.value());
                    }
                    constants::DW_AT_low_pc => {
                        if let AttributeValue::Addr(address) = attr.value() {
                            unit.low_pc = address;
                        }
                    }
                    constants::DW_AT_stmt_list => {
                        if let AttributeValue::DebugLineRef(offset) = attr.value() {
                            line_program_offset = Some(offset);
                        }
                    }
                    constants::DW_AT_str_offsets_base => {
                        if let AttributeValue::DebugStrOffsetsBase(base) = attr.value() {
                            unit.str_offsets_base = base;
                        }
                    }
                    constants::DW_AT_addr_base => {
                        if let AttributeValue::DebugAddrBase(base) = attr.value() {
                            unit.addr_base = base;
                        }
                    }
                    constants::DW_AT_loclists_base => {
                        if let AttributeValue::DebugLocListsBase(base) = attr.value() {
                            unit.loclists_base = base;
                        }
                    }
                    constants::DW_AT_rnglists_base => {
                        if let AttributeValue::DebugRngListsBase(base) = attr.value() {
                            unit.rnglists_base = base;
                        }
                    }
                    _ => {}
                }
            }
        }

        unit.name = match name {
            Some(val) => dwarf.attr_string(&unit, val).ok(),
            None => None,
        };
        unit.comp_dir = match comp_dir {
            Some(val) => dwarf.attr_string(&unit, val).ok(),
            None => None,
        };
        unit.line_program = match line_program_offset {
            Some(offset) => Some(dwarf.debug_line.program(
                offset,
                unit.header.address_size(),
                unit.comp_dir.clone(),
                unit.name.clone(),
            )?),
            None => None,
        };
        Ok(unit)
    }

    /// Return the encoding parameters for this unit.
    #[inline]
    pub fn encoding(&self) -> Encoding {
        self.header.encoding()
    }

    /// Read the `DebuggingInformationEntry` at the given offset.
    pub fn entry(&self, offset: UnitOffset<R::Offset>) -> Result<DebuggingInformationEntry<R>> {
        self.header.entry(&self.abbreviations, offset)
    }

    /// Navigate this unit's `DebuggingInformationEntry`s.
    #[inline]
    pub fn entries(&self) -> EntriesCursor<R> {
        self.header.entries(&self.abbreviations)
    }

    /// Navigate this unit's `DebuggingInformationEntry`s
    /// starting at the given offset.
    #[inline]
    pub fn entries_at_offset(&self, offset: UnitOffset<R::Offset>) -> Result<EntriesCursor<R>> {
        self.header.entries_at_offset(&self.abbreviations, offset)
    }

    /// Navigate this unit's `DebuggingInformationEntry`s as a tree
    /// starting at the given offset.
    #[inline]
    pub fn entries_tree(&self, offset: Option<UnitOffset<R::Offset>>) -> Result<EntriesTree<R>> {
        self.header.entries_tree(&self.abbreviations, offset)
    }

    /// Read the raw data that defines the Debugging Information Entries.
    #[inline]
    pub fn entries_raw(&self, offset: Option<UnitOffset<R::Offset>>) -> Result<EntriesRaw<R>> {
        self.header.entries_raw(&self.abbreviations, offset)
    }
}

impl<T: ReaderOffset> UnitSectionOffset<T> {
    /// Convert an offset to be relative to the start of the given unit,
    /// instead of relative to the start of the section.
    /// Returns `None` if the offset is not within the unit entries.
    pub fn to_unit_offset<R>(&self, unit: &Unit<R>) -> Option<UnitOffset<T>>
    where
        R: Reader<Offset = T>,
    {
        let (offset, unit_offset) = match (self, unit.offset) {
            (
                UnitSectionOffset::DebugInfoOffset(offset),
                UnitSectionOffset::DebugInfoOffset(unit_offset),
            ) => (offset.0, unit_offset.0),
            (
                UnitSectionOffset::DebugTypesOffset(offset),
                UnitSectionOffset::DebugTypesOffset(unit_offset),
            ) => (offset.0, unit_offset.0),
            _ => return None,
        };
        let offset = match offset.checked_sub(unit_offset) {
            Some(offset) => UnitOffset(offset),
            None => return None,
        };
        if !unit.header.is_valid_offset(offset) {
            return None;
        }
        Some(offset)
    }
}

impl<T: ReaderOffset> UnitOffset<T> {
    /// Convert an offset to be relative to the start of the .debug_info section,
    /// instead of relative to the start of the given compilation unit.
    ///
    /// Does not check that the offset is valid.
    pub fn to_unit_section_offset<R>(&self, unit: &Unit<R>) -> UnitSectionOffset<T>
    where
        R: Reader<Offset = T>,
    {
        match unit.offset {
            UnitSectionOffset::DebugInfoOffset(unit_offset) => {
                UnitSectionOffset::DebugInfoOffset(DebugInfoOffset(unit_offset.0 + self.0))
            }
            UnitSectionOffset::DebugTypesOffset(unit_offset) => {
                UnitSectionOffset::DebugTypesOffset(DebugTypesOffset(unit_offset.0 + self.0))
            }
        }
    }
}

/// An iterator for the address ranges of a `DebuggingInformationEntry`.
///
/// Returned by `Dwarf::die_ranges` and `Dwarf::unit_ranges`.
#[derive(Debug)]
pub struct RangeIter<R: Reader>(RangeIterInner<R>);

#[derive(Debug)]
enum RangeIterInner<R: Reader> {
    Single(Option<Range>),
    List(RngListIter<R>),
}

impl<R: Reader> Default for RangeIter<R> {
    fn default() -> Self {
        RangeIter(RangeIterInner::Single(None))
    }
}

impl<R: Reader> RangeIter<R> {
    /// Advance the iterator to the next range.
    pub fn next(&mut self) -> Result<Option<Range>> {
        match self.0 {
            RangeIterInner::Single(ref mut range) => Ok(range.take()),
            RangeIterInner::List(ref mut list) => list.next(),
        }
    }
}

#[cfg(feature = "fallible-iterator")]
impl<R: Reader> fallible_iterator::FallibleIterator for RangeIter<R> {
    type Item = Range;
    type Error = Error;

    #[inline]
    fn next(&mut self) -> ::core::result::Result<Option<Self::Item>, Self::Error> {
        RangeIter::next(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read::EndianSlice;
    use crate::{Endianity, LittleEndian};

    /// Ensure that `Dwarf<R>` is covariant wrt R.
    #[test]
    fn test_dwarf_variance() {
        /// This only needs to compile.
        fn _f<'a: 'b, 'b, E: Endianity>(x: Dwarf<EndianSlice<'a, E>>) -> Dwarf<EndianSlice<'b, E>> {
            x
        }
    }

    /// Ensure that `Unit<R>` is covariant wrt R.
    #[test]
    fn test_dwarf_unit_variance() {
        /// This only needs to compile.
        fn _f<'a: 'b, 'b, E: Endianity>(x: Unit<EndianSlice<'a, E>>) -> Unit<EndianSlice<'b, E>> {
            x
        }
    }

    #[test]
    fn test_format_error() {
        let owned_dwarf =
            Dwarf::load(|_| -> Result<_> { Ok(vec![1, 2]) }, |_| Ok(vec![1, 2])).unwrap();
        let dwarf = owned_dwarf.borrow(|section| EndianSlice::new(&section, LittleEndian));

        match dwarf.debug_str.get_str(DebugStrOffset(1)) {
            Ok(r) => panic!("Unexpected str {:?}", r),
            Err(e) => {
                assert_eq!(
                    dwarf.format_error(e),
                    "Hit the end of input before it was expected at .debug_str+0x1"
                );
            }
        }
        match dwarf.debug_str_sup.get_str(DebugStrOffset(1)) {
            Ok(r) => panic!("Unexpected str {:?}", r),
            Err(e) => {
                assert_eq!(
                    dwarf.format_error(e),
                    "Hit the end of input before it was expected at .debug_str(sup)+0x1"
                );
            }
        }
        assert_eq!(dwarf.format_error(Error::Io), Error::Io.description());
    }
}
