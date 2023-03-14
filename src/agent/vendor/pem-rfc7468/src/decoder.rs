//! Decoder for PEM encapsulated data.
//!
//! From RFC 7468 Section 2:
//!
//! > Textual encoding begins with a line comprising "-----BEGIN ", a
//! > label, and "-----", and ends with a line comprising "-----END ", a
//! > label, and "-----".  Between these lines, or "encapsulation
//! > boundaries", are base64-encoded data according to Section 4 of
//! > [RFC 4648].
//!
//! [RFC 4648]: https://datatracker.ietf.org/doc/html/rfc4648

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::{grammar, Error, Result, POST_ENCAPSULATION_BOUNDARY, PRE_ENCAPSULATION_BOUNDARY};
use base64ct::{Base64, Encoding};
use core::str;

/// Decode a PEM document according to RFC 7468's "Strict" grammar.
///
/// On success, writes the decoded document into the provided buffer, returning
/// the decoded label and the portion of the provided buffer containing the
/// decoded message.
pub fn decode<'i, 'o>(pem: &'i [u8], buf: &'o mut [u8]) -> Result<(&'i str, &'o [u8])> {
    Decoder::new().decode(pem, buf)
}

/// Decode a PEM document according to RFC 7468's "Strict" grammar, returning
/// the result as a [`Vec`] upon success.
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub fn decode_vec(pem: &[u8]) -> Result<(&str, Vec<u8>)> {
    Decoder::new().decode_vec(pem)
}

/// Decode the encapsulation boundaries of a PEM document according to RFC 7468's "Strict" grammar.
///
/// On success, returning the decoded label.
pub fn decode_label(pem: &[u8]) -> Result<&str> {
    Ok(Encapsulation::try_from(pem)?.label())
}

/// PEM decoder.
///
/// This type provides a degree of configurability for how PEM is decoded.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Decoder {
    /// Number of characters at which to line-wrap Base64-encoded data
    /// (default `64`).
    ///
    /// Must be a multiple of `4`, or otherwise decoding operations will return
    /// `Error::Base64`.
    // TODO(tarcieri): support for wrap widths which aren't multiples of 4?
    pub wrap_width: usize,
}

impl Decoder {
    /// Create a new [`Decoder`] with the default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Decode a PEM document according to RFC 7468's "Strict" grammar.
    ///
    /// On success, writes the decoded document into the provided buffer, returning
    /// the decoded label and the portion of the provided buffer containing the
    /// decoded message.
    pub fn decode<'i, 'o>(&self, pem: &'i [u8], buf: &'o mut [u8]) -> Result<(&'i str, &'o [u8])> {
        let encapsulation = Encapsulation::try_from(pem)?;
        let label = encapsulation.label();
        let decoded_bytes = encapsulation.decode(self, buf)?;
        Ok((label, decoded_bytes))
    }

    /// Decode a PEM document according to RFC 7468's "Strict" grammar, returning
    /// the result as a [`Vec`] upon success.
    #[cfg(feature = "alloc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub fn decode_vec<'a>(&self, pem: &'a [u8]) -> Result<(&'a str, Vec<u8>)> {
        let encapsulation = Encapsulation::try_from(pem)?;
        let label = encapsulation.label();

        // count all chars (gives over-estimation, due to whitespace)
        let max_len = encapsulation.encapsulated_text.len() * 3 / 4;

        let mut result = vec![0u8; max_len];
        let decoded_len = encapsulation.decode(self, &mut result)?.len();

        // Actual encoded length can be slightly shorter than estimated
        // TODO(tarcieri): more reliable length estimation
        result.truncate(decoded_len);
        Ok((label, result))
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self {
            wrap_width: crate::BASE64_WRAP_WIDTH,
        }
    }
}

/// PEM encapsulation parser.
///
/// This parser performs an initial pass over the data, locating the
/// pre-encapsulation (`---BEGIN [...]---`) and post-encapsulation
/// (`---END [...]`) boundaries while attempting to avoid branching
/// on the potentially secret Base64-encoded data encapsulated between
/// the two boundaries.
///
/// It only supports a single encapsulated message at present. Future work
/// could potentially include extending it provide an iterator over a series
/// of encapsulated messages.
#[derive(Copy, Clone, Debug)]
struct Encapsulation<'a> {
    /// Type label extracted from the pre/post-encapsulation boundaries.
    ///
    /// From RFC 7468 Section 2:
    ///
    /// > The type of data encoded is labeled depending on the type label in
    /// > the "-----BEGIN " line (pre-encapsulation boundary).  For example,
    /// > the line may be "-----BEGIN CERTIFICATE-----" to indicate that the
    /// > content is a PKIX certificate (see further below).  Generators MUST
    /// > put the same label on the "-----END " line (post-encapsulation
    /// > boundary) as the corresponding "-----BEGIN " line.  Labels are
    /// > formally case-sensitive, uppercase, and comprised of zero or more
    /// > characters; they do not contain consecutive spaces or hyphen-minuses,
    /// > nor do they contain spaces or hyphen-minuses at either end.  Parsers
    /// > MAY disregard the label in the post-encapsulation boundary instead of
    /// > signaling an error if there is a label mismatch: some extant
    /// > implementations require the labels to match; others do not.
    label: &'a str,

    /// Encapsulated text portion contained between the boundaries.
    ///
    /// This data should be encoded as Base64, however this type performs no
    /// validation of it so it can be handled in constant-time.
    encapsulated_text: &'a [u8],
}

