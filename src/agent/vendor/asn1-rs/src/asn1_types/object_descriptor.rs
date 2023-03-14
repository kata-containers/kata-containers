use crate::{asn1_string, TestValidCharset};
use crate::{Error, Result};
use alloc::string::String;

// X.680 section 44.3
// ObjectDescriptor ::= [UNIVERSAL 7] IMPLICIT GraphicString

asn1_string!(ObjectDescriptor);

impl<'a> TestValidCharset for ObjectDescriptor<'a> {
    fn test_valid_charset(i: &[u8]) -> Result<()> {
        if !i.iter().all(u8::is_ascii) {
            return Err(Error::StringInvalidCharset);
        }
        Ok(())
    }
}
