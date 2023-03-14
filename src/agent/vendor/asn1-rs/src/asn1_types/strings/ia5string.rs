use crate::{asn1_string, TestValidCharset};
use crate::{Error, Result};
use alloc::string::String;

asn1_string!(Ia5String);

impl<'a> TestValidCharset for Ia5String<'a> {
    fn test_valid_charset(i: &[u8]) -> Result<()> {
        if !i.iter().all(u8::is_ascii) {
            return Err(Error::StringInvalidCharset);
        }
        Ok(())
    }
}
