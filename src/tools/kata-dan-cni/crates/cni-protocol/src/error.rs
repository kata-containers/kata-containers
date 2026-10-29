// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CniErrorCode {
    IncompatibleVersion = 1,
    InvalidEnvironment = 4,
    IoFailure = 5,
    DecodeFailure = 6,
    InvalidNetworkConfig = 7,
    RetryLater = 11,
    DelegateProtocol = 100,
}

impl CniErrorCode {
    #[must_use]
    pub const fn value(self) -> u32 {
        self as u32
    }
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CniErrorResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cni_version: Option<String>,
    pub code: u32,
    pub msg: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub details: String,
}

#[derive(Debug, Eq, PartialEq)]
pub struct CniError {
    response: CniErrorResponse,
}

impl CniError {
    #[must_use]
    pub fn new(code: CniErrorCode, message: impl Into<String>, details: impl Into<String>) -> Self {
        Self::from_code(code.value(), message, details)
    }

    #[must_use]
    pub fn from_code(code: u32, message: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            response: CniErrorResponse {
                cni_version: None,
                code,
                msg: message.into(),
                details: details.into(),
            },
        }
    }

    #[must_use]
    pub const fn code(&self) -> u32 {
        self.response.code
    }

    #[must_use]
    pub fn response(&self) -> &CniErrorResponse {
        &self.response
    }

    #[must_use]
    pub fn with_cni_version(mut self, cni_version: impl Into<String>) -> Self {
        self.response.cni_version = Some(cni_version.into());
        self
    }
}

impl Display for CniError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} (code {})",
            self.response.msg, self.response.code
        )?;
        if !self.response.details.is_empty() {
            write!(formatter, ": {}", self.response.details)?;
        }
        Ok(())
    }
}

impl Error for CniError {}
