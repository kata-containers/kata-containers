pub mod bit_string;
pub mod date;
pub mod restricted_string;
pub mod tag;
pub mod wrapper;

use tag::Tag;

pub trait Asn1Type {
    const TAG: Tag;
    const NAME: &'static str;
}

impl Asn1Type for () {
    const TAG: Tag = Tag::NULL;
    const NAME: &'static str = "()";
}

impl Asn1Type for String {
    const TAG: Tag = Tag::UTF8_STRING;
    const NAME: &'static str = "String";
}

impl Asn1Type for bool {
    const TAG: Tag = Tag::BOOLEAN;
    const NAME: &'static str = "bool";
}

impl Asn1Type for u8 {
    const TAG: Tag = Tag::INTEGER;
    const NAME: &'static str = "u8";
}

impl Asn1Type for u16 {
    const TAG: Tag = Tag::INTEGER;
    const NAME: &'static str = "u16";
}

impl Asn1Type for u32 {
    const TAG: Tag = Tag::INTEGER;
    const NAME: &'static str = "u32";
}

impl Asn1Type for u64 {
    const TAG: Tag = Tag::INTEGER;
    const NAME: &'static str = "u64";
}

impl Asn1Type for u128 {
    const TAG: Tag = Tag::INTEGER;
    const NAME: &'static str = "u128";
}
