// SPDX-License-Identifier: MIT

use super::{buffer::RuleMessageBuffer, RULE_HEADER_LEN};
use crate::{
    constants::*,
    utils::{Emitable, Parseable},
    DecodeError,
};

bitflags! {
    pub struct RuleFlags: u32 {
        const FIB_RULE_PERMANENT = FIB_RULE_PERMANENT;
        const FIB_RULE_INVERT = FIB_RULE_INVERT;
        const FIB_RULE_UNRESOLVED = FIB_RULE_UNRESOLVED;
        const FIB_RULE_IIF_DETACHED = FIB_RULE_IIF_DETACHED;
        const FIB_RULE_DEV_DETACHED = FIB_RULE_DEV_DETACHED;
        const FIB_RULE_OIF_DETACHED = FIB_RULE_OIF_DETACHED;
        const FIB_RULE_FIND_SADDR = FIB_RULE_FIND_SADDR;
    }
}

impl Default for RuleFlags {
    fn default() -> Self {
        Self::empty()
    }
}

// see https://github.com/torvalds/linux/blob/master/include/uapi/linux/fib_rules.h
// see https://github.com/torvalds/linux/blob/master/include/net/fib_rules.h
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct RuleHeader {
    /// Address family: one of the `AF_*` constants.
    pub family: u8,
    pub dst_len: u8,
    pub src_len: u8,
    pub tos: u8,
    /// RT_TABLE_*
    pub table: u8,
    /// FR_ACT_*
    pub action: u8,
    /// fib rule flags
    pub flags: u32,
}

impl Emitable for RuleHeader {
    fn buffer_len(&self) -> usize {
        RULE_HEADER_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut packet = RuleMessageBuffer::new(buffer);
        packet.set_family(self.family);
        packet.set_dst_len(self.dst_len);
        packet.set_src_len(self.src_len);
        packet.set_flags(self.flags);
        packet.set_table(self.table);
        packet.set_tos(self.tos);
        packet.set_action(self.action);
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<RuleMessageBuffer<&'a T>> for RuleHeader {
    fn parse(buf: &RuleMessageBuffer<&'a T>) -> Result<Self, DecodeError> {
        Ok(RuleHeader {
            family: buf.family(),
            dst_len: buf.dst_len(),
            src_len: buf.src_len(),
            tos: buf.tos(),
            table: buf.table(),
            action: buf.action(),
            flags: buf.flags(),
        })
    }
}
