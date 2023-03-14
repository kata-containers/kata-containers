//! Decoding functions for PEM-encoded data
//!
//! A PEM object is a container, which can store (amongst other formats) a public X.509
//! Certificate, or a CRL, etc. It contains only printable characters.
//! PEM-encoded binary data is essentially a beginning and matching end tag that encloses
//! base64-encoded binary data (see:
//! <https://en.wikipedia.org/wiki/Privacy-enhanced_Electronic_Mail>).
//!
//! # Examples
//!
//! To parse a certificate in PEM format, first create the `Pem` object, then decode
//! contents:
//!
//! ```rust,no_run
//! use x509_parser::pem::Pem;
//! use x509_parser::x509::X509Version;
//!
//! static IGCA_PEM: &str = "../assets/IGC_A.pem";
//!
//! # fn main() {
//! let data = std::fs::read(IGCA_PEM).expect("Could not read file");
//! for pem in Pem::iter_from_buffer(&data) {
//!     let pem = pem.expect("Reading next PEM block failed");
//!     let x509 = pem.parse_x509().expect("X.509: decoding DER failed");
//!     assert_eq!(x509.tbs_certificate.version, X509Version::V3);
//! }
//! # }
//! ```
//!
//! This is the most direct method to parse PEM data.
//!
//! Another method to parse the certificate is to use `parse_x509_pem`:
//!
//! ```rust,no_run
//! use x509_parser::pem::parse_x509_pem;
//! use x509_parser::parse_x509_certificate;
//!
//! static IGCA_PEM: &[u8] = include_bytes!("../assets/IGC_A.pem");
//!
//! # fn main() {
//! let res = parse_x509_pem(IGCA_PEM);
//! match res {
//!     Ok((rem, pem)) => {
//!         assert!(rem.is_empty());
//!         //
//!         assert_eq!(pem.label, String::from("CERTIFICATE"));
//!         //
//!         let res_x509 = parse_x509_certificate(&pem.contents);
//!         assert!(res_x509.is_ok());
//!     },
//!     _ => panic!("PEM parsing failed: {:?}", res),
//! }
//! # }
//! ```
//!
//! Note that all methods require to store the `Pem` object in a variable, mainly because decoding
//! the PEM object requires allocation of buffers, and that the lifetime of X.509 certificates will
//! be bound to these buffers.

use crate::certificate::X509Certificate;
use crate::error::{PEMError, X509Error};
use crate::parse_x509_certificate;
use nom::{Err, IResult};
use std::io::{BufRead, Cursor, Seek, SeekFrom};

/// Representation of PEM data
#[derive(Clone, PartialEq, Debug)]
pub struct Pem {
    /// The PEM label
    pub label: String,
    /// The PEM decoded data
    pub contents: Vec<u8>,
}

#[deprecated(since = "0.8.3", note = "please use `parse_x509_pem` instead")]
pub fn pem_to_der(i: &[u8]) -> IResult<&[u8], Pem, PEMError> {
    parse_x509_pem(i)
}

/// Read a PEM-encoded structure, and decode the base64 data
///
/// Return a structure describing the PEM object: the enclosing tag, and the data.
/// Allocates a new buffer for the decoded data.
///
/// Note that only the *first* PEM block is decoded. To iterate all blocks from PEM data,
/// use [`Pem::iter_from_buffer`].
///
/// For X.509 (`CERTIFICATE` tag), the data is a certificate, encoded in DER. To parse the
/// certificate content, use `Pem::parse_x509` or `parse_x509_certificate`.
pub fn parse_x509_pem(i: &[u8]) -> IResult<&'_ [u8], Pem, PEMError> {
    let reader = Cursor::new(i);
    let res = Pem::read(reader);
    match res {
        Ok((pem, bytes_read)) => Ok((&i[bytes_read..], pem)),
        Err(e) => Err(Err::Error(e)),
    }
}

