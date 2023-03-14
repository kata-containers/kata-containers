#![allow(unused)]

use std::io::{Bytes, Read};

use crate::util::der::{DerClass, DerError, DerType};
use crate::util::oid::ObjectIdentifier;

struct DerStackItem {
    len: Option<usize>,
    parsed_len: usize,
}

pub struct DerReader<R: Read> {
    input: Bytes<R>,
    stack: Vec<DerStackItem>,
    der_type: DerType,
    constructed: bool,
    contents: Option<Vec<u8>>,
    read_count: usize,
}

impl<'a> DerReader<&'a [u8]> {
    pub fn from_bytes(input: &'a impl AsRef<[u8]>) -> Self {
        Self::from_reader(input.as_ref())
    }
}

impl<R: Read> DerReader<R> {
    pub fn from_reader(input: R) -> Self {
        Self {
            input: input.bytes(),
            stack: Vec::new(),
            der_type: DerType::EndOfContents,
            constructed: false,
            contents: None,
            read_count: 0,
        }
    }

    pub fn next(&mut self) -> Result<Option<DerType>, DerError> {
        let mut depth = self.stack.len();
        let mut is_indefinite_parent = false;
        if depth > 0 {
            match self.stack[depth - 1].len {
                Some(val) => {
                    if val == self.stack[depth - 1].parsed_len {
                        self.stack.pop();

                        depth = self.stack.len();
                        if depth > 0 {
                            self.stack[depth - 1].parsed_len += val;
                        }

                        return Ok(Some(DerType::EndOfContents));
                    }
                }
                None => {
                    is_indefinite_parent = true;
                }
            }
        }

        let start_read_count = self.read_count;

        match self.get_tag()? {
            None => return Ok(None),
            Some((DerType::EndOfContents, constructed)) => {
                if !is_indefinite_parent {
                    return Err(DerError::InvalidTag(format!(
                        "End of contents type is not allowed here."
                    )));
                }

                if constructed {
                    return Err(DerError::InvalidTag(format!(
                        "End of contents type cannot be constructed."
                    )));
                }

                match self.get_length()? {
                    Some(0) => {}
                    Some(val) => {
                        return Err(DerError::InvalidLength(format!(
                            "End of contents content length must be 0: {}",
                            val
                        )));
                    }
                    None => {
                        return Err(DerError::InvalidLength(format!(
                            "End of contents content length must be 0: indefinite"
                        )));
                    }
                }

                self.stack.pop();

                self.der_type = DerType::EndOfContents;
                self.constructed = constructed;
                self.contents = None;
            }
            Some((der_type, true)) => {
                if !der_type.can_constructed() {
                    return Err(DerError::InvalidTag(format!(
                        "{} type cannot be constructed.",
                        der_type
                    )));
                }

                let olength = self.get_length()?;
                let offset = self.read_count - start_read_count;
                self.stack.push(DerStackItem {
                    len: olength.map(|val| val + offset),
                    parsed_len: offset,
                });

                self.der_type = der_type;
                self.constructed = true;
                self.contents = None;
            }
            Some((der_type, false)) => {
                if !der_type.can_primitive() {
                    return Err(DerError::InvalidTag(format!(
                        "{} type cannot be primitive.",
                        der_type
                    )));
                }

                let length = match self.get_length()? {
                    Some(val) => val,
                    None => {
                        return Err(DerError::InvalidLength(format!(
                            "Primitive type content length cannot be indefinite."
                        )));
                    }
                };

                let mut contents = Vec::with_capacity(length);
                for _ in 0..length {
                    match self.get()? {
                        Some(val) => contents.push(val),
                        None => return Err(DerError::UnexpectedEndOfInput),
                    }
                }

                if depth > 0 {
                    let offset = self.read_count - start_read_count;
                    self.stack[depth - 1].parsed_len += offset;
                }

                self.der_type = der_type;
                self.constructed = false;
                self.contents = Some(contents);
            }
        }

        Ok(Some(self.der_type))
    }

    pub fn skip_contents(&mut self) -> Result<(), DerError> {
        if self.constructed {
            let mut depth = 1;
            loop {
                match self.next()? {
                    Some(DerType::EndOfContents) => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    Some(_) => {
                        if self.constructed {
                            depth += 1;
                        }
                    }
                    None => break,
                }
            }
        }

        Ok(())
    }

    pub fn is_constructed(&self) -> bool {
        self.constructed
    }

    pub fn is_primitive(&self) -> bool {
        !self.constructed
    }

