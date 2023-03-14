// SPDX-License-Identifier: Apache-2.0 or MIT
//
// Copyright 2021 Sony Group Corporation
//

use crate::ScmpCompareOp;
use libseccomp_sys::*;

/// Represents a rule in a libseccomp filter context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ScmpArgCompare(scmp_arg_cmp);

impl ScmpArgCompare {
    /// Creates and returns a new condition to attach to a filter rule.
    ///
    /// The rule will match if the comparison of argument `arg` (zero-indexed argument
    /// of the syscall) with the value provided by `datum` using the compare operator
    /// provided by `op` is true.
    ///
    /// You can use the [`scmp_cmp!`](crate::scmp_cmp) macro instead of this to create
    /// `ScmpArgCompare` in a more elegant way.
    ///
    /// # Arguments
    ///
    /// * `arg` - The number of the argument
    /// * `op` - A comparison operator
    /// * `datum` - A value to compare to
    #[must_use]
    pub const fn new(arg: u32, op: ScmpCompareOp, datum: u64) -> Self {
        if let ScmpCompareOp::MaskedEqual(mask) = op {
            Self(scmp_arg_cmp {
                arg,
                op: op.to_sys(),
                datum_a: mask,
                datum_b: datum,
            })
        } else {
            Self(scmp_arg_cmp {
                arg,
                op: op.to_sys(),
                datum_a: datum,
                datum_b: 0,
            })
        }
    }
}

impl From<ScmpArgCompare> for scmp_arg_cmp {
    fn from(v: ScmpArgCompare) -> scmp_arg_cmp {
        v.0
    }
}

impl From<&ScmpArgCompare> for scmp_arg_cmp {
    fn from(v: &ScmpArgCompare) -> scmp_arg_cmp {
        v.0
    }
}

#[rustfmt::skip]
#[doc(hidden)]
#[macro_export]
macro_rules! __private_scmp_cmp_arg {
    (arg0) => { 0 };
    (arg1) => { 1 };
    (arg2) => { 2 };
    (arg3) => { 3 };
    (arg4) => { 4 };
    (arg5) => { 5 };
}

/// A macro to create [`ScmpArgCompare`] in a more elegant way.
///
/// ```
/// use libseccomp::{ScmpArgCompare, ScmpCompareOp, scmp_cmp};
///
/// assert_eq!(
///     scmp_cmp!($arg0 != 123),
///     ScmpArgCompare::new(0, ScmpCompareOp::NotEqual, 123),
/// );
/// assert_eq!(
///     scmp_cmp!($arg1 < 123),
///     ScmpArgCompare::new(1, ScmpCompareOp::Less, 123),
/// );
/// assert_eq!(
///     scmp_cmp!($arg2 <= 123),
///     ScmpArgCompare::new(2, ScmpCompareOp::LessOrEqual, 123),
/// );
/// assert_eq!(
///     scmp_cmp!($arg3 == 123),
///     ScmpArgCompare::new(3, ScmpCompareOp::Equal, 123),
/// );
/// assert_eq!(
///     scmp_cmp!($arg4 >= 123),
///     ScmpArgCompare::new(4, ScmpCompareOp::GreaterEqual, 123),
/// );
/// assert_eq!(
///     scmp_cmp!($arg5 > 123),
///     ScmpArgCompare::new(5, ScmpCompareOp::Greater, 123),
/// );
/// assert_eq!(
///     scmp_cmp!($arg0 & 0x0f0 == 123),
///     ScmpArgCompare::new(0, ScmpCompareOp::MaskedEqual(0x0f0), 123),
/// );
/// ```
#[macro_export]
macro_rules! scmp_cmp {
    ($_:tt $arg:tt != $datum:expr) => {
        $crate::ScmpArgCompare::new(
            $crate::__private_scmp_cmp_arg!($arg),
            $crate::ScmpCompareOp::NotEqual,
            $datum,
        )
    };
    ($_:tt $arg:tt < $datum:expr) => {
        $crate::ScmpArgCompare::new(
            $crate::__private_scmp_cmp_arg!($arg),
            $crate::ScmpCompareOp::Less,
            $datum,
        )
    };
    ($_:tt $arg:tt <= $datum:expr) => {
        $crate::ScmpArgCompare::new(
            $crate::__private_scmp_cmp_arg!($arg),
            $crate::ScmpCompareOp::LessOrEqual,
            $datum,
        )
    };
    ($_:tt $arg:tt == $datum:expr) => {
        $crate::ScmpArgCompare::new(
            $crate::__private_scmp_cmp_arg!($arg),
            $crate::ScmpCompareOp::Equal,
            $datum,
        )
    };
    ($_:tt $arg:tt >= $datum:expr) => {
        $crate::ScmpArgCompare::new(
            $crate::__private_scmp_cmp_arg!($arg),
            $crate::ScmpCompareOp::GreaterEqual,
            $datum,
        )
    };
    ($_:tt $arg:tt > $datum:expr) => {
        $crate::ScmpArgCompare::new(
            $crate::__private_scmp_cmp_arg!($arg),
            $crate::ScmpCompareOp::Greater,
            $datum,
        )
    };
    ($_:tt $arg:tt & $mask:tt == $datum:expr) => {
        $crate::ScmpArgCompare::new(
            $crate::__private_scmp_cmp_arg!($arg),
            $crate::ScmpCompareOp::MaskedEqual(
                #[allow(unused_parens)]
                $mask,
            ),
            $datum,
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scmpargcompare() {
        assert_eq!(
            ScmpArgCompare::new(0, ScmpCompareOp::NotEqual, 8),
            ScmpArgCompare(scmp_arg_cmp {
                arg: 0,
                op: scmp_compare::SCMP_CMP_NE,
                datum_a: 8,
                datum_b: 0,
            })
        );
        assert_eq!(
            ScmpArgCompare::new(0, ScmpCompareOp::MaskedEqual(0b0010), 2),
            ScmpArgCompare(scmp_arg_cmp {
                arg: 0,
                op: scmp_compare::SCMP_CMP_MASKED_EQ,
                datum_a: 0b0010,
                datum_b: 2,
            })
        );
        assert_eq!(
            scmp_arg_cmp::from(ScmpArgCompare::new(0, ScmpCompareOp::NotEqual, 8)),
            scmp_arg_cmp {
                arg: 0,
                op: scmp_compare::SCMP_CMP_NE,
                datum_a: 8,
                datum_b: 0,
            }
        );
        assert_eq!(
            scmp_arg_cmp::from(&ScmpArgCompare::new(0, ScmpCompareOp::NotEqual, 8)),
            scmp_arg_cmp {
                arg: 0,
                op: scmp_compare::SCMP_CMP_NE,
                datum_a: 8,
                datum_b: 0,
            }
        );
    }
}
