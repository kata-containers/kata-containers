use crate::{asn1_string, TestValidCharset};
use crate::{Error, Result};
use alloc::string::String;

asn1_string!(GraphicString);

impl<'a> TestValidCharset for GraphicString<'a> {
    fn test_valid_charset(i: &[u8]) -> Result<()> {
        if !i.iter().all(u8::is_ascii) {
            return Err(Error::StringInvalidCharset);
        }
        Ok(())
    }
}