    pub fn contents(&self) -> Option<&[u8]> {
        match &self.contents {
            Some(val) => Some(val),
            None => None,
        }
    }

    pub fn to_null(&self) -> Result<(), DerError> {
        if let DerType::Null = self.der_type {
            if let Some(contents) = &self.contents {
                if contents.len() != 0 {
                    return Err(DerError::InvalidLength(format!(
                        "Null content length must be 0: {}",
                        contents.len()
                    )));
                }

                Ok(())
            } else {
                unreachable!();
            }
        } else {
            panic!(
                "{} type is not supported to convert to null.",
                self.der_type
            );
        }
    }

    pub fn to_boolean(&self) -> Result<bool, DerError> {
        if let DerType::Boolean = self.der_type {
            if let Some(contents) = &self.contents {
                if contents.len() != 1 {
                    return Err(DerError::InvalidLength(format!(
                        "Boolean content length must be 1: {}",
                        contents.len()
                    )));
                }

                let value = contents[0] != 0;
                Ok(value)
            } else {
                unreachable!();
            }
        } else {
            panic!(
                "{} type is not supported to convert to bool.",
                self.der_type
            );
        }
    }

    pub fn to_u8(&self) -> Result<u8, DerError> {
        if let DerType::Integer | DerType::Enumerated = self.der_type {
            if let Some(contents) = &self.contents {
                if contents.len() == 0 {
                    return Err(DerError::InvalidLength(format!(
                        "{} content length must be 1 or more.",
                        self.der_type
                    )));
                }

                if contents.len() > 1 {
                    return Err(DerError::Overflow);
                }

                Ok(contents[0])
            } else {
                unreachable!();
            }
        } else {
            panic!("{} type is not supported to convert to u8.", self.der_type);
        }
    }

    pub fn to_u64(&self) -> Result<u64, DerError> {
        if let DerType::Integer | DerType::Enumerated = self.der_type {
            if let Some(contents) = &self.contents {
                if contents.len() == 0 {
                    return Err(DerError::InvalidLength(format!(
                        "{} content length must be 1 or more.",
                        self.der_type
                    )));
                }

                let mut value = 0u64;
                let mut shift_count = 0u8;
                for i in 0..contents.len() {
                    let b = contents[i];
                    shift_count += 8;
                    if shift_count > 64 {
                        return Err(DerError::Overflow);
                    }
                    value = (value << 8) | b as u64;
                }
                Ok(value)
            } else {
                unreachable!();
            }
        } else {
            panic!("{} type is not supported to convert to u64.", self.der_type);
        }
    }

    pub fn to_be_bytes(&self, sign: bool, min_len: usize) -> Vec<u8> {
        if let DerType::Integer = self.der_type {
            if let Some(contents) = &self.contents {
                if contents.len() < min_len {
                    let mut vec = Vec::with_capacity(min_len);
                    if sign && contents.len() > 0 && (contents[0] & 0b10000000) != 0 {
                        vec.push(0b10000000);
                        for _ in 0..(min_len - contents.len() - 1) {
                            vec.push(0);
                        }
                        vec.push(contents[0] & 0b01111111);
                        vec.extend_from_slice(&contents[1..]);
                    } else {
                        for _ in 0..(min_len - contents.len()) {
                            vec.push(0);
                        }
                        vec.extend_from_slice(contents);
                    }
                    vec
                } else if contents.len() - 1 >= min_len
                    && !sign
                    && contents.len() > 0
                    && contents[0] == 0
                {
                    contents[1..].to_vec()
                } else {
                    contents.to_vec()
                }
            } else {
                unreachable!();
            }
        } else {
            panic!(
                "{} type is not supported to convert to BitVec",
                self.der_type
            );
        }
    }

    pub fn to_vec(&self) -> Result<Vec<u8>, DerError> {
        if let DerType::OctetString = self.der_type {
            if let Some(contents) = &self.contents {
                Ok(contents.to_vec())
            } else {
                unreachable!();
            }
        } else {
            panic!(
                "{} type is not supported to convert to OctetString",
                self.der_type
            );
        }
    }

    pub fn to_bit_vec(&self) -> Result<(Vec<u8>, u8), DerError> {
        if let DerType::BitString = self.der_type {
            if let Some(contents) = &self.contents {
                if contents.len() < 2 {
                    return Err(DerError::InvalidLength(format!(
                        "Bit String content length must be 2 or more."
                    )));
                }

                let unused_bits = contents[0];
                if unused_bits > 7 {
                    return Err(DerError::InvalidContents(format!(
                        "Unused bit count of Bit String must be from 0 to 7."
                    )));
                }

                Ok((contents[1..contents.len()].to_vec(), unused_bits))
            } else {
                unreachable!();
            }
        } else {
            panic!(
                "{} type is not supported to convert to BitVec",
                self.der_type
            );
        }
    }

