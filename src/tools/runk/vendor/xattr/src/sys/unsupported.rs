use std::ffi::{OsStr, OsString};
use std::io;
use std::os::unix::io::RawFd;
use std::path::Path;

use UnsupportedPlatformError;

/// An iterator over a set of extended attributes names.
#[derive(Clone, Debug)]
pub struct XAttrs;

impl Iterator for XAttrs {
    type Item = OsString;
    fn next(&mut self) -> Option<OsString> {
        unreachable!("this should never exist")
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        unreachable!("this should never exist")
    }
}

pub fn get_fd(_: RawFd, _: &OsStr) -> io::Result<Vec<u8>> {
    Err(io::Error::new(
        io::ErrorKind::Other,
        UnsupportedPlatformError,
    ))
}

pub fn set_fd(_: RawFd, _: &OsStr, _: &[u8]) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Other,
        UnsupportedPlatformError,
    ))
}

pub fn remove_fd(_: RawFd, _: &OsStr) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Other,
        UnsupportedPlatformError,
    ))
}

pub fn list_fd(_: RawFd) -> io::Result<XAttrs> {
    Err(io::Error::new(
        io::ErrorKind::Other,
        UnsupportedPlatformError,
    ))
}

pub fn get_path(_: &Path, _: &OsStr) -> io::Result<Vec<u8>> {
    Err(io::Error::new(
        io::ErrorKind::Other,
        UnsupportedPlatformError,
    ))
}

pub fn set_path(_: &Path, _: &OsStr, _: &[u8]) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Other,
        UnsupportedPlatformError,
    ))
}

pub fn remove_path(_: &Path, _: &OsStr) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Other,
        UnsupportedPlatformError,
    ))
}

pub fn list_path(_: &Path) -> io::Result<XAttrs> {
    Err(io::Error::new(
        io::ErrorKind::Other,
        UnsupportedPlatformError,
    ))
}
