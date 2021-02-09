use core::cmp::Ordering;
use core::marker::PhantomData;

use crate::common::{DebugInfoOffset, Encoding, SectionId};
use crate::endianity::Endianity;
use crate::read::lookup::{DebugLookup, LookupEntryIter, LookupParser};
use crate::read::{
    parse_debug_info_offset, EndianSlice, Error, Reader, ReaderOffset, Result, Section,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArangeHeader<T = usize> {
    encoding: Encoding,
    length: T,
    offset: DebugInfoOffset<T>,
    segment_size: u8,
}

/// A single parsed arange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArangeEntry<T: Copy = usize> {
    segment: Option<u64>,
    address: u64,
    length: u64,
    unit_header_offset: DebugInfoOffset<T>,
}

impl<T: Copy> ArangeEntry<T> {
    /// Return the segment selector of this arange.
    #[inline]
    pub fn segment(&self) -> Option<u64> {
        self.segment
    }

    /// Return the beginning address of this arange.
    #[inline]
    pub fn address(&self) -> u64 {
        self.address
    }

    /// Return the length of this arange.
    #[inline]
    pub fn length(&self) -> u64 {
        self.length
    }

    /// Return the offset into the .debug_info section for this arange.
    #[inline]
    pub fn debug_info_offset(&self) -> DebugInfoOffset<T> {
        self.unit_header_offset
    }
}