    pub fn to_string(&self) -> Result<String, DerError> {
        if let DerType::Utf8String = self.der_type {
            if let Some(contents) = &self.contents {
                let value = String::from_utf8(contents.to_vec()).map_err(|_| {
                    DerError::InvalidContents("Invalid UTF-8 sequence found".to_string())
                })?;
                Ok(value)
            } else {
                unreachable!();
            }
        } else {
            panic!(
                "{} type is not supported to convert to String.",
                self.der_type
            );
        }
    }

    pub fn to_object_identifier(&self) -> Result<ObjectIdentifier, DerError> {
        if let DerType::ObjectIdentifier = self.der_type {
            if let Some(contents) = &self.contents {
                let mut oid = Vec::<u64>::new();
                if contents.len() > 0 {
                    let b0 = contents[0];
                    oid.push((b0 / 40) as u64);
                    oid.push((b0 % 40) as u64);

                    let mut buf = 0u64;
                    let mut shift_count = 0u8;
                    for i in 1..contents.len() {
                        let b = contents[i];
                        shift_count += 7;
                        if shift_count > 64 {
                            return Err(DerError::Overflow);
                        }
                        buf = (buf << 7) | (b & 0x7F) as u64;
                        if b & 0x80 == 0 {
                            oid.push(buf);
                            buf = 0u64;
                            shift_count = 0;
                        }
                    }
                }
                return Ok(ObjectIdentifier::from_slice(&oid));
            } else {
                unreachable!();
            }
        }
        panic!(
            "{} type is not supported to convert to ObjectIdentifier.",
            self.der_type
        );
    }

    fn get_tag(&mut self) -> Result<Option<(DerType, bool)>, DerError> {
        let result = match self.get()? {
            Some(val) => {
                let der_class = Self::lookup_der_class(val >> 6);
                let constructed = ((val >> 5) & 0x01) != 0;
                let tag_no = if (val & 0x1F) > 30 {
                    let mut buf = 0u64;
                    let mut shift_count = 0u8;
                    loop {
                        match self.get()? {
                            Some(val) => {
                                shift_count += 7;
                                if shift_count > 64 {
                                    return Err(DerError::Overflow);
                                }
                                buf = (buf << 7) | (val & 0x7F) as u64;
                                if val & 0x80 == 0 {
                                    break;
                                }
                            }
                            None => return Err(DerError::UnexpectedEndOfInput),
                        }
                    }
                    buf
                } else {
                    (val & 0x1F) as u64
                };

                Some((Self::lookup_der_type(der_class, tag_no), constructed))
            }
            None => None,
        };
        Ok(result)
    }

    fn lookup_der_class(class_no: u8) -> DerClass {
        match class_no {
            0b00 => DerClass::Universal,
            0b01 => DerClass::Application,
            0b10 => DerClass::ContextSpecific,
            0b11 => DerClass::Private,
            _ => unreachable!(),
        }
    }

    fn lookup_der_type(class: DerClass, tag_no: u64) -> DerType {
        match (class, tag_no) {
            (DerClass::Universal, 0) => DerType::EndOfContents,
            (DerClass::Universal, 1) => DerType::Boolean,
            (DerClass::Universal, 2) => DerType::Integer,
            (DerClass::Universal, 3) => DerType::BitString,
            (DerClass::Universal, 4) => DerType::OctetString,
            (DerClass::Universal, 5) => DerType::Null,
            (DerClass::Universal, 6) => DerType::ObjectIdentifier,
            (DerClass::Universal, 7) => DerType::ObjectDescriptor,
            (DerClass::Universal, 8) => DerType::External,
            (DerClass::Universal, 9) => DerType::Real,
            (DerClass::Universal, 10) => DerType::Enumerated,
            (DerClass::Universal, 11) => DerType::EmbeddedPdv,
            (DerClass::Universal, 12) => DerType::Utf8String,
            (DerClass::Universal, 13) => DerType::RelativeOid,
            (DerClass::Universal, 14) => DerType::Time,
            (DerClass::Universal, 16) => DerType::Sequence,
            (DerClass::Universal, 17) => DerType::Set,
            (DerClass::Universal, 18) => DerType::NumericString,
            (DerClass::Universal, 19) => DerType::PrintableString,
            (DerClass::Universal, 20) => DerType::TeletexString,
            (DerClass::Universal, 21) => DerType::VideotexString,
            (DerClass::Universal, 22) => DerType::Ia5String,
            (DerClass::Universal, 23) => DerType::UtcTime,
            (DerClass::Universal, 24) => DerType::GeneralizedTime,
            (DerClass::Universal, 25) => DerType::GraphicString,
            (DerClass::Universal, 26) => DerType::VisibleString,
            (DerClass::Universal, 27) => DerType::GeneralString,
            (DerClass::Universal, 28) => DerType::UniversalString,
            (DerClass::Universal, 29) => DerType::CharacterString,
            (DerClass::Universal, 30) => DerType::BmpString,
            (DerClass::Universal, 31) => DerType::Date,
            (DerClass::Universal, 32) => DerType::TimeOfDay,
            (DerClass::Universal, 33) => DerType::DateTime,
            (DerClass::Universal, 34) => DerType::Duration,
            _ => DerType::Other(class, tag_no),
        }
    }

