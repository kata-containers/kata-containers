// SPDX-License-Identifier: MIT

/// Must be set on all request messages (typically from user space to kernel space)
pub const NLM_F_REQUEST: u16 = 1;
///  Indicates the message is part of a multipart message terminated by NLMSG_DONE
pub const NLM_F_MULTIPART: u16 = 2;
/// Request for an acknowledgment on success. Typical direction of request is from user space
/// (CPC) to kernel space (FEC).
pub const NLM_F_ACK: u16 = 4;
/// Echo this request.  Typical direction of request is from user space (CPC) to kernel space
/// (FEC).
pub const NLM_F_ECHO: u16 = 8;
/// Dump was inconsistent due to sequence change
pub const NLM_F_DUMP_INTR: u16 = 16;
/// Dump was filtered as requested
pub const NLM_F_DUMP_FILTERED: u16 = 32;
/// Return the complete table instead of a single entry.
pub const NLM_F_ROOT: u16 = 256;
/// Return all entries matching criteria passed in message content.
pub const NLM_F_MATCH: u16 = 512;
/// Return an atomic snapshot of the table. Requires `CAP_NET_ADMIN` capability or a effective UID
/// of 0.
pub const NLM_F_ATOMIC: u16 = 1024;
pub const NLM_F_DUMP: u16 = 768;
/// Replace existing matching object.
pub const NLM_F_REPLACE: u16 = 256;
/// Don't replace if the object already exists.
pub const NLM_F_EXCL: u16 = 512;
/// Create object if it doesn't already exist.
pub const NLM_F_CREATE: u16 = 1024;
/// Add to the end of the object list.
pub const NLM_F_APPEND: u16 = 2048;

/// Do not delete recursively
pub const NLM_F_NONREC: u16 = 256;
/// request was capped
pub const NLM_F_CAPPED: u16 = 256;
/// extended ACK TVLs were included
pub const NLM_F_ACK_TLVS: u16 = 512;
