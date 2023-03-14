use crate::ber::BitStringObject;
use crate::ber::{BerObject, BerObjectContent};
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use asn1_rs::{Class, Header, Length, Tag};
use core::fmt;
use core::iter::FromIterator;
use core::str;
use debug::HexSlice;

use rusticata_macros::debug;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PrettyPrinterFlag {
    Recursive,
    ShowHeader,
}

/// Pretty-print BER object
///
/// This method is recursive by default. To prevent that, unset the `Recursive` flag.
pub struct PrettyBer<'a> {
    obj: &'a BerObject<'a>,
    indent: usize,
    inc: usize,

    flags: Vec<PrettyPrinterFlag>,
}

impl<'a> BerObject<'a> {
    pub fn as_pretty(&'a self, indent: usize, increment: usize) -> PrettyBer<'a> {
        PrettyBer::new(self, vec![PrettyPrinterFlag::Recursive], indent, increment)
    }
}

impl<'a> PrettyBer<'a> {
    pub const fn new(
        obj: &'a BerObject<'a>,
        flags: Vec<PrettyPrinterFlag>,
        indent: usize,
        increment: usize,
    ) -> Self {
        Self {
            obj,
            indent,
            inc: increment,
            flags,
        }
    }

    pub fn set_flag(&mut self, flag: PrettyPrinterFlag) {
        if !self.flags.contains(&flag) {
            self.flags.push(flag);
        }
    }

    pub fn unset_flag(&mut self, flag: PrettyPrinterFlag) {
        self.flags.retain(|&f| f != flag);
    }

    pub fn is_flag_set(&self, flag: PrettyPrinterFlag) -> bool {
        self.flags.contains(&flag)
    }

    pub fn next_indent<'b>(&self, obj: &'b BerObject) -> PrettyBer<'b> {
        PrettyBer {
            obj,
            indent: self.indent + self.inc,
            inc: self.inc,
            flags: self.flags.to_vec(),
        }
    }

    #[inline]
    fn is_recursive(&self) -> bool {
        self.is_flag_set(PrettyPrinterFlag::Recursive)
    }
}

fn dbg_header(header: &Header, f: &mut fmt::Formatter) -> fmt::Result {
    let s_constructed = if header.is_constructed() { "+" } else { "" };
    let l = match header.length() {
        Length::Definite(sz) => sz.to_string(),
        Length::Indefinite => "Indefinite".to_string(),
    };
    match header.class() {
        Class::Universal => {
            write!(f, "[{}]{} {}", header.tag(), s_constructed, l)?;
        }
        Class::ContextSpecific => {
            write!(f, "[{}]{} {}", header.tag().0, s_constructed, l)?;
        }

        class => {
            write!(f, "[{} {}]{} {}", class, header.tag().0, s_constructed, l)?;
        }
    }
    Ok(())
}

