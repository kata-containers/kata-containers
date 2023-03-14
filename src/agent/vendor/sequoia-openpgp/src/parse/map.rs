//! Packet maps.
//!
//! If configured to do so, a `PacketParser` will create a map that
//! charts the byte-stream, describing where the information was
//! extracted from.
//!
//! # Examples
//!
//! ```
//! # fn main() -> sequoia_openpgp::Result<()> {
//! use sequoia_openpgp as openpgp;
//! use openpgp::parse::{Parse, PacketParserBuilder};
//!
//! let message_data = b"\xcb\x12t\x00\x00\x00\x00\x00Hello world.";
//! let pp = PacketParserBuilder::from_bytes(message_data)?
//!     .map(true) // Enable mapping.
//!     .build()?
//!     .expect("One packet, not EOF");
//! let map = pp.map().expect("Mapping is enabled");
//!
//! assert_eq!(map.iter().nth(0).unwrap().name(), "CTB");
//! assert_eq!(map.iter().nth(0).unwrap().offset(), 0);
//! assert_eq!(map.iter().nth(0).unwrap().as_bytes(), &[0xcb]);
//! # Ok(()) }
//! ```

use std::cmp;

/// Map created during parsing.
#[derive(Clone, Debug)]
pub struct Map {
    length: usize,
    entries: Vec<Entry>,
    header: Vec<u8>,
    data: Vec<u8>,
}
assert_send_and_sync!(Map);

/// Represents an entry in the map.
#[derive(Clone, Debug)]
struct Entry {
    offset: usize,
    length: usize,
    field: &'static str,
}

impl Map {
    /// Creates a new map.
    pub(super) fn new(header: Vec<u8>) -> Self {
        Map {
            length: 0,
            entries: Vec::new(),
            header,
            data: Vec::new(),
        }
    }

    /// Adds a field to the map.
    pub(super) fn add(&mut self, field: &'static str, length: usize) {
        self.entries.push(Entry {
            offset: self.length, length, field
        });
        self.length += length;
    }

    /// Finalizes the map providing the actual data.
    pub(super) fn finalize(&mut self, data: Vec<u8>) {
        self.data = data;
    }

    /// Creates an iterator over the map.
    ///
    /// Returns references to [`Field`]s.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::parse::{Parse, PacketParserBuilder};
    ///
    /// let message_data = b"\xcb\x12t\x00\x00\x00\x00\x00Hello world.";
    /// let pp = PacketParserBuilder::from_bytes(message_data)?
    ///     .map(true) // Enable mapping.
    ///     .build()?
    ///     .expect("One packet, not EOF");
    /// let map = pp.map().expect("Mapping is enabled");
    ///
    /// assert_eq!(map.iter().count(), 6);
    /// # Ok(()) }
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = Field> + Send + Sync {
        Iter::new(self)
    }
}

/// Represents an entry in the map.
///
/// A field has a [`name`] returning a human-readable field name
/// (e.g. "CTB", or "version"), an [`offset`] into the packet, and the
/// read [`data`].
///
///   [`name`]: Field::name
///   [`offset`]: Field::offset
///   [`data`]: Field::as_bytes
#[derive(Clone, Debug)]
pub struct Field<'a> {
    /// Name of the field.
    name: &'static str,
    /// Offset of the field in the packet.
    offset: usize,
    /// Value of the field.
    data: &'a [u8],
}
assert_send_and_sync!(Field<'_>);

impl<'a> Field<'a> {
    fn new(map: &'a Map, i: usize) -> Option<Field<'a>> {
        // Old-style CTB with indeterminate length emits no length
        // field.
        let has_length = map.header.len() > 1;
        if i == 0 {
            Some(Field {
                offset: 0,
                name: "CTB",
                data: &map.header.as_slice()[..1],
            })
        } else if i == 1 && has_length {
            Some(Field {
                offset: 1,
                name: "length",
                data: &map.header.as_slice()[1..]
            })
        } else {
            let offset_length = if has_length { 1 } else { 0 };
            map.entries.get(i - 1 - offset_length).map(|e| {
                let len = map.data.len();
                let start = cmp::min(len, e.offset);
                let end = cmp::min(len, e.offset + e.length);
                Field {
                    offset: map.header.len() + e.offset,
                    name: e.field,
                    data: &map.data[start..end],
                }
            })
        }
    }

    /// Returns the name of the field.
    ///
    /// Note: The returned names are for display purposes only and may
    /// change in the future.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::parse::{Parse, PacketParserBuilder};
    ///
    /// let message_data = b"\xcb\x12t\x00\x00\x00\x00\x00Hello world.";
    /// let pp = PacketParserBuilder::from_bytes(message_data)?
    ///     .map(true) // Enable mapping.
    ///     .build()?
    ///     .expect("One packet, not EOF");
    /// let map = pp.map().expect("Mapping is enabled");
    ///
    /// assert_eq!(map.iter().nth(0).unwrap().name(), "CTB");
    /// assert_eq!(map.iter().nth(1).unwrap().name(), "length");
    /// assert_eq!(map.iter().nth(2).unwrap().name(), "format");
    /// # Ok(()) }
    /// ```
    pub fn name(&self) -> &'a str {
        self.name
    }

    /// Returns the offset of the field in the packet.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::parse::{Parse, PacketParserBuilder};
    ///
    /// let message_data = b"\xcb\x12t\x00\x00\x00\x00\x00Hello world.";
    /// let pp = PacketParserBuilder::from_bytes(message_data)?
    ///     .map(true) // Enable mapping.
    ///     .build()?
    ///     .expect("One packet, not EOF");
    /// let map = pp.map().expect("Mapping is enabled");
    ///
    /// assert_eq!(map.iter().nth(0).unwrap().offset(), 0);
    /// assert_eq!(map.iter().nth(1).unwrap().offset(), 1);
    /// assert_eq!(map.iter().nth(2).unwrap().offset(), 2);
    /// # Ok(()) }
    /// ```
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the value of the field.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::parse::{Parse, PacketParserBuilder};
    ///
    /// let message_data = b"\xcb\x12t\x00\x00\x00\x00\x00Hello world.";
    /// let pp = PacketParserBuilder::from_bytes(message_data)?
    ///     .map(true) // Enable mapping.
    ///     .build()?
    ///     .expect("One packet, not EOF");
    /// let map = pp.map().expect("Mapping is enabled");
    ///
    /// assert_eq!(map.iter().nth(0).unwrap().as_bytes(), &[0xcb]);
    /// assert_eq!(map.iter().nth(1).unwrap().as_bytes(), &[0x12]);
    /// assert_eq!(map.iter().nth(2).unwrap().as_bytes(), "t".as_bytes());
    /// # Ok(()) }
    /// ```
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

/// An iterator over the map.
struct Iter<'a> {
    map: &'a Map,
    i: usize,
}

impl<'a> Iter<'a> {
    fn new(map: &'a Map) -> Iter<'a> {
        Iter {
            map,
            i: 0,
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = Field<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let field = Field::new(self.map, self.i);
        if field.is_some() {
            self.i += 1;
        }
        field
    }
}
