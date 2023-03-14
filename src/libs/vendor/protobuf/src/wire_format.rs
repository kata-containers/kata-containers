//! Serialization constants.

// TODO: temporary
pub use self::WireType::*;

/// Tag occupies 3 bits
pub const TAG_TYPE_BITS: u32 = 3;
/// Tag mask
pub const TAG_TYPE_MASK: u32 = (1u32 << TAG_TYPE_BITS) - 1;
/// Max possible field number
pub const FIELD_NUMBER_MAX: u32 = 0x1fffffff;

/// One of six defined protobuf wire types
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum WireType {
    /// Varint (e. g. `int32` or `sint64`)
    WireTypeVarint = 0,
    /// Fixed size 64 bit (e. g. `fixed64` or `double`)
    WireTypeFixed64 = 1,
    /// Length-delimited (e. g. `message` or `string`)
    WireTypeLengthDelimited = 2,
    /// Groups are not supported by rust-protobuf
    WireTypeStartGroup = 3,
    /// Groups are not supported by rust-protobuf
    WireTypeEndGroup = 4,
    /// Fixed size 64 bit (e. g. `fixed32` or `float`)
    WireTypeFixed32 = 5,
}

impl Copy for WireType {}

impl WireType {
    /// Parse wire type
    pub fn new(n: u32) -> Option<WireType> {
        match n {
            0 => Some(WireTypeVarint),
            1 => Some(WireTypeFixed64),
            2 => Some(WireTypeLengthDelimited),
            3 => Some(WireTypeStartGroup),
            4 => Some(WireTypeEndGroup),
            5 => Some(WireTypeFixed32),
            _ => None,
        }
    }
}

/// Parsed protobuf tag, which is a pair of field number and wire type
#[derive(Clone)]
pub struct Tag {
    field_number: u32,
    wire_type: WireType,
}

impl Copy for Tag {}

impl Tag {
    /// Pack a tag to integer
    pub fn value(self) -> u32 {
        (self.field_number << TAG_TYPE_BITS) | (self.wire_type as u32)
    }

    /// Parse integer into `Tag` object
    // TODO: should return Result instead of Option
    pub fn new(value: u32) -> Option<Tag> {
        let wire_type = WireType::new(value & TAG_TYPE_MASK);
        if wire_type.is_none() {
            return None;
        }
        let field_number = value >> TAG_TYPE_BITS;
        if field_number == 0 {
            return None;
        }
        Some(Tag {
            field_number: field_number,
            wire_type: wire_type.unwrap(),
        })
    }

    /// Create a tag from a field number and wire type.
    ///
    /// # Panics
    ///
    /// If field number is outside of allowed range.
    pub fn make(field_number: u32, wire_type: WireType) -> Tag {
        assert!(field_number > 0 && field_number <= FIELD_NUMBER_MAX);
        Tag {
            field_number: field_number,
            wire_type: wire_type,
        }
    }

    /// Tag as pair of (field number, wire type)
    pub fn unpack(self) -> (u32, WireType) {
        (self.field_number(), self.wire_type())
    }

    fn wire_type(self) -> WireType {
        self.wire_type
    }

    /// Protobuf field number
    pub fn field_number(self) -> u32 {
        self.field_number
    }
}