impl<'a> Encapsulation<'a> {
    /// Parse the type label and encapsulated text from between the
    /// pre/post-encapsulation boundaries.
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        // Strip the "preamble": optional text occurring before the pre-encapsulation boundary
        let data = grammar::strip_preamble(data)?;

        // Parse pre-encapsulation boundary (including label)
        let data = data
            .strip_prefix(PRE_ENCAPSULATION_BOUNDARY)
            .ok_or(Error::PreEncapsulationBoundary)?;

        let (label, body) = grammar::split_label(data).ok_or(Error::Label)?;

        let mut body = match grammar::strip_trailing_eol(body).unwrap_or(body) {
            [head @ .., b'-', b'-', b'-', b'-', b'-'] => head,
            _ => return Err(Error::PreEncapsulationBoundary),
        };

        // Ensure body ends with a properly labeled post-encapsulation boundary
        for &slice in [POST_ENCAPSULATION_BOUNDARY, label.as_bytes()].iter().rev() {
            // Ensure the input ends with the post encapsulation boundary as
            // well as a matching label
            if !body.ends_with(slice) {
                return Err(Error::PostEncapsulationBoundary);
            }

            body = body
                .get(..(body.len() - slice.len()))
                .ok_or(Error::PostEncapsulationBoundary)?;
        }

        let encapsulated_text =
            grammar::strip_trailing_eol(body).ok_or(Error::PostEncapsulationBoundary)?;

        Ok(Self {
            label,
            encapsulated_text,
        })
    }

    /// Get the label parsed from the encapsulation boundaries.
    pub fn label(self) -> &'a str {
        self.label
    }

    /// Get an iterator over the (allegedly) Base64-encoded lines of the
    /// encapsulated text.
    pub fn encapsulated_text(self, wrap_width: usize) -> Result<Lines<'a>> {
        if (wrap_width > 0) && (wrap_width % 4 == 0) {
            Ok(Lines {
                bytes: self.encapsulated_text,
                is_start: true,
                wrap_width,
            })
        } else {
            Err(Error::Base64)
        }
    }

    /// Decode the "encapsulated text", i.e. Base64-encoded data which lies between
    /// the pre/post-encapsulation boundaries.
    fn decode<'o>(&self, decoder: &Decoder, buf: &'o mut [u8]) -> Result<&'o [u8]> {
        // Ensure wrap width is supported.
        if (decoder.wrap_width == 0) || (decoder.wrap_width % 4 != 0) {
            return Err(Error::Base64);
        }

        let mut out_len = 0;

        for line in self.encapsulated_text(decoder.wrap_width)? {
            let line = line?;

            match Base64::decode(line, &mut buf[out_len..]) {
                Err(error) => {
                    // in the case that we are decoding the first line
                    // and we error, then attribute the error to an unsupported header
                    // if a colon char is present in the line
                    if out_len == 0 && line.iter().any(|&b| b == grammar::CHAR_COLON) {
                        return Err(Error::HeaderDisallowed);
                    } else {
                        return Err(error.into());
                    }
                }
                Ok(out) => out_len += out.len(),
            }
        }

        Ok(&buf[..out_len])
    }
}

impl<'a> TryFrom<&'a [u8]> for Encapsulation<'a> {
    type Error = Error;

    fn try_from(bytes: &'a [u8]) -> Result<Self> {
        Self::parse(bytes)
    }
}

/// Iterator over the lines in the encapsulated text.
struct Lines<'a> {
    /// Remaining data being iterated over.
    bytes: &'a [u8],

    /// `true` if no lines have been read.
    is_start: bool,

    /// Base64 line-wrapping width in bytes.
    wrap_width: usize,
}

impl<'a> Iterator for Lines<'a> {
    type Item = Result<&'a [u8]>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bytes.len() > self.wrap_width {
            let (line, rest) = self.bytes.split_at(self.wrap_width);
            if let Some(rest) = grammar::strip_leading_eol(rest) {
                self.is_start = false;
                self.bytes = rest;
                Some(Ok(line))
            } else {
                // if bytes remaining does not split at `wrap_width` such
                // that the next char(s) in the rest is vertical whitespace
                // then attribute the error generically as `EncapsulatedText`
                // unless we are at the first line and the line contains a colon
                // then it may be a unsupported header
                Some(Err(
                    if self.is_start && line.iter().any(|&b| b == grammar::CHAR_COLON) {
                        Error::HeaderDisallowed
                    } else {
                        Error::EncapsulatedText
                    },
                ))
            }
        } else if !self.bytes.is_empty() {
            let line = self.bytes;
            self.bytes = &[];
            Some(Ok(line))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Encapsulation;
    use crate::BASE64_WRAP_WIDTH;

    #[test]
    fn pkcs8_example() {
        let pem = include_bytes!("../tests/examples/pkcs8.pem");
        let result = Encapsulation::parse(pem).unwrap();
        assert_eq!(result.label, "PRIVATE KEY");

        let mut lines = result.encapsulated_text(BASE64_WRAP_WIDTH).unwrap();
        assert_eq!(
            lines.next().unwrap().unwrap(),
            &[
                77, 67, 52, 67, 65, 81, 65, 119, 66, 81, 89, 68, 75, 50, 86, 119, 66, 67, 73, 69,
                73, 66, 102, 116, 110, 72, 80, 112, 50, 50, 83, 101, 119, 89, 109, 109, 69, 111,
                77, 99, 88, 56, 86, 119, 73, 52, 73, 72, 119, 97, 113, 100, 43, 57, 76, 70, 80,
                106, 47, 49, 53, 101, 113, 70
            ]
        );
        assert_eq!(lines.next(), None);
    }
}
