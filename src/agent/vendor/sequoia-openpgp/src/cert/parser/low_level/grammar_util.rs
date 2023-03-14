//! Contains utility functions and definitions for the grammar.

use crate::{
    packet::Unknown,
};

/// A local alias that is either a concrete packet body or a (likely
/// Unknown) packet.
///
/// This is used in the parser grammar, but cannot be defined there.
/// lalrpop's parser doesn't allow polymorphic type aliases.
pub type PacketOrUnknown<T> = std::result::Result<T, Unknown>;
