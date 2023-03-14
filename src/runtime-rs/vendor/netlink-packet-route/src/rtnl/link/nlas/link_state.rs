// SPDX-License-Identifier: MIT

use crate::constants::*;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum State {
    /// Status can't be determined
    Unknown,
    /// Some component is missing
    NotPresent,
    /// Down
    Down,
    /// Down due to state of lower layer
    LowerLayerDown,
    /// In some test mode
    Testing,
    /// Not up but pending an external event
    Dormant,
    /// Up, ready to send packets
    Up,
    /// Unrecognized value. This should go away when `TryFrom` is stable in Rust
    // FIXME: there's not point in having this. When TryFrom is stable we'll remove it
    Other(u8),
}

impl From<u8> for State {
    fn from(value: u8) -> Self {
        use self::State::*;
        match value {
            IF_OPER_UNKNOWN => Unknown,
            IF_OPER_NOTPRESENT => NotPresent,
            IF_OPER_DOWN => Down,
            IF_OPER_LOWERLAYERDOWN => LowerLayerDown,
            IF_OPER_TESTING => Testing,
            IF_OPER_DORMANT => Dormant,
            IF_OPER_UP => Up,
            _ => Other(value),
        }
    }
}

impl From<State> for u8 {
    fn from(value: State) -> Self {
        use self::State::*;
        match value {
            Unknown => IF_OPER_UNKNOWN,
            NotPresent => IF_OPER_NOTPRESENT,
            Down => IF_OPER_DOWN,
            LowerLayerDown => IF_OPER_LOWERLAYERDOWN,
            Testing => IF_OPER_TESTING,
            Dormant => IF_OPER_DORMANT,
            Up => IF_OPER_UP,
            Other(other) => other,
        }
    }
}
