// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use crate::{CniError, CniErrorCode};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CniCommand {
    Add,
    Del,
    Check,
    Version,
}

impl CniCommand {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Add => "ADD",
            Self::Del => "DEL",
            Self::Check => "CHECK",
            Self::Version => "VERSION",
        }
    }
}

impl Display for CniCommand {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for CniCommand {
    type Err = CniError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "ADD" => Ok(Self::Add),
            "DEL" => Ok(Self::Del),
            "CHECK" => Ok(Self::Check),
            "VERSION" => Ok(Self::Version),
            _ => Err(CniError::new(
                CniErrorCode::InvalidEnvironment,
                "unknown CNI command",
                format!("CNI_COMMAND={value:?}"),
            )),
        }
    }
}
