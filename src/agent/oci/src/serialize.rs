// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use serde::{Deserialize, Serialize};
use serde_json;

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::fs::File;
use std::io;

pub type Result<T> = std::result::Result<T, SerializeError>;

#[derive(Debug)]
pub enum SerializeError {
    Io(io::Error),
    Json(serde_json::Error),
}

impl Display for SerializeError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match *self {
            SerializeError::Io(ref e) => e.fmt(f),
            SerializeError::Json(ref e) => e.fmt(f),
        }
    }
}

impl Error for SerializeError {
    fn description(&self) -> &str {
        match *self {
            SerializeError::Io(ref e) => e.description(),
            SerializeError::Json(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&dyn Error> {
        match *self {
            SerializeError::Io(ref e) => Some(e),
            SerializeError::Json(ref e) => Some(e),
        }
    }
}

impl From<io::Error> for SerializeError {
    fn from(e: io::Error) -> SerializeError {
        SerializeError::Io(e)
    }
}

impl From<serde_json::Error> for SerializeError {
    fn from(e: serde_json::Error) -> SerializeError {
        SerializeError::Json(e)
    }
}

pub fn to_writer<W, T>(o: &T, w: W) -> Result<()>
where
    W: io::Write,
    T: Serialize,
{
    Ok(serde_json::to_writer(w, o)?)
}

pub fn serialize<T>(o: &T, path: &str) -> Result<()>
where
    T: Serialize,
{
    let f = File::create(path)?;
    Ok(serde_json::to_writer(f, o)?)
}

pub fn to_string<T>(o: &T) -> Result<String>
where
    T: Serialize,
{
    Ok(serde_json::to_string(o)?)
}

pub fn deserialize<T>(path: &str) -> Result<T>
where
    for<'a> T: Deserialize<'a>,
{
    let f = File::open(path)?;
    Ok(serde_json::from_reader(f)?)
}
