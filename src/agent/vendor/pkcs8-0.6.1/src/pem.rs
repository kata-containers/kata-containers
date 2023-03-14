//! PEM encoding support (RFC 7468)

use crate::{Error, Result};
use alloc::{borrow::ToOwned, string::String, vec::Vec};
use base64ct::{Base64, Encoding};
use core::str;
use zeroize::Zeroizing;

/// Encapsulation boundaries
pub(crate) struct Boundary {
    /// Pre-encapsulation boundary
    pre: &'static str,

    /// Post-encapsulation boundary
    post: &'static str,
}

/// Encrypted private key encapsulation boundary
#[cfg(feature = "pkcs5")]
pub(crate) const ENCRYPTED_PRIVATE_KEY_BOUNDARY: Boundary = Boundary {
    pre: "-----BEGIN ENCRYPTED PRIVATE KEY-----\n",
    post: "\n-----END ENCRYPTED PRIVATE KEY-----",
};

/// Private key encapsulation boundary
pub(crate) const PRIVATE_KEY_BOUNDARY: Boundary = Boundary {
    pre: "-----BEGIN PRIVATE KEY-----\n",
    post: "\n-----END PRIVATE KEY-----",
};

/// Public key encapsulation boundary
pub(crate) const PUBLIC_KEY_BOUNDARY: Boundary = Boundary {
    pre: "-----BEGIN PUBLIC KEY-----\n",
    post: "\n-----END PUBLIC KEY-----",
};

/// Size of Base64 "chunks" i.e. how many Base64 encoded characters to include
/// on a single line.
const CHUNK_SIZE: usize = 64;

/// Parse "PEM encoding" as described in RFC 7468:
/// <https://tools.ietf.org/html/rfc7468>
///
/// Note that this decoder supports only a subset of the original
/// "Privacy Enhanced Mail" encoding as this parser specifically
/// implements a dialect intended for textual encodings of PKIX,
/// PKCS, and CMS structures.
// TODO(tarcieri): better harden for fully constant-time operation
pub(crate) fn decode(s: &str, boundary: Boundary) -> Result<Zeroizing<Vec<u8>>> {
    let s = s.trim_end();

    // TODO(tarcieri): handle missing newlines
    let s = s.strip_prefix(boundary.pre).ok_or(Error::Decode)?;
    let s = s.strip_suffix(boundary.post).ok_or(Error::Decode)?;

    let mut s = Zeroizing::new(s.to_owned());

    // TODO(tarcieri): stricter constant-time whitespace trimming
    s.retain(|c| !c.is_whitespace());

    Base64::decode_vec(&*s)
        .map(Zeroizing::new)
        .map_err(|_| Error::Decode)
}

/// Serialize "PEM encoding" as described in RFC 7468:
/// <https://tools.ietf.org/html/rfc7468>
pub(crate) fn encode(data: &[u8], boundary: Boundary) -> String {
    let mut output = String::new();
    output.push_str(boundary.pre);

    let b64 = Zeroizing::new(Base64::encode_string(data));
    let chunks = b64.as_bytes().chunks(CHUNK_SIZE);
    let nchunks = chunks.len();

    for (i, chunk) in chunks.enumerate() {
        let line = str::from_utf8(chunk).expect("malformed Base64");
        output.push_str(line);

        if i < nchunks.checked_sub(1).expect("unexpected Base64 chunks") {
            // The final newline is expected to be part of `boundary.post`
            output.push('\n');
        } else if line.len() % 4 != 0 {
            // Add '=' padding (the `b64ct` crate doesn't handle this)
            for _ in 0..(4 - (line.len() % 4)) {
                output.push('=');
            }
        }
    }

    output.push_str(boundary.post);
    output
}
