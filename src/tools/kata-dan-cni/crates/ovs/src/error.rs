// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OvsError {
    InvalidInput(String),
    /// An OVSDB operation reported a failed zero-timeout transaction guard.
    TransactionConflict(String),
    OperationFailed(String),
}

impl Display for OvsError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "invalid OVS input: {message}"),
            Self::TransactionConflict(message) => {
                write!(formatter, "OVS transaction conflict: {message}")
            }
            Self::OperationFailed(message) => write!(formatter, "OVS operation failed: {message}"),
        }
    }
}

impl Error for OvsError {}
