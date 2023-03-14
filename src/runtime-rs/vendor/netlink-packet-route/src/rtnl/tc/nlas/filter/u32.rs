// SPDX-License-Identifier: MIT

/// U32 filter
///
/// In its simplest form the U32 filter is a list of records, each consisting
/// of two fields: a selector and an action. The selectors, described below,
/// are compared with the currently processed IP packet until the first match
/// occurs, and then the associated action is performed.
use anyhow::Context;
use byteorder::{ByteOrder, NativeEndian};

use crate::{
    nlas::{self, DefaultNla, NlaBuffer, NlasIterator},
    parsers::parse_u32,
    tc::{constants::*, Action},
    traits::{Emitable, Parseable},
    DecodeError,
};

pub const KIND: &str = "u32";

const U32_SEL_BUF_LEN: usize = 16;
const U32_KEY_BUF_LEN: usize = 16;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Nla {
    Unspec(Vec<u8>),
    ClassId(u32),
    Hash(u32),
    Link(u32),
    Divisor(u32),
    Sel(Sel),
    Police(Vec<u8>),
    Act(Vec<Action>),
    Indev(Vec<u8>),
    Pcnt(Vec<u8>),
    Mark(Vec<u8>),
    Flags(u32),
    Other(DefaultNla),
}

impl nlas::Nla for Nla {
    fn value_len(&self) -> usize {
        use self::Nla::*;
        match self {
            Unspec(b) | Police(b) | Indev(b) | Pcnt(b) | Mark(b) => b.len(),
            ClassId(_) | Hash(_) | Link(_) | Divisor(_) | Flags(_) => 4,
            Sel(s) => s.buffer_len(),
            Act(acts) => acts.as_slice().buffer_len(),
            Other(attr) => attr.value_len(),
        }
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        use self::Nla::*;
        match self {
            Unspec(b) | Police(b) | Indev(b) | Pcnt(b) | Mark(b) => {
                buffer.copy_from_slice(b.as_slice())
            }
            ClassId(i) | Hash(i) | Link(i) | Divisor(i) | Flags(i) => {
                NativeEndian::write_u32(buffer, *i)
            }
            Sel(s) => s.emit(buffer),
            Act(acts) => acts.as_slice().emit(buffer),
            Other(attr) => attr.emit_value(buffer),
        }
    }

