//! Privacy-Enhanced Mail (PEM) format utilities
//!
//! Based on the RFC-7468
//! ([Textual Encodings of PKIX, PKCS, and CMS Structures](https://tools.ietf.org/html/rfc7468)).

use base64::DecodeError;
use std::borrow::Cow;
use std::fmt;
use std::io::BufRead;
use std::str::FromStr;
use thiserror::Error;

const PEM_HEADER_START: &str = "-----BEGIN";
const PEM_FOOTER_START: &str = "-----END";
const PEM_DASHES_BOUNDARIES: &str = "-----";

#[derive(Debug, Clone, Error)]
pub enum PemError {
    /// header not found
    #[error("header not found")]
    HeaderNotFound,

    /// invalid pem header
    #[error("invalid pem header")]
    InvalidHeader,

    /// footer not found
    #[error("footer not found")]
    FooterNotFound,

    /// couldn't decode base64
    #[error("couldn't decode base64: {source}")]
    Base64Decoding { source: DecodeError },
}

/// Privacy-Enhanced Mail (PEM) format structured representation
#[derive(Debug, Clone, PartialEq)]
pub struct Pem<'a> {
    label: String,
    data: Cow<'a, [u8]>,
}

impl<'a> Pem<'a> {
    pub fn new<S: Into<String>, D: Into<Cow<'a, [u8]>>>(label: S, data: D) -> Self {
        Self {
            label: label.into(),
            data: data.into(),
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn into_data(self) -> Cow<'a, [u8]> {
        self.data
    }
}

impl Pem<'static> {
    pub fn read_from(reader: &mut impl BufRead) -> Result<Self, PemError> {
        read_pem(reader)
    }
}

impl FromStr for Pem<'static> {
    type Err = PemError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_pem(s.as_bytes())
    }
}

impl fmt::Display for Pem<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} {}-----", PEM_HEADER_START, self.label)?;

        let encoded = base64::encode(&self.data);
        let bytes = encoded.as_bytes();
        for chunk in bytes.chunks(64) {
            let chunk = std::str::from_utf8(chunk).map_err(|_| fmt::Error)?;
            writeln!(f, "{}", chunk)?;
        }

        write!(f, "{} {}-----", PEM_FOOTER_START, self.label)?;

        Ok(())
    }
}

impl From<Pem<'_>> for String {
    fn from(pem: Pem<'_>) -> Self {
        pem.to_string()
    }
}

/// Parse a PEM-encoded stream from a [u8] representation
///
/// If the input contains line ending characters (`\r`, `\n`), a copy of input
/// is allocated striping these. If you can strip these with minimal data copy
/// you should do it beforehand.
pub fn parse_pem<T: ?Sized + AsRef<[u8]>>(input: &T) -> Result<Pem<'static>, PemError> {
    parse_pem_impl(input.as_ref())
}

fn parse_pem_impl(input: &[u8]) -> Result<Pem<'static>, PemError> {
    let header_start_idx = h_find(input, PEM_HEADER_START.as_bytes()).ok_or(PemError::HeaderNotFound)?;

    let label_start_idx = header_start_idx + PEM_HEADER_START.as_bytes().len();
    let label_end_idx = h_find(&input[label_start_idx..], b"-").ok_or(PemError::InvalidHeader)? + label_start_idx;
    let label = String::from_utf8_lossy(&input[label_start_idx..label_end_idx])
        .trim()
        .to_owned();

    let header_end_idx = h_find(&input[label_end_idx..], PEM_DASHES_BOUNDARIES.as_bytes())
        .ok_or(PemError::InvalidHeader)?
        + label_end_idx
        + PEM_DASHES_BOUNDARIES.as_bytes().len();

    let footer_start_idx =
        h_find(&input[header_end_idx..], PEM_FOOTER_START.as_bytes()).ok_or(PemError::FooterNotFound)? + header_end_idx;

    let raw_data = &input[header_end_idx..footer_start_idx];

    let data = if h_find(raw_data, b"\n").is_some() {
        // Line ending characters should be striped... Sadly, this means we need to copy and allocate.
        let striped_raw_data: Vec<u8> = raw_data
            .iter()
            .copied()
            .filter(|&byte| byte != b'\r' && byte != b'\n')
            .collect();
        base64::decode(&striped_raw_data).map_err(|source| PemError::Base64Decoding { source })?
    } else {
        // Can be decoded as is!
        base64::decode(raw_data).map_err(|source| PemError::Base64Decoding { source })?
    };

    Ok(Pem {
        label,
        data: Cow::Owned(data),
    })
}

fn h_find(buffer: &[u8], value: &[u8]) -> Option<usize> {
    buffer.windows(value.len()).position(|window| window == value)
}