impl Pem {
    /// Read the next PEM-encoded structure, and decode the base64 data
    ///
    /// Returns the certificate (encoded in DER) and the number of bytes read.
    /// Allocates a new buffer for the decoded data.
    ///
    /// Note that a PEM file can contain multiple PEM blocks. This function returns the
    /// *first* decoded object, starting from the current reader position.
    /// To get all objects, call this function repeatedly until `PEMError::MissingHeader`
    /// is returned.
    ///
    /// # Examples
    /// ```
    /// let file = std::fs::File::open("assets/certificate.pem").unwrap();
    /// let subject = x509_parser::pem::Pem::read(std::io::BufReader::new(file))
    ///      .unwrap().0
    ///     .parse_x509().unwrap()
    ///     .tbs_certificate.subject.to_string();
    /// assert_eq!(subject, "CN=lists.for-our.info");
    /// ```
    pub fn read(mut r: impl BufRead + Seek) -> Result<(Pem, usize), PEMError> {
        let mut line = String::new();
        let label = loop {
            let num_bytes = r.read_line(&mut line)?;
            if num_bytes == 0 {
                // EOF
                return Err(PEMError::MissingHeader);
            }
            if !line.starts_with("-----BEGIN ") {
                line.clear();
                continue;
            }
            let mut iter = line.split_whitespace();
            let label = iter.nth(1).ok_or(PEMError::InvalidHeader)?;
            break label;
        };
        let label = label.split('-').next().ok_or(PEMError::InvalidHeader)?;
        let mut s = String::new();
        loop {
            let mut l = String::new();
            let num_bytes = r.read_line(&mut l)?;
            if num_bytes == 0 {
                return Err(PEMError::IncompletePEM);
            }
            if l.starts_with("-----END ") {
                // finished reading
                break;
            }
            s.push_str(l.trim_end());
        }

        let contents = base64::decode(&s).or(Err(PEMError::Base64DecodeError))?;
        let pem = Pem {
            label: label.to_string(),
            contents,
        };
        Ok((pem, r.seek(SeekFrom::Current(0))? as usize))
    }

    /// Decode the PEM contents into a X.509 object
    pub fn parse_x509(&self) -> Result<X509Certificate, ::nom::Err<X509Error>> {
        parse_x509_certificate(&self.contents).map(|(_, x509)| x509)
    }

    /// Returns an iterator over the PEM-encapsulated parts of a buffer
    ///
    /// Only the sections enclosed in blocks starting with `-----BEGIN xxx-----`
    /// and ending with `-----END xxx-----` will be considered.
    /// Lines before, between or after such blocks will be ignored.
    ///
    /// The iterator is fallible: `next()` returns a `Result<Pem, PEMError>` object.
    /// An error indicates a block is present but invalid.
    ///
    /// If the buffer does not contain any block, iterator will be empty.
    pub fn iter_from_buffer(i: &[u8]) -> PemIterator<Cursor<&[u8]>> {
        let reader = Cursor::new(i);
        PemIterator { reader }
    }

    /// Returns an iterator over the PEM-encapsulated parts of a reader
    ///
    /// Only the sections enclosed in blocks starting with `-----BEGIN xxx-----`
    /// and ending with `-----END xxx-----` will be considered.
    /// Lines before, between or after such blocks will be ignored.
    ///
    /// The iterator is fallible: `next()` returns a `Result<Pem, PEMError>` object.
    /// An error indicates a block is present but invalid.
    ///
    /// If the reader does not contain any block, iterator will be empty.
    pub fn iter_from_reader<R: BufRead + Seek>(reader: R) -> PemIterator<R> {
        PemIterator { reader }
    }
}

/// Iterator over PEM-encapsulated blocks
///
/// Only the sections enclosed in blocks starting with `-----BEGIN xxx-----`
/// and ending with `-----END xxx-----` will be considered.
/// Lines before, between or after such blocks will be ignored.
///
/// The iterator is fallible: `next()` returns a `Result<Pem, PEMError>` object.
/// An error indicates a block is present but invalid.
///
/// If the buffer does not contain any block, iterator will be empty.
#[allow(missing_debug_implementations)]
pub struct PemIterator<Reader: BufRead + Seek> {
    reader: Reader,
}

impl<R: BufRead + Seek> Iterator for PemIterator<R> {
    type Item = Result<Pem, PEMError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Ok(&[]) = self.reader.fill_buf() {
            return None;
        }
        let reader = self.reader.by_ref();
        let r = Pem::read(reader).map(|(pem, _)| pem);
        if let Err(PEMError::MissingHeader) = r {
            None
        } else {
            Some(r)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_pem_from_file() {
        let file = std::io::BufReader::new(std::fs::File::open("assets/certificate.pem").unwrap());
        let subject = Pem::read(file)
            .unwrap()
            .0
            .parse_x509()
            .unwrap()
            .tbs_certificate
            .subject
            .to_string();
        assert_eq!(subject, "CN=lists.for-our.info");
    }
}