    fn kind(&self) -> u16 {
        use self::Nla::*;
        match self {
            Unspec(_) => TCA_U32_UNSPEC,
            ClassId(_) => TCA_U32_CLASSID,
            Hash(_) => TCA_U32_HASH,
            Link(_) => TCA_U32_LINK,
            Divisor(_) => TCA_U32_DIVISOR,
            Sel(_) => TCA_U32_SEL,
            Police(_) => TCA_U32_POLICE,
            Act(_) => TCA_U32_ACT,
            Indev(_) => TCA_U32_INDEV,
            Pcnt(_) => TCA_U32_PCNT,
            Mark(_) => TCA_U32_MARK,
            Flags(_) => TCA_U32_FLAGS,
            Other(attr) => attr.kind(),
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for Nla {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::Nla::*;
        let payload = buf.value();
        Ok(match buf.kind() {
            TCA_U32_UNSPEC => Unspec(payload.to_vec()),
            TCA_U32_CLASSID => {
                ClassId(parse_u32(payload).context("failed to parse TCA_U32_UNSPEC")?)
            }
            TCA_U32_HASH => Hash(parse_u32(payload).context("failed to parse TCA_U32_HASH")?),
            TCA_U32_LINK => Link(parse_u32(payload).context("failed to parse TCA_U32_LINK")?),
            TCA_U32_DIVISOR => {
                Divisor(parse_u32(payload).context("failed to parse TCA_U32_DIVISOR")?)
            }
            TCA_U32_SEL => Sel(self::Sel::parse(
                &SelBuffer::new_checked(payload).context("invalid TCA_U32_SEL")?,
            )
            .context("failed to parse TCA_U32_SEL")?),
            TCA_U32_POLICE => Police(payload.to_vec()),
            TCA_U32_ACT => {
                let mut acts = vec![];
                for act in NlasIterator::new(payload) {
                    let act = act.context("invalid TCA_U32_ACT")?;
                    acts.push(Action::parse(&act).context("failed to parse TCA_U32_ACT")?);
                }
                Act(acts)
            }
            TCA_U32_INDEV => Indev(payload.to_vec()),
            TCA_U32_PCNT => Pcnt(payload.to_vec()),
            TCA_U32_MARK => Mark(payload.to_vec()),
            TCA_U32_FLAGS => Flags(parse_u32(payload).context("failed to parse TCA_U32_FLAGS")?),
            _ => Other(DefaultNla::parse(buf).context("failed to parse u32 nla")?),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Sel {
    pub flags: u8,
    pub offshift: u8,
    pub nkeys: u8,
    pub offmask: u16,
    pub off: u16,
    pub offoff: u16,
    pub hoff: u16,
    pub hmask: u32,
    pub keys: Vec<Key>,
}

buffer!(SelBuffer(U32_SEL_BUF_LEN) {
    flags: (u8, 0),
    offshift: (u8, 1),
    nkeys: (u8, 2),
    //pad: (u8, 3),
    offmask: (u16, 4..6),
    off: (u16, 6..8),
    offoff: (u16, 8..10),
    hoff: (u16, 10..12),
    hmask: (u32, 12..U32_SEL_BUF_LEN),
    keys: (slice, U32_SEL_BUF_LEN..),
});

impl Emitable for Sel {
    fn buffer_len(&self) -> usize {
        U32_SEL_BUF_LEN + (self.nkeys as usize * U32_KEY_BUF_LEN)
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut packet = SelBuffer::new(buffer);
        packet.set_flags(self.flags);
        packet.set_offshift(self.offshift);
        packet.set_offmask(self.offmask);
        packet.set_off(self.off);
        packet.set_offoff(self.offoff);
        packet.set_hoff(self.hoff);
        packet.set_hmask(self.hmask);
        packet.set_nkeys(self.nkeys);
        assert_eq!(self.nkeys as usize, self.keys.len());

        let key_buf = packet.keys_mut();
        for (i, k) in self.keys.iter().enumerate() {
            k.emit(&mut key_buf[(i * U32_KEY_BUF_LEN)..((i + 1) * U32_KEY_BUF_LEN)]);
        }
    }
}

impl<T: AsRef<[u8]> + ?Sized> Parseable<SelBuffer<&T>> for Sel {
    fn parse(buf: &SelBuffer<&T>) -> Result<Self, DecodeError> {
        let nkeys = buf.nkeys();
        let mut keys = Vec::<Key>::with_capacity(nkeys.into());
        let key_payload = buf.keys();
        for i in 0..nkeys {
            let i = i as usize;
            let keybuf = KeyBuffer::new_checked(
                &key_payload[(i * U32_KEY_BUF_LEN)..(i + 1) * U32_KEY_BUF_LEN],
            )
            .context("invalid u32 key")?;
            keys.push(Key::parse(&keybuf).context("failed to parse u32 key")?);
        }

        Ok(Self {
            flags: buf.flags(),
            offshift: buf.offshift(),
            nkeys,
            offmask: buf.offmask(),
            off: buf.off(),
            offoff: buf.offoff(),
            hoff: buf.hoff(),
            hmask: buf.hmask(),
            keys,
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Key {
    pub mask: u32,
    pub val: u32,
    pub off: i32,
    pub offmask: i32,
}

buffer!(KeyBuffer(U32_KEY_BUF_LEN) {
    mask: (u32, 0..4),
    val: (u32, 4..8),
    off: (i32, 8..12),
    offmask: (i32, 12..U32_KEY_BUF_LEN),
});

impl Emitable for Key {
    fn buffer_len(&self) -> usize {
        U32_KEY_BUF_LEN
    }
    fn emit(&self, buffer: &mut [u8]) {
        let mut packet = KeyBuffer::new(buffer);
        packet.set_mask(self.mask);
        packet.set_val(self.val);
        packet.set_off(self.off);
        packet.set_offmask(self.offmask);
    }
}

impl<T: AsRef<[u8]>> Parseable<KeyBuffer<T>> for Key {
    fn parse(buf: &KeyBuffer<T>) -> Result<Self, DecodeError> {
        Ok(Self {
            mask: buf.mask(),
            val: buf.val(),
            off: buf.off(),
            offmask: buf.offmask(),
        })
    }
}
