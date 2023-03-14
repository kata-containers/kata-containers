// SPDX-License-Identifier: MIT

/// Handles
pub const TC_H_MAJ_MASK: u32 = 0xFFFF0000;
pub const TC_H_MIN_MASK: u32 = 0x0000FFFF;

#[macro_export]
macro_rules! TC_H_MAKE {
    ($maj: expr, $min: expr) => {
        ($maj & TC_H_MAJ_MASK) | ($min & TC_H_MIN_MASK)
    };
}

pub const TC_H_UNSPEC: u32 = 0;
pub const TC_H_ROOT: u32 = 0xFFFFFFFF;
pub const TC_H_INGRESS: u32 = 0xFFFFFFF1;
pub const TC_H_CLSACT: u32 = TC_H_INGRESS;

pub const TC_H_MIN_PRIORITY: u32 = 0xFFE0;
pub const TC_H_MIN_INGRESS: u32 = 0xFFF2;
pub const TC_H_MIN_EGRESS: u32 = 0xFFF3;

/// U32 filters
pub const TCA_U32_UNSPEC: u16 = 0;
pub const TCA_U32_CLASSID: u16 = 1;
pub const TCA_U32_HASH: u16 = 2;
pub const TCA_U32_LINK: u16 = 3;
pub const TCA_U32_DIVISOR: u16 = 4;
pub const TCA_U32_SEL: u16 = 5;
pub const TCA_U32_POLICE: u16 = 6;
pub const TCA_U32_ACT: u16 = 7;
pub const TCA_U32_INDEV: u16 = 8;
pub const TCA_U32_PCNT: u16 = 9;
pub const TCA_U32_MARK: u16 = 10;
pub const TCA_U32_FLAGS: u16 = 11;
pub const TCA_U32_PAD: u16 = 12;
pub const TCA_U32_MAX: u16 = TCA_U32_PAD;

/// U32 Flags
pub const TC_U32_TERMINAL: u8 = 1;
pub const TC_U32_OFFSET: u8 = 2;
pub const TC_U32_VAROFFSET: u8 = 4;
pub const TC_U32_EAT: u8 = 8;
pub const TC_U32_MAXDEPTH: u8 = 8;

/// Action attributes
pub const TCA_ACT_UNSPEC: u16 = 0;
pub const TCA_ACT_KIND: u16 = 1;
pub const TCA_ACT_OPTIONS: u16 = 2;
pub const TCA_ACT_INDEX: u16 = 3;
pub const TCA_ACT_STATS: u16 = 4;
pub const TCA_ACT_PAD: u16 = 5;
pub const TCA_ACT_COOKIE: u16 = 6;

//TODO(wllenyj): Why not subtract 1? See `linux/pkt_cls.h` for original definition.
pub const TCA_ACT_MAX: u16 = 7;
pub const TCA_OLD_COMPAT: u16 = TCA_ACT_MAX + 1;
pub const TCA_ACT_MAX_PRIO: u16 = 32;
pub const TCA_ACT_BIND: u16 = 1;
pub const TCA_ACT_NOBIND: u16 = 0;
pub const TCA_ACT_UNBIND: u16 = 1;
pub const TCA_ACT_NOUNBIND: u16 = 0;
pub const TCA_ACT_REPLACE: u16 = 1;
pub const TCA_ACT_NOREPLACE: u16 = 0;

pub const TC_ACT_UNSPEC: i32 = -1;
pub const TC_ACT_OK: i32 = 0;
pub const TC_ACT_RECLASSIFY: i32 = 1;
pub const TC_ACT_SHOT: i32 = 2;
pub const TC_ACT_PIPE: i32 = 3;
pub const TC_ACT_STOLEN: i32 = 4;
pub const TC_ACT_QUEUED: i32 = 5;
pub const TC_ACT_REPEAT: i32 = 6;
pub const TC_ACT_REDIRECT: i32 = 7;
pub const TC_ACT_TRAP: i32 = 8;

pub const TC_ACT_VALUE_MAX: i32 = TC_ACT_TRAP;

pub const TC_ACT_JUMP: i32 = 0x10000000;

pub const TCA_ACT_TAB: u16 = 1; // TCA_ROOT_TAB
pub const TCAA_MAX: u16 = 1;

/// Mirred action attr
pub const TCA_MIRRED_UNSPEC: u16 = 0;
pub const TCA_MIRRED_TM: u16 = 1;
pub const TCA_MIRRED_PARMS: u16 = 2;
pub const TCA_MIRRED_PAD: u16 = 3;
pub const TCA_MIRRED_MAX: u16 = TCA_MIRRED_PAD;

pub const TCA_EGRESS_REDIR: i32 = 1; /* packet redirect to EGRESS */
pub const TCA_EGRESS_MIRROR: i32 = 2; /* mirror packet to EGRESS */
pub const TCA_INGRESS_REDIR: i32 = 3; /* packet redirect to INGRESS */
pub const TCA_INGRESS_MIRROR: i32 = 4; /* mirror packet to INGRESS */
