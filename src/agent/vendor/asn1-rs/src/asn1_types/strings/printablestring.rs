use crate::{asn1_string, TestValidCharset};
use crate::{Error, Result};
use alloc::string::String;

asn1_string!(PrintableString);

impl<'a> TestValidCharset for PrintableString<'a> {
    fn test_valid_charset(i: &[u8]) -> Result<()> {
        // Argument must be a reference, because of the .iter().all(F) call below
        #[allow(clippy::trivially_copy_pass_by_ref)]
        fn is_printable(b: &u8) -> bool {
            matches!(*b,
            b'a'..=b'z'
            | b'A'..=b'Z'
            | b'0'..=b'9'
            | b' '
            | b'\''
            | b'('
            | b')'
            | b'+'
            | b','
            | b'-'
            | b'.'
            | b'/'
            | b':'
            | b'='
            | b'?')
        }

        if !i.iter().all(is_printable) {
            return Err(Error::StringInvalidCharset);
        }
        Ok(())
    }
}
