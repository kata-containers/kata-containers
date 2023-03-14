use crate::asn1_string;
use crate::Result;
use crate::TestValidCharset;
use alloc::string::String;

asn1_string!(Utf8String);

impl<'a> TestValidCharset for Utf8String<'a> {
    fn test_valid_charset(i: &[u8]) -> Result<()> {
        let _ = core::str::from_utf8(i)?;
        Ok(())
    }
}
