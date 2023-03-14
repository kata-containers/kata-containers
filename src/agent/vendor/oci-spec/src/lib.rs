#![deny(missing_docs, warnings)]
#![doc = include_str!("../README.md")]

#[cfg(feature = "distribution")]
pub mod distribution;
mod error;
#[cfg(feature = "image")]
pub mod image;
#[cfg(feature = "runtime")]
pub mod runtime;

use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use serde::{de::DeserializeOwned, Serialize};

pub use error::*;

fn from_file<P: AsRef<Path>, T: DeserializeOwned>(path: P) -> Result<T> {
    let path = path.as_ref();
    let manifest_file = std::io::BufReader::new(fs::File::open(path)?);
    let manifest = serde_json::from_reader(manifest_file)?;
    Ok(manifest)
}

fn from_reader<R: Read, T: DeserializeOwned>(reader: R) -> Result<T> {
    let manifest = serde_json::from_reader(reader)?;
    Ok(manifest)
}

fn to_file<P: AsRef<Path>, T: Serialize>(item: &T, path: P, pretty: bool) -> Result<()> {
    let path = path.as_ref();
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    let file = std::io::BufWriter::new(file);

    match pretty {
        true => serde_json::to_writer_pretty(file, item)?,
        false => serde_json::to_writer(file, item)?,
    }

    Ok(())
}

fn to_writer<W: Write, T: Serialize>(item: &T, writer: &mut W, pretty: bool) -> Result<()> {
    match pretty {
        true => serde_json::to_writer_pretty(writer, item)?,
        false => serde_json::to_writer(writer, item)?,
    }

    Ok(())
}

fn to_string<T: Serialize>(item: &T, pretty: bool) -> Result<String> {
    Ok(match pretty {
        true => serde_json::to_string_pretty(item)?,
        false => serde_json::to_string(item)?,
    })
}
