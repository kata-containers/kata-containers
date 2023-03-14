//! OID string parser with `const` support.

use crate::{encoder::Encoder, Arc, ObjectIdentifier};

/// Const-friendly OID string parser.
///
/// Parses an OID from the dotted string representation.
pub(crate) struct Parser {
    /// Current arc in progress
    current_arc: Arc,

    /// BER/DER encoder
    encoder: Encoder,
}

impl Parser {
    /// Parse an OID from a dot-delimited string e.g. `1.2.840.113549.1.1.1`
    pub(crate) const fn parse(s: &str) -> Self {
        let bytes = s.as_bytes();
        const_assert!(!bytes.is_empty(), "OID string is empty");
        const_assert!(
            matches!(bytes[0], b'0'..=b'9'),
            "OID must start with a digit"
        );

        let current_arc = 0;
        let encoder = Encoder::new();
        Self {
            current_arc,
            encoder,
        }
        .parse_bytes(bytes)
    }

    /// Finish parsing, returning the result
    pub(crate) const fn finish(self) -> ObjectIdentifier {
        self.encoder.finish()
    }

    /// Parse the remaining bytes
    const fn parse_bytes(mut self, bytes: &[u8]) -> Self {
        match bytes {
            [] => {
                self.encoder = self.encoder.encode(self.current_arc);
                self
            }
            [byte @ b'0'..=b'9', remaining @ ..] => {
                let digit = byte.saturating_sub(b'0');
                self.current_arc = self.current_arc * 10 + digit as Arc;
                self.parse_bytes(remaining)
            }
            [b'.', remaining @ ..] => {
                const_assert!(!remaining.is_empty(), "invalid trailing '.' in OID");
                self.encoder = self.encoder.encode(self.current_arc);
                self.current_arc = 0;
                self.parse_bytes(remaining)
            }
            [byte, ..] => {
                const_assert!(
                    matches!(byte, b'0'..=b'9' | b'.'),
                    "invalid character in OID"
                );

                // Unreachable (checked by above `const_assert!`)
                // Needed for match exhaustiveness and matching types
                self
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Parser;

    #[test]
    fn parse() {
        let oid = Parser::parse("1.23.456").finish();
        assert_eq!(oid, "1.23.456".parse().unwrap());
    }

    #[test]
    #[should_panic]
    fn reject_empty_string() {
        Parser::parse("");
    }

    #[test]
    #[should_panic]
    fn reject_non_digits() {
        Parser::parse("X");
    }

    #[test]
    #[should_panic]
    fn reject_trailing_dot() {
        Parser::parse("1.23.");
    }
}