    fn get_length(&mut self) -> Result<Option<usize>, DerError> {
        let result = match self.get()? {
            Some(val) if val == 0xFF => {
                return Err(DerError::InvalidLength(format!(
                    "Length 0x{:X} is reserved for possible future extension.",
                    val
                )));
            }
            Some(val) if val == 0x80 => None,
            Some(val) if val < 0x80 => Some(val as usize),
            Some(val) => {
                let len_size = (val & 0x7F) as usize;
                if len_size > std::mem::size_of::<usize>() {
                    return Err(DerError::Overflow);
                }
                let mut num = 0usize;
                for _ in 0..len_size {
                    match self.get()? {
                        Some(val) => {
                            num = num << 8 | val as usize;
                        }
                        None => return Err(DerError::UnexpectedEndOfInput),
                    }
                }
                Some(num)
            }
            None => return Err(DerError::UnexpectedEndOfInput),
        };
        Ok(result)
    }

    fn get(&mut self) -> Result<Option<u8>, DerError> {
        let result = match self.input.next() {
            Some(Ok(val)) => {
                self.read_count += 1;
                Some(val)
            }
            Some(Err(err)) => return Err(DerError::ReadFailure(err)),
            None => None,
        };
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use anyhow::Result;
    use std::fs::File;
    use std::path::PathBuf;

    use crate::util::der::DerBuilder;

    #[test]
    fn parse_der() -> Result<()> {
        let bytes = load_file("der/RSA_2048bit_raw_public.der")?;

        let mut parser = DerReader::from_reader(bytes);
        assert!(matches!(parser.next()?, Some(DerType::Sequence)));
        assert!(matches!(parser.next()?, Some(DerType::Integer)));
        assert!(matches!(parser.next()?, Some(DerType::Integer)));
        assert!(matches!(parser.next()?, Some(DerType::EndOfContents)));
        Ok(())
    }

    #[test]
    fn parse_der_2() -> Result<()> {
        let mut vec = Vec::new();
        let _ = load_file("der/RSA_2048bit_raw_public.der")?.read_to_end(&mut vec)?;

        let mut parser = DerReader::from_bytes(&vec);
        assert!(matches!(parser.next()?, Some(DerType::Sequence)));
        assert!(matches!(parser.next()?, Some(DerType::Integer)));
        assert!(matches!(parser.next()?, Some(DerType::Integer)));
        assert!(matches!(parser.next()?, Some(DerType::EndOfContents)));
        Ok(())
    }

    #[test]
    fn parse_der_3() -> Result<()> {
        let mut builder = DerBuilder::new();
        builder.begin(DerType::Sequence);
        {
            builder.begin(DerType::Sequence);
            {
                builder.append_integer_from_u8(1);
            }
            builder.end();
        }
        builder.end();

        let input = builder.build();
        let mut parser = DerReader::from_bytes(&input);
        assert!(matches!(parser.next()?, Some(DerType::Sequence)));
        assert!(matches!(parser.next()?, Some(DerType::Sequence)));
        assert!(matches!(parser.next()?, Some(DerType::Integer)));
        assert!(matches!(parser.next()?, Some(DerType::EndOfContents)));
        assert!(matches!(parser.next()?, Some(DerType::EndOfContents)));

        Ok(())
    }

    fn load_file(path: &str) -> Result<File> {
        let mut pb = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pb.push("data");
        pb.push(path);

        let file = File::open(&pb)?;
        Ok(file)
    }
}