impl<T: Copy + Ord> PartialOrd for ArangeEntry<T> {
    fn partial_cmp(&self, other: &ArangeEntry<T>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Copy + Ord> Ord for ArangeEntry<T> {
    fn cmp(&self, other: &ArangeEntry<T>) -> Ordering {
        // The expected comparison, but ignore header.
        self.segment
            .cmp(&other.segment)
            .then(self.address.cmp(&other.address))
            .then(self.length.cmp(&other.length))
    }
}

#[derive(Clone, Debug)]
struct ArangeParser<R: Reader> {
    // This struct is never instantiated.
    phantom: PhantomData<R>,
}

impl<R: Reader> LookupParser<R> for ArangeParser<R> {
    type Header = ArangeHeader<R::Offset>;
    type Entry = ArangeEntry<R::Offset>;

    /// Parse an arange set header. Returns a tuple of the aranges to be
    /// parsed for this set, and the newly created ArangeHeader struct.
    fn parse_header(input: &mut R) -> Result<(R, Self::Header)> {
        let (length, format) = input.read_initial_length()?;
        let mut rest = input.split(length)?;

        let version = rest.read_u16()?;
        if version != 2 {
            return Err(Error::UnknownVersion(u64::from(version)));
        }

        let offset = parse_debug_info_offset(&mut rest, format)?;
        let address_size = rest.read_u8()?;
        let segment_size = rest.read_u8()?;

        // unit_length + version + offset + address_size + segment_size
        let header_length = format.initial_length_size() + 2 + format.word_size() + 1 + 1;

        // The first tuple following the header in each set begins at an offset that is
        // a multiple of the size of a single tuple (that is, the size of a segment selector
        // plus twice the size of an address).
        let tuple_length = address_size
            .checked_mul(2)
            .and_then(|x| x.checked_add(segment_size))
            .ok_or(Error::InvalidAddressRange)?;
        if tuple_length == 0 {
            return Err(Error::InvalidAddressRange)?;
        }
        let padding = if header_length % tuple_length == 0 {
            0
        } else {
            tuple_length - header_length % tuple_length
        };
        rest.skip(R::Offset::from_u8(padding))?;

        let encoding = Encoding {
            format,
            version,
            address_size,
            // TODO: segment_size
        };
        Ok((
            rest,
            ArangeHeader {
                encoding,
                length,
                offset,
                segment_size,
            },
        ))
    }

    /// Parse a single arange. Return `None` for the null arange, `Some` for an actual arange.
    fn parse_entry(input: &mut R, header: &Self::Header) -> Result<Option<Self::Entry>> {
        let address_size = header.encoding.address_size;
        let segment_size = header.segment_size; // May be zero!

        let tuple_length = R::Offset::from_u8(2 * address_size + segment_size);
        if tuple_length > input.len() {
            input.empty();
            return Ok(None);
        }

        let segment = if segment_size != 0 {
            input.read_address(segment_size)?
        } else {
            0
        };
        let address = input.read_address(address_size)?;
        let length = input.read_address(address_size)?;

        match (segment, address, length) {
            // There may be multiple sets of tuples, each terminated by a zero tuple.
            // It's not clear what purpose these zero tuples serve.  For now, we
            // simply skip them.
            (0, 0, 0) => Self::parse_entry(input, header),
            _ => Ok(Some(ArangeEntry {
                segment: if segment_size != 0 {
                    Some(segment)
                } else {
                    None
                },
                address,
                length,
                unit_header_offset: header.offset,
            })),
        }
    }
}

/// The `DebugAranges` struct represents the DWARF address range information
/// found in the `.debug_aranges` section.
#[derive(Debug, Clone)]
pub struct DebugAranges<R: Reader>(DebugLookup<R, ArangeParser<R>>);

impl<'input, Endian> DebugAranges<EndianSlice<'input, Endian>>
where
    Endian: Endianity,
{
    /// Construct a new `DebugAranges` instance from the data in the `.debug_aranges`
    /// section.
    ///
    /// It is the caller's responsibility to read the `.debug_aranges` section and
    /// present it as a `&[u8]` slice. That means using some ELF loader on
    /// Linux, a Mach-O loader on OSX, etc.
    ///
    /// ```
    /// use gimli::{DebugAranges, LittleEndian};
    ///
    /// # let buf = [];
    /// # let read_debug_aranges_section = || &buf;
    /// let debug_aranges =
    ///     DebugAranges::new(read_debug_aranges_section(), LittleEndian);
    /// ```
    pub fn new(debug_aranges_section: &'input [u8], endian: Endian) -> Self {
        Self::from(EndianSlice::new(debug_aranges_section, endian))
    }
}

impl<R: Reader> DebugAranges<R> {
    /// Iterate the aranges in the `.debug_aranges` section.
    ///
    /// ```
    /// use gimli::{DebugAranges, EndianSlice, LittleEndian};
    ///
    /// # let buf = [];
    /// # let read_debug_aranges_section = || &buf;
    /// let debug_aranges = DebugAranges::new(read_debug_aranges_section(), LittleEndian);
    ///
    /// let mut iter = debug_aranges.items();
    /// while let Some(arange) = iter.next().unwrap() {
    ///     println!("arange starts at {}, has length {}", arange.address(), arange.length());
    /// }
    /// ```
    pub fn items(&self) -> ArangeEntryIter<R> {
        ArangeEntryIter(self.0.items())
    }
}

impl<R: Reader> Section<R> for DebugAranges<R> {
    fn id() -> SectionId {
        SectionId::DebugAranges
    }

    fn reader(&self) -> &R {
        self.0.reader()
    }
}

impl<R: Reader> From<R> for DebugAranges<R> {
    fn from(debug_aranges_section: R) -> Self {
        DebugAranges(DebugLookup::from(debug_aranges_section))
    }
}

/// An iterator over the aranges from a `.debug_aranges` section.
///
/// Can be [used with
/// `FallibleIterator`](./index.html#using-with-fallibleiterator).
#[derive(Debug, Clone)]
pub struct ArangeEntryIter<R: Reader>(LookupEntryIter<R, ArangeParser<R>>);

impl<R: Reader> ArangeEntryIter<R> {
    /// Advance the iterator and return the next arange.
    ///
    /// Returns the newly parsed arange as `Ok(Some(arange))`. Returns `Ok(None)`
    /// when iteration is complete and all aranges have already been parsed and
    /// yielded. If an error occurs while parsing the next arange, then this error
    /// is returned as `Err(e)`, and all subsequent calls return `Ok(None)`.
    pub fn next(&mut self) -> Result<Option<ArangeEntry<R::Offset>>> {
        self.0.next()
    }
}

#[cfg(feature = "fallible-iterator")]
impl<R: Reader> fallible_iterator::FallibleIterator for ArangeEntryIter<R> {
    type Item = ArangeEntry<R::Offset>;
    type Error = Error;

    fn next(&mut self) -> ::core::result::Result<Option<Self::Item>, Self::Error> {
        self.0.next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{DebugInfoOffset, Format};
    use crate::endianity::LittleEndian;
    use crate::read::lookup::LookupParser;
    use crate::read::EndianSlice;

    #[test]
    fn test_parse_header_ok() {
        #[rustfmt::skip]
        let buf = [
            // 32-bit length = 32.
            0x20, 0x00, 0x00, 0x00,
            // Version.
            0x02, 0x00,
            // Offset.
            0x01, 0x02, 0x03, 0x04,
            // Address size.
            0x08,
            // Segment size.
            0x04,
            // Length to here = 12, tuple length = 20.
            // Padding to tuple length multiple = 4.
            0x10, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,

            // Dummy arange tuple data.
            0x20, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,

            // Dummy next arange.
            0x30, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        let rest = &mut EndianSlice::new(&buf, LittleEndian);

        let (tuples, header) = ArangeParser::parse_header(rest).expect("should parse header ok");

        assert_eq!(
            *rest,
            EndianSlice::new(&buf[buf.len() - 16..], LittleEndian)
        );
        assert_eq!(
            tuples,
            EndianSlice::new(&buf[buf.len() - 32..buf.len() - 16], LittleEndian)
        );
        assert_eq!(
            header,
            ArangeHeader {
                encoding: Encoding {
                    format: Format::Dwarf32,
                    version: 2,
                    address_size: 8,
                },
                length: 0x20,
                offset: DebugInfoOffset(0x0403_0201),
                segment_size: 4,
            }
        );
    }

    #[test]
    fn test_parse_header_overflow_error() {
        #[rustfmt::skip]
        let buf = [
            // 32-bit length = 32.
            0x20, 0x00, 0x00, 0x00,
            // Version.
            0x02, 0x00,
            // Offset.
            0x01, 0x02, 0x03, 0x04,
            // Address size.
            0xff,
            // Segment size.
            0xff,
            // Length to here = 12, tuple length = 20.
            // Padding to tuple length multiple = 4.
            0x10, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,

            // Dummy arange tuple data.
            0x20, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,

            // Dummy next arange.
            0x30, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        let rest = &mut EndianSlice::new(&buf, LittleEndian);

        let error = ArangeParser::parse_header(rest).expect_err("should fail to parse header");
        assert_eq!(error, Error::InvalidAddressRange);
    }

    #[test]
    fn test_parse_header_div_by_zero_error() {
        #[rustfmt::skip]
        let buf = [
            // 32-bit length = 32.
            0x20, 0x00, 0x00, 0x00,
            // Version.
            0x02, 0x00,
            // Offset.
            0x01, 0x02, 0x03, 0x04,
            // Address size = 0. Could cause a division by zero if we aren't
            // careful.
            0x00,
            // Segment size.
            0x00,
            // Length to here = 12, tuple length = 20.
            // Padding to tuple length multiple = 4.
            0x10, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,

            // Dummy arange tuple data.
            0x20, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,

            // Dummy next arange.
            0x30, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        let rest = &mut EndianSlice::new(&buf, LittleEndian);

        let error = ArangeParser::parse_header(rest).expect_err("should fail to parse header");
        assert_eq!(error, Error::InvalidAddressRange);
    }

    #[test]
    fn test_parse_entry_ok() {
        let header = ArangeHeader {
            encoding: Encoding {
                format: Format::Dwarf32,
                version: 2,
                address_size: 4,
            },
            length: 0,
            offset: DebugInfoOffset(0),
            segment_size: 0,
        };
        let buf = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09];
        let rest = &mut EndianSlice::new(&buf, LittleEndian);
        let entry = ArangeParser::parse_entry(rest, &header).expect("should parse entry ok");
        assert_eq!(*rest, EndianSlice::new(&buf[buf.len() - 1..], LittleEndian));
        assert_eq!(
            entry,
            Some(ArangeEntry {
                segment: None,
                address: 0x0403_0201,
                length: 0x0807_0605,
                unit_header_offset: header.offset,
            })
        );
    }

    #[test]
    fn test_parse_entry_segment() {
        let header = ArangeHeader {
            encoding: Encoding {
                format: Format::Dwarf32,
                version: 2,
                address_size: 4,
            },
            length: 0,
            offset: DebugInfoOffset(0),
            segment_size: 8,
        };
        #[rustfmt::skip]
        let buf = [
            // Segment.
            0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
            // Address.
            0x01, 0x02, 0x03, 0x04,
            // Length.
            0x05, 0x06, 0x07, 0x08,
            // Next tuple.
            0x09
        ];
        let rest = &mut EndianSlice::new(&buf, LittleEndian);
        let entry = ArangeParser::parse_entry(rest, &header).expect("should parse entry ok");
        assert_eq!(*rest, EndianSlice::new(&buf[buf.len() - 1..], LittleEndian));
        assert_eq!(
            entry,
            Some(ArangeEntry {
                segment: Some(0x1817_1615_1413_1211),
                address: 0x0403_0201,
                length: 0x0807_0605,
                unit_header_offset: header.offset,
            })
        );
    }

    #[test]
    fn test_parse_entry_zero() {
        let header = ArangeHeader {
            encoding: Encoding {
                format: Format::Dwarf32,
                version: 2,
                address_size: 4,
            },
            length: 0,
            offset: DebugInfoOffset(0),
            segment_size: 0,
        };
        #[rustfmt::skip]
        let buf = [
            // Zero tuple.
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // Address.
            0x01, 0x02, 0x03, 0x04,
            // Length.
            0x05, 0x06, 0x07, 0x08,
            // Next tuple.
            0x09
        ];
        let rest = &mut EndianSlice::new(&buf, LittleEndian);
        let entry = ArangeParser::parse_entry(rest, &header).expect("should parse entry ok");
        assert_eq!(*rest, EndianSlice::new(&buf[buf.len() - 1..], LittleEndian));
        assert_eq!(
            entry,
            Some(ArangeEntry {
                segment: None,
                address: 0x0403_0201,
                length: 0x0807_0605,
                unit_header_offset: header.offset,
            })
        );
    }
}
