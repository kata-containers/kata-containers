// SPDX-License-Identifier: MIT

use anyhow::anyhow;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("Encode error occurred: {inner}")]
pub struct EncodeError {
    inner: anyhow::Error,
}

impl From<&'static str> for EncodeError {
    fn from(msg: &'static str) -> Self {
        EncodeError {
            inner: anyhow!(msg),
        }
    }
}

impl From<String> for EncodeError {
    fn from(msg: String) -> Self {
        EncodeError {
            inner: anyhow!(msg),
        }
    }
}

impl From<anyhow::Error> for EncodeError {
    fn from(inner: anyhow::Error) -> EncodeError {
        EncodeError { inner }
    }
}

#[derive(Debug, Error)]
#[error("Decode error occurred: {inner}")]
pub struct DecodeError {
    inner: anyhow::Error,
}

impl From<&'static str> for DecodeError {
    fn from(msg: &'static str) -> Self {
        DecodeError {
            inner: anyhow!(msg),
        }
    }
}

impl From<String> for DecodeError {
    fn from(msg: String) -> Self {
        DecodeError {
            inner: anyhow!(msg),
        }
    }
}

impl From<anyhow::Error> for DecodeError {
    fn from(inner: anyhow::Error) -> DecodeError {
        DecodeError { inner }
    }
}
