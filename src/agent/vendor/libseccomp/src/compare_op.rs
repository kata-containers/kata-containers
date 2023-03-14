// SPDX-License-Identifier: Apache-2.0 or MIT
//
// Copyright 2021 Sony Group Corporation
//

use crate::error::ErrorKind::*;
use crate::error::{Result, SeccompError};
use libseccomp_sys::*;
use std::str::FromStr;

/// Represents a comparison operator which can be used in a filter rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ScmpCompareOp {
    /// Not equal
    NotEqual,
    /// Less than
    Less,
    /// Less than or equal
    LessOrEqual,
    /// Equal
    Equal,
    /// Greater than or equal
    GreaterEqual,
    /// Greater than
    Greater,
    /// Masked equality
    ///
    /// This works like `Equal` with the exception that the syscall argument is
    /// masked with `mask` via an bitwise AND (i.e. you can check specific bits in the
    /// argument).
    MaskedEqual(#[doc = "mask"] u64),
}

impl ScmpCompareOp {
    pub(crate) const fn to_sys(self) -> scmp_compare {
        match self {
            Self::NotEqual => scmp_compare::SCMP_CMP_NE,
            Self::Less => scmp_compare::SCMP_CMP_LT,
            Self::LessOrEqual => scmp_compare::SCMP_CMP_LE,
            Self::Equal => scmp_compare::SCMP_CMP_EQ,
            Self::GreaterEqual => scmp_compare::SCMP_CMP_GE,
            Self::Greater => scmp_compare::SCMP_CMP_GT,
            Self::MaskedEqual(_) => scmp_compare::SCMP_CMP_MASKED_EQ,
        }
    }
}

impl FromStr for ScmpCompareOp {
    type Err = SeccompError;

    /// Converts string seccomp comparison operator to `ScmpCompareOp`.
    ///
    /// # Arguments
    ///
    /// * `cmp_op` - A string comparison operator, e.g. `SCMP_CMP_*`.
    ///
    /// See the [`seccomp_rule_add(3)`] man page for details on valid comparison operator values.
    ///
    /// [`seccomp_rule_add(3)`]: https://www.man7.org/linux/man-pages/man3/seccomp_rule_add.3.html
    ///
    /// # Errors
    ///
    /// If an invalid comparison operator is specified, an error will be returned.
    fn from_str(cmp_op: &str) -> Result<Self> {
        match cmp_op {
            "SCMP_CMP_NE" => Ok(Self::NotEqual),
            "SCMP_CMP_LT" => Ok(Self::Less),
            "SCMP_CMP_LE" => Ok(Self::LessOrEqual),
            "SCMP_CMP_EQ" => Ok(Self::Equal),
            "SCMP_CMP_GE" => Ok(Self::GreaterEqual),
            "SCMP_CMP_GT" => Ok(Self::Greater),
            "SCMP_CMP_MASKED_EQ" => Ok(Self::MaskedEqual(u64::default())),
            _ => Err(SeccompError::new(ParseError)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_compare_op() {
        let test_data = [
            ("SCMP_CMP_NE", ScmpCompareOp::NotEqual),
            ("SCMP_CMP_LT", ScmpCompareOp::Less),
            ("SCMP_CMP_LE", ScmpCompareOp::LessOrEqual),
            ("SCMP_CMP_EQ", ScmpCompareOp::Equal),
            ("SCMP_CMP_GE", ScmpCompareOp::GreaterEqual),
            ("SCMP_CMP_GT", ScmpCompareOp::Greater),
            (
                "SCMP_CMP_MASKED_EQ",
                ScmpCompareOp::MaskedEqual(u64::default()),
            ),
        ];

        for data in test_data {
            assert_eq!(
                ScmpCompareOp::from_str(data.0).unwrap().to_sys(),
                data.1.to_sys()
            );
        }
        assert!(ScmpCompareOp::from_str("SCMP_INVALID_FLAG").is_err());
    }
}
