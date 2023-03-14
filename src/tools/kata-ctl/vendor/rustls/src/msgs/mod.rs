#![allow(clippy::upper_case_acronyms)]

#[macro_use]
mod macros;

pub mod alert;
pub mod base;
pub mod ccs;
pub mod codec;
pub mod deframer;
#[allow(non_camel_case_types)]
pub mod enums;
pub mod fragmenter;
#[allow(non_camel_case_types)]
pub mod handshake;
pub mod hsjoiner;
pub mod message;
pub mod persist;

#[cfg(test)]
mod handshake_test;

#[cfg(test)]
mod persist_test;

#[cfg(test)]
mod enums_test;

#[cfg(test)]
mod message_test;

#[cfg(test)]
mod test {
    use std::convert::TryFrom;

    #[test]
    fn smoketest() {
        use super::codec::Reader;
        use super::message::{Message, OpaqueMessage};
        let bytes = include_bytes!("handshake-test.1.bin");
        let mut r = Reader::init(bytes);

        while r.any_left() {
            let m = OpaqueMessage::read(&mut r).unwrap();

            let out = m.clone().encode();
            assert!(!out.is_empty());

            Message::try_from(m.into_plain_message()).unwrap();
        }
    }
}
