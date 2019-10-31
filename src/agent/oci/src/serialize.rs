// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use serde;
use serde::{Deserialize, Serialize};
use serde_json;

use std::error::Error;
use std::fmt::{self, Formatter};
use std::fs::File;
use std::io;

#[derive(Debug)]
pub enum SerializeError {
    Io(io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for SerializeError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
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

pub fn to_writer<W, T>(o: &T, mut w: W) -> Result<(), SerializeError>
where
    W: io::Write,
    T: Serialize,
{
    Ok(serde_json::to_writer(&mut w, &o)?)
}

pub fn serialize<T>(o: &T, path: &str) -> Result<(), SerializeError>
where
    T: Serialize,
{
    let mut f = File::create(path)?;
    Ok(serde_json::to_writer(&mut f, &o)?)
}

pub fn to_string<T>(o: &T) -> Result<String, SerializeError>
where
    T: Serialize,
{
    Ok(serde_json::to_string(&o)?)
}

pub fn deserialize<T>(path: &str) -> Result<T, SerializeError>
where
    for<'a> T: Deserialize<'a>,
{
    let f = File::open(path)?;
    Ok(serde_json::from_reader(&f)?)
}
