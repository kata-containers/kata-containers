// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::error;
use ring::digest::{Context, SHA256};
use std::io::{self, Read};
use url::Url;

pub(crate) struct DigestAdapter {
    url: Url,
    reader: Box<dyn Read + Send>,
    hash: Vec<u8>,
    digest: Option<Context>,
}

impl DigestAdapter {
    pub(crate) fn sha256(reader: Box<dyn Read + Send>, hash: &[u8], url: Url) -> Self {
        Self {
            url,
            reader,
            hash: hash.to_owned(),
            digest: Some(Context::new(&SHA256)),
        }
    }
}

impl Read for DigestAdapter {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        assert!(
            self.digest.is_some(),
            "DigestAdapter::read called after end of file"
        );

        let size = self.reader.read(buf)?;
        if size == 0 {
            let result = std::mem::replace(&mut self.digest, None).unwrap().finish();
            if result.as_ref() != self.hash.as_slice() {
                error::HashMismatchSnafu {
                    context: self.url.to_string(),
                    calculated: hex::encode(result),
                    expected: hex::encode(&self.hash),
                }
                .fail()?;
            }
            Ok(size)
        } else if let Some(digest) = &mut self.digest {
            digest.update(&buf[..size]);
            Ok(size)
        } else {
            unreachable!();
        }
    }
}

pub(crate) struct MaxSizeAdapter {
    reader: Box<dyn Read + Send>,
    /// How the `max_size` was specified. For example the max size of `root.json` is specified by
    /// the `max_root_size` argument in `Settings`. `specifier` is used to construct an error
    /// message when the `MaxSizeAdapter` detects that too many bytes have been read.
    specifier: &'static str,
    max_size: u64,
    counter: u64,
}

impl MaxSizeAdapter {
    pub(crate) fn new(
        reader: Box<dyn Read + Send>,
        specifier: &'static str,
        max_size: u64,
    ) -> Self {
        Self {
            reader,
            specifier,
            max_size,
            counter: 0,
        }
    }
}

impl Read for MaxSizeAdapter {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let size = self.reader.read(buf)?;
        self.counter += size as u64;
        if self.counter > self.max_size {
            error::MaxSizeExceededSnafu {
                max_size: self.max_size,
                specifier: self.specifier,
            }
            .fail()?;
        }
        Ok(size)
    }
}

#[cfg(test)]
mod tests {
    use crate::io::{DigestAdapter, MaxSizeAdapter};
    use hex_literal::hex;
    use std::io::{Cursor, Read};
    use url::Url;

    #[test]
    fn test_max_size_adapter() {
        let mut reader = MaxSizeAdapter::new(Box::new(Cursor::new(b"hello".to_vec())), "test", 5);
        let mut buf = Vec::new();
        assert!(reader.read_to_end(&mut buf).is_ok());
        assert_eq!(buf, b"hello");

        let mut reader = MaxSizeAdapter::new(Box::new(Cursor::new(b"hello".to_vec())), "test", 4);
        let mut buf = Vec::new();
        assert!(reader.read_to_end(&mut buf).is_err());
    }

    #[test]
    fn test_digest_adapter() {
        let mut reader = DigestAdapter::sha256(
            Box::new(Cursor::new(b"hello".to_vec())),
            &hex!("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"),
            Url::parse("file:///").unwrap(),
        );
        let mut buf = Vec::new();
        assert!(reader.read_to_end(&mut buf).is_ok());
        assert_eq!(buf, b"hello");

        let mut reader = DigestAdapter::sha256(
            Box::new(Cursor::new(b"hello".to_vec())),
            &hex!("0ebdc3317b75839f643387d783535adc360ca01f33c75f7c1e7373adcd675c0b"),
            Url::parse("file:///").unwrap(),
        );
        let mut buf = Vec::new();
        assert!(reader.read_to_end(&mut buf).is_err());
    }
}