impl<'a> fmt::Debug for PrettyBer<'a> {
    #[rustfmt::skip]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.indent > 0 {
            write!(f, "{:1$}", " ", self.indent)?;
        };
        if self.flags.contains(&PrettyPrinterFlag::ShowHeader) {
            dbg_header(&self.obj.header, f)?;
            write!(f, " ")?;
        };
        fn print_utf32_string_with_type(f: &mut fmt::Formatter, s: &[u8], ty: &str) -> fmt::Result {
            let chars: Option<Vec<char>> = s
                .chunks_exact(4)
                .map(|a| core::char::from_u32(u32::from_be_bytes([a[0], a[1], a[2], a[3]])))
                .collect();

            match chars {
                Some(b)  => writeln!(f, "{}(\"{}\")", ty, String::from_iter(b)),
                None => writeln!(f, "{}({:?}) <error decoding utf32 string>", ty, s),
            }
        }
        match self.obj.content {
            BerObjectContent::EndOfContent           => write!(f, "EndOfContent"),
            BerObjectContent::Boolean(b)             => write!(f, "Boolean({:?})", b),
            BerObjectContent::Integer(i)             => write!(f, "Integer({:?})", HexSlice(i)),
            BerObjectContent::Enum(i)                => write!(f, "Enum({})", i),
            BerObjectContent::OID(ref v)             => write!(f, "OID({:?})", v),
            BerObjectContent::RelativeOID(ref v)     => write!(f, "RelativeOID({:?})", v),
            BerObjectContent::Null                   => write!(f, "Null"),
            BerObjectContent::OctetString(v)         => write!(f, "OctetString({:?})", HexSlice(v)),
            BerObjectContent::BitString(u,BitStringObject{data:v})
                                                     => write!(f, "BitString({},{:?})", u, HexSlice(v)),
            BerObjectContent::GeneralizedTime(ref time)     => write!(f, "GeneralizedTime(\"{}\")", time),
            BerObjectContent::UTCTime(ref time)             => write!(f, "UTCTime(\"{}\")", time),
            BerObjectContent::VisibleString(s)       => write!(f, "VisibleString(\"{}\")", s),
            BerObjectContent::GeneralString(s)       => write!(f, "GeneralString(\"{}\")", s),
            BerObjectContent::GraphicString(s)       => write!(f, "GraphicString(\"{}\")", s),
            BerObjectContent::PrintableString(s)     => write!(f, "PrintableString(\"{}\")", s),
            BerObjectContent::NumericString(s)       => write!(f, "NumericString(\"{}\")", s),
            BerObjectContent::UTF8String(s)          => write!(f, "UTF8String(\"{}\")", s),
            BerObjectContent::IA5String(s)           => write!(f, "IA5String(\"{}\")", s),
            BerObjectContent::T61String(s)           => write!(f, "T61String({})", s),
            BerObjectContent::VideotexString(s)      => write!(f, "VideotexString({})", s),
            BerObjectContent::ObjectDescriptor(s)    => write!(f, "ObjectDescriptor(\"{}\")", s),
            BerObjectContent::BmpString(s)           => write!(f, "BmpString(\"{}\")", s),
            BerObjectContent::UniversalString(s)     => print_utf32_string_with_type(f, s, "UniversalString"),
            BerObjectContent::Optional(ref o) => {
                match o {
                    Some(obj) => write!(f, "OPTION {:?}", obj),
                    None => write!(f, "NONE"),
                }
            }
            BerObjectContent::Tagged(class, tag, ref obj) => {
                writeln!(f, "ContextSpecific [{} {}] {{", class, tag.0)?;
                write!(f, "{:?}", self.next_indent(obj))?;
                if self.indent > 0 {
                    write!(f, "{:1$}", " ", self.indent)?;
                };
                write!(f, "}}")?;
                Ok(())
            },
            BerObjectContent::Set(ref v) |
            BerObjectContent::Sequence(ref v)        => {
                let ty = if self.obj.header.tag() == Tag::Sequence { "Sequence" } else { "Set" };
                if self.is_recursive() {
                    writeln!(f, "{}[", ty)?;
                    for o in v {
                        write!(f, "{:?}", self.next_indent(o))?;
                    };
                    if self.indent > 0 {
                        write!(f, "{:1$}", " ", self.indent)?;
                    };
                    write!(f, "]")?;
                } else {
                    write!(f, "{}", ty)?;
                }
                Ok(())
            },
            BerObjectContent::Unknown(ref any) => {
                write!(f, "Unknown {:x?}", HexSlice(any.data))
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PrettyPrinterFlag;
    use crate::ber::*;

    #[test]
    fn test_pretty_print() {
        let d = BerObject::from_obj(BerObjectContent::Sequence(vec![
            BerObject::from_int_slice(b"\x01\x00\x01"),
            BerObject::from_int_slice(b"\x01\x00\x01"),
            BerObject::from_obj(BerObjectContent::Set(vec![
                BerObject::from_int_slice(b"\x01"),
                BerObject::from_int_slice(b"\x02"),
            ])),
        ]));

        println!("{:?}", d.as_pretty(0, 2));

        let mut pp = d.as_pretty(0, 4);
        pp.set_flag(PrettyPrinterFlag::ShowHeader);
        println!("{:?}", pp);
    }
}