/// Parse a PEM-encoded stream from a BufRead object.
///
/// Maybe slower than the AsRef<[u8]>-based implementation because additional copies are incurred,
/// but in most cases it's probably easier to work with and not that bad anyway.
pub fn read_pem(reader: &mut impl BufRead) -> Result<Pem<'static>, PemError> {
    let mut buf = Vec::with_capacity(1024);

    // skip until start of header
    h_read_until(reader, PEM_HEADER_START.as_bytes(), &mut buf).ok_or(PemError::HeaderNotFound)?;
    buf.clear();

    // read until end of header
    h_read_until(reader, PEM_DASHES_BOUNDARIES.as_bytes(), &mut buf).ok_or(PemError::InvalidHeader)?;
    let buf_utf8 = core::str::from_utf8(&buf).map_err(|_| PemError::InvalidHeader)?;
    let label = buf_utf8.trim_end_matches(PEM_DASHES_BOUNDARIES).trim().to_owned();
    buf.clear();

    // read to footer
    h_read_until(reader, PEM_FOOTER_START.as_bytes(), &mut buf).ok_or(PemError::FooterNotFound)?;
    let base64_data: Vec<u8> = h_trim_end_matches(&buf, PEM_FOOTER_START.as_bytes())
        .iter()
        .cloned()
        .filter(|&byte| byte != b'\r' && byte != b'\n')
        .collect();
    let data = base64::decode(&base64_data).map_err(|source| PemError::Base64Decoding { source })?;

    // read until end of footer
    h_read_until(reader, PEM_DASHES_BOUNDARIES.as_bytes(), &mut buf).ok_or(PemError::FooterNotFound)?;

    Ok(Pem {
        label,
        data: Cow::Owned(data),
    })
}

// Helper to read until some pattern is matched. Returns None on any error
// (cannot be copy pasted for any purpose and should stay private!).
fn h_read_until(reader: &mut impl BufRead, pat: &[u8], buf: &mut Vec<u8>) -> Option<usize> {
    let mut read = 0;
    let first_delim = *pat.first()?;
    'outer: loop {
        read += reader.read_until(first_delim, buf).ok()?;

        for &next_delim in &pat[1..] {
            let mut next = [0];
            reader.read_exact(&mut next).ok()?;
            buf.push(next[0]);
            read += 1;

            if next[0] != next_delim {
                continue 'outer;
            }
        }

        break Some(read);
    }
}

// Helper to trim trailing characters matching the given pattern for bytes slice
fn h_trim_end_matches<'a>(slice: &'a [u8], pat: &[u8]) -> &'a [u8] {
    for (&slice_elem, &pat_elem) in slice.iter().rev().zip(pat.iter().rev()) {
        if slice_elem != pat_elem {
            return slice; // pattern doesn't match, return all the slice
        }
    }

    // pattern did match, return sub-slice
    &slice[..slice.len() - pat.len()]
}

/// Build a PEM-encoded structure into a String.
pub fn to_pem<S, T>(label: S, data: &T) -> String
where
    S: Into<String>,
    T: ?Sized + AsRef<[u8]>,
{
    Pem::new(label, data.as_ref()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    const PEM_BYTES: &[u8] = include_bytes!("../../test_assets/intermediate_ca.crt");
    const PEM_STR: &str = include_str!("../../test_assets/intermediate_ca.crt");

    #[test]
    fn parse() {
        let pem_from_bytes = parse_pem(PEM_BYTES).unwrap();
        assert_eq!(pem_from_bytes.label, "CERTIFICATE");

        let pem_from_str = PEM_STR.parse::<Pem>().unwrap();
        pretty_assertions::assert_eq!(pem_from_bytes, pem_from_str);
    }

    #[test]
    fn reader_based() {
        let mut reader = BufReader::new(PEM_BYTES);

        let pem_from_reader = read_pem(&mut reader).unwrap();
        assert_eq!(pem_from_reader.label, "CERTIFICATE");

        let pem_from_str = PEM_STR.parse::<Pem>().unwrap();
        pretty_assertions::assert_eq!(pem_from_reader, pem_from_str);
    }

    // This test should not run on Windows. writeln! add `/r` ending character to Pem in String format on Windows targets.
    #[cfg(not(windows))]
    #[test]
    fn to_string() {
        let pem = PEM_STR.parse::<Pem>().unwrap();
        let reconverted_pem = pem.to_string();
        pretty_assertions::assert_eq!(reconverted_pem, PEM_STR);
    }

    const FLATTENED_PEM: &str = "-----BEGIN GARBAGE-----GARBAGE-----END GARBAGE-----";

    #[test]
    fn flattened() {
        FLATTENED_PEM.parse::<Pem>().unwrap();
        read_pem(&mut BufReader::new(FLATTENED_PEM.as_bytes())).unwrap();
    }

    const MULTIPLE_PEM: &str = "-----BEGIN GARBAGE1-----GARBAGE-----END GARBAGE1-----\
         -----BEGIN GARBAGE2-----GARBAGE-----END GARBAGE2-----";

    #[test]
    fn multiple() {
        // reading multiple PEM from some bytes stream is easier with read-based API
        let mut reader = BufReader::new(MULTIPLE_PEM.as_bytes());
        let pem1 = read_pem(&mut reader).unwrap();
        assert_eq!(pem1.label, "GARBAGE1");
        let pem2 = read_pem(&mut reader).unwrap();
        assert_eq!(pem2.label, "GARBAGE2");
    }
}
