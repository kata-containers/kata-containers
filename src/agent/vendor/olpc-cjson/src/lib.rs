// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `olpc-cjson` provides a [`serde_json::Formatter`] to serialize data as [canonical JSON], as
//! defined by OLPC and used in [TUF].
//!
//! [`serde_json::Formatter`]: ../serde_json/ser/trait.Formatter.html
//! [canonical JSON]: http://wiki.laptop.org/go/Canonical_JSON
//! [TUF]: https://theupdateframework.github.io/
//!
//! OLPC's canonical JSON specification is subtly different from other "canonical JSON"
//! specifications, and is also not a strict subset of JSON (specifically, ASCII control characters
//! 0x00&ndash;0x1f are printed literally, which is not valid JSON). Therefore, `serde_json` cannot
//! necessarily deserialize JSON produced by this formatter.
//!
//! This crate is not developed or endorsed by OLPC; use of the term is solely to distinguish this
//! specification of canonical JSON from [other specifications of canonical JSON][xkcd].
//!
//! [xkcd]: https://xkcd.com/927/
//!
//! ```rust
//! use olpc_cjson::CanonicalFormatter;
//! use serde::Serialize;
//! use serde_json::json;
//!
//! let value = json!({"b": 12, "a": "qwerty"});
//! let mut buf = Vec::new();
//! let mut ser = serde_json::Serializer::with_formatter(&mut buf, CanonicalFormatter::new());
//! value.serialize(&mut ser).unwrap();
//! assert_eq!(buf, br#"{"a":"qwerty","b":12}"#);
//! ```

#![deny(rust_2018_idioms)]
#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate)]

use serde::Serialize;
use serde_json::ser::{CharEscape, CompactFormatter, Formatter, Serializer};
use std::collections::BTreeMap;
use std::io::{Error, ErrorKind, Result, Write};
use unicode_normalization::UnicodeNormalization;

/// A [`Formatter`] that produces canonical JSON.
///
/// See the [crate-level documentation](../index.html) for more detail.
///
/// [`Formatter`]: ../serde_json/ser/trait.Formatter.html
#[derive(Debug, Default)]
pub struct CanonicalFormatter {
    object_stack: Vec<Object>,
}

/// Internal struct to keep track of an object in progress of being built.
///
/// As keys and values are received by `CanonicalFormatter`, they are written to `next_key` and
/// `next_value` by using the `CanonicalFormatter::writer` convenience method.
///
/// How this struct behaves when `Formatter` methods are called:
///
/// ```plain
/// [other methods]  // values written to the writer received by method
/// begin_object     // create this object
/// /-> begin_object_key    // object.key_done = false;
/// |   [other methods]     // values written to object.next_key, writer received by method ignored
/// |   end_object_key      // object.key_done = true;
/// |   begin_object_value  // [nothing]
/// |   [other methods]     // values written to object.next_value
/// |   end_object_value    // object.next_key and object.next_value are inserted into object.obj
/// \---- // jump back if more values are present
/// end_object       // write the object (sorted by its keys) to the writer received by the method
/// ```
#[derive(Debug, Default)]
struct Object {
    obj: BTreeMap<Vec<u8>, Vec<u8>>,
    next_key: Vec<u8>,
    next_value: Vec<u8>,
    key_done: bool,
}

impl CanonicalFormatter {
    /// Create a new `CanonicalFormatter` object.
    pub fn new() -> Self {
        Self::default()
    }

    /// Convenience method to return the appropriate writer given the current context.
    ///
    /// If we are currently writing an object (that is, if `!self.object_stack.is_empty()`), we
    /// need to write the value to either the next key or next value depending on that state
    /// machine. See the docstrings for `Object` for more detail.
    ///
    /// If we are not currently writing an object, pass through `writer`.
    fn writer<'a, W: Write + ?Sized>(&'a mut self, writer: &'a mut W) -> Box<dyn Write + 'a> {
        self.object_stack
            .last_mut()
            .map_or(Box::new(writer), |object| {
                if object.key_done {
                    Box::new(&mut object.next_value)
                } else {
                    Box::new(&mut object.next_key)
                }
            })
    }

    /// Returns a mutable reference to the top of the object stack.
    fn obj_mut(&mut self) -> Result<&mut Object> {
        self.object_stack.last_mut().ok_or_else(|| {
            Error::new(
                ErrorKind::Other,
                "serde_json called an object method without calling begin_object first",
            )
        })
    }
}

/// Wraps `serde_json::CompactFormatter` to use the appropriate writer (see
/// `CanonicalFormatter::writer`).
macro_rules! wrapper {
    ($f:ident) => {
        fn $f<W: Write + ?Sized>(&mut self, writer: &mut W) -> Result<()> {
            CompactFormatter.$f(&mut self.writer(writer))
        }
    };

    ($f:ident, $t:ty) => {
        fn $f<W: Write + ?Sized>(&mut self, writer: &mut W, arg: $t) -> Result<()> {
            CompactFormatter.$f(&mut self.writer(writer), arg)
        }
    };
}

/// This is used in three places. Write it once.
macro_rules! float_err {
    () => {
        Err(Error::new(
            ErrorKind::InvalidInput,
            "floating point numbers are not allowed in canonical JSON",
        ))
    };
}

impl Formatter for CanonicalFormatter {
    wrapper!(write_null);
    wrapper!(write_bool, bool);
    wrapper!(write_i8, i8);
    wrapper!(write_i16, i16);
    wrapper!(write_i32, i32);
    wrapper!(write_i64, i64);
    wrapper!(write_u8, u8);
    wrapper!(write_u16, u16);
    wrapper!(write_u32, u32);
    wrapper!(write_u64, u64);

    fn write_f32<W: Write + ?Sized>(&mut self, _writer: &mut W, _value: f32) -> Result<()> {
        float_err!()
    }

    fn write_f64<W: Write + ?Sized>(&mut self, _writer: &mut W, _value: f64) -> Result<()> {
        float_err!()
    }

    // By default this is only used for u128/i128. If serde_json's `arbitrary_precision` feature is
    // enabled, all numbers are internally stored as strings, and this method is always used (even
    // for floating point values).
    fn write_number_str<W: Write + ?Sized>(&mut self, writer: &mut W, value: &str) -> Result<()> {
        if value.chars().any(|c| c == '.' || c == 'e' || c == 'E') {
            float_err!()
        } else {
            CompactFormatter.write_number_str(&mut self.writer(writer), value)
        }
    }

    wrapper!(begin_string);
    wrapper!(end_string);

    // Strings are normalized as Normalization Form C (NFC). `str::nfc` is provided by the
    // `UnicodeNormalization` trait and returns an iterator of `char`s.
    fn write_string_fragment<W: Write + ?Sized>(
        &mut self,
        writer: &mut W,
        fragment: &str,
    ) -> Result<()> {
        fragment.nfc().try_for_each(|ch| {
            self.writer(writer)
                .write_all(ch.encode_utf8(&mut [0; 4]).as_bytes())
        })
    }

    // Only quotes and backslashes are escaped in canonical JSON.
    fn write_char_escape<W: Write + ?Sized>(
        &mut self,
        writer: &mut W,
        char_escape: CharEscape,
    ) -> Result<()> {
        match char_escape {
            CharEscape::Quote | CharEscape::ReverseSolidus => {
                self.writer(writer).write_all(b"\\")?;
            }
            _ => {}
        }
        self.writer(writer).write_all(&[match char_escape {
            CharEscape::Quote => b'\"',
            CharEscape::ReverseSolidus => b'\\',
            CharEscape::Solidus => b'/',
            CharEscape::Backspace => b'\x08',
            CharEscape::FormFeed => b'\x0c',
            CharEscape::LineFeed => b'\n',
            CharEscape::CarriageReturn => b'\r',
            CharEscape::Tab => b'\t',
            CharEscape::AsciiControl(byte) => byte,
        }])
    }

    wrapper!(begin_array);
    wrapper!(end_array);
    wrapper!(begin_array_value, bool); // hack: this passes through the `first` argument
    wrapper!(end_array_value);

    // Here are the object methods. Because keys must be sorted, we serialize the object's keys and
    // values in memory as a `BTreeMap`, then write it all out when `end_object_value` is called.

    fn begin_object<W: Write + ?Sized>(&mut self, writer: &mut W) -> Result<()> {
        CompactFormatter.begin_object(&mut self.writer(writer))?;
        self.object_stack.push(Object::default());
        Ok(())
    }

    fn end_object<W: Write + ?Sized>(&mut self, writer: &mut W) -> Result<()> {
        let object = self.object_stack.pop().ok_or_else(|| {
            Error::new(
                ErrorKind::Other,
                "serde_json called Formatter::end_object object method
                 without calling begin_object first",
            )
        })?;
        let mut writer = self.writer(writer);
        let mut first = true;

        for (key, value) in object.obj {
            CompactFormatter.begin_object_key(&mut writer, first)?;
            writer.write_all(&key)?;
            CompactFormatter.end_object_key(&mut writer)?;

            CompactFormatter.begin_object_value(&mut writer)?;
            writer.write_all(&value)?;
            CompactFormatter.end_object_value(&mut writer)?;

            first = false;
        }

        CompactFormatter.end_object(&mut writer)
    }

    fn begin_object_key<W: Write + ?Sized>(&mut self, _writer: &mut W, _first: bool) -> Result<()> {
        let mut object = self.obj_mut()?;
        object.key_done = false;
        Ok(())
    }

    fn end_object_key<W: Write + ?Sized>(&mut self, _writer: &mut W) -> Result<()> {
        let mut object = self.obj_mut()?;
        object.key_done = true;
        Ok(())
    }

    fn begin_object_value<W: Write + ?Sized>(&mut self, _writer: &mut W) -> Result<()> {
        Ok(())
    }

    fn end_object_value<W: Write + ?Sized>(&mut self, _writer: &mut W) -> Result<()> {
        let object = self.obj_mut()?;
        let key = std::mem::take(&mut object.next_key);
        let value = std::mem::take(&mut object.next_value);
        object.obj.insert(key, value);
        Ok(())
    }

    // This is for serde_json's `raw_value` feature, which provides a RawValue type that is passed
    // through as-is. That's not good enough for canonical JSON, so we parse it and immediately
    // write it back out... as canonical JSON.
    fn write_raw_fragment<W: Write + ?Sized>(
        &mut self,
        writer: &mut W,
        fragment: &str,
    ) -> Result<()> {
        let mut ser = Serializer::with_formatter(self.writer(writer), Self::new());
        serde_json::from_str::<serde_json::Value>(fragment)?.serialize(&mut ser)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::CanonicalFormatter;
    use serde::Serialize;
    use serde_json::Serializer;
    use std::io::Result;

    /// Small wrapper around the `serde_json` json! macro to encode the value as canonical JSON.
    macro_rules! encode {
        ($($tt:tt)+) => {
            (|v: serde_json::Value| -> Result<Vec<u8>> {
                let mut buf = Vec::new();
                let mut ser = Serializer::with_formatter(&mut buf, CanonicalFormatter::new());
                v.serialize(&mut ser)?;
                Ok(buf)
            })(serde_json::json!($($tt)+))
        };
    }

    /// These smoke tests come from securesystemslib, the library used by the TUF reference
    /// implementation.
    ///
    /// `<https://github.com/secure-systems-lab/securesystemslib/blob/f466266014aff529510216b8c2f8c8f39de279ec/tests/test_formats.py#L354-L389>`
    #[test]
    fn securesystemslib_asserts() -> Result<()> {
        assert_eq!(encode!([1, 2, 3])?, b"[1,2,3]");
        assert_eq!(encode!([1, 2, 3])?, b"[1,2,3]");
        assert_eq!(encode!([])?, b"[]");
        assert_eq!(encode!({})?, b"{}");
        assert_eq!(encode!({"A": [99]})?, br#"{"A":[99]}"#);
        assert_eq!(encode!({"A": true})?, br#"{"A":true}"#);
        assert_eq!(encode!({"B": false})?, br#"{"B":false}"#);
        assert_eq!(encode!({"x": 3, "y": 2})?, br#"{"x":3,"y":2}"#);
        assert_eq!(encode!({"x": 3, "y": null})?, br#"{"x":3,"y":null}"#);

        // Test conditions for invalid arguments.
        assert!(encode!(8.0).is_err());
        assert!(encode!({"x": 8.0}).is_err());

        Ok(())
    }

    /// Canonical JSON prints literal ASCII control characters instead of escaping them. Check
    /// ASCII 0x00 - 0x1f, plus backslash and double quote (the only escaped characters).
    ///
    /// The accepted strings were validated with securesystemslib, commit
    /// f466266014aff529510216b8c2f8c8f39de279ec.
    ///
    /// ```python
    /// import securesystemslib.formats
    /// encode = securesystemslib.formats.encode_canonical
    /// for c in range(0x20):
    ///     print(repr(encode(chr(c))))
    /// print(repr(encode('\\')))
    /// print(repr(encode('"')))
    /// ```
    ///
    /// This can be a little difficult to wrap a mental string parser around. But you can verify
    /// that all the control characters result in a 3-byte JSON string:
    ///
    /// ```python
    /// >>> all(map(lambda c: len(encode(chr(c))) == 3, range(0x20)))
    /// True
    /// ```
    #[test]
    fn ascii_control_characters() -> Result<()> {
        assert_eq!(encode!("\x00")?, b"\"\x00\"");
        assert_eq!(encode!("\x01")?, b"\"\x01\"");
        assert_eq!(encode!("\x02")?, b"\"\x02\"");
        assert_eq!(encode!("\x03")?, b"\"\x03\"");
        assert_eq!(encode!("\x04")?, b"\"\x04\"");
        assert_eq!(encode!("\x05")?, b"\"\x05\"");
        assert_eq!(encode!("\x06")?, b"\"\x06\"");
        assert_eq!(encode!("\x07")?, b"\"\x07\"");
        assert_eq!(encode!("\x08")?, b"\"\x08\"");
        assert_eq!(encode!("\x09")?, b"\"\x09\"");
        assert_eq!(encode!("\x0a")?, b"\"\x0a\"");
        assert_eq!(encode!("\x0b")?, b"\"\x0b\"");
        assert_eq!(encode!("\x0c")?, b"\"\x0c\"");
        assert_eq!(encode!("\x0d")?, b"\"\x0d\"");
        assert_eq!(encode!("\x0e")?, b"\"\x0e\"");
        assert_eq!(encode!("\x0f")?, b"\"\x0f\"");
        assert_eq!(encode!("\x10")?, b"\"\x10\"");
        assert_eq!(encode!("\x11")?, b"\"\x11\"");
        assert_eq!(encode!("\x12")?, b"\"\x12\"");
        assert_eq!(encode!("\x13")?, b"\"\x13\"");
        assert_eq!(encode!("\x14")?, b"\"\x14\"");
        assert_eq!(encode!("\x15")?, b"\"\x15\"");
        assert_eq!(encode!("\x16")?, b"\"\x16\"");
        assert_eq!(encode!("\x17")?, b"\"\x17\"");
        assert_eq!(encode!("\x18")?, b"\"\x18\"");
        assert_eq!(encode!("\x19")?, b"\"\x19\"");
        assert_eq!(encode!("\x1a")?, b"\"\x1a\"");
        assert_eq!(encode!("\x1b")?, b"\"\x1b\"");
        assert_eq!(encode!("\x1c")?, b"\"\x1c\"");
        assert_eq!(encode!("\x1d")?, b"\"\x1d\"");
        assert_eq!(encode!("\x1e")?, b"\"\x1e\"");
        assert_eq!(encode!("\x1f")?, b"\"\x1f\"");

        // Try to trigger a panic in our unsafe blocks (from_utf8_unchecked)...
        assert_eq!(encode!({"\t": "\n"})?, b"{\"\t\":\"\n\"}");

        assert_eq!(encode!("\\")?, b"\"\\\\\"");
        assert_eq!(encode!("\"")?, b"\"\\\"\"");

        Ok(())
    }

    /// A more involved test than any of the above for olpc-cjson's core competency: ordering
    /// things.
    #[test]
    fn ordered_nested_object() -> Result<()> {
        assert_eq!(
            encode!({
                "nested": {
                    "bad": true,
                    "good": false
                },
                "b": 2,
                "a": 1,
                "c": {
                    "h": {
                        "h": -5,
                        "i": 3
                    },
                    "a": null,
                    "x": {}
                }
            })?,
            br#"{"a":1,"b":2,"c":{"a":null,"h":{"h":-5,"i":3},"x":{}},"nested":{"bad":true,"good":false}}"#.to_vec(),
        );

        Ok(())
    }

    /// This test asserts that the cannonical representation of some real-world data always comes
    /// out the same.
    #[allow(clippy::unreadable_literal)]
    #[test]
    fn actual_tuf_signed() {
        let encode_result = encode!(
        {
          "signed": {
            "_type": "timestamp",
            "spec_version": "1.0.0",
            "version": 1604605512,
            "expires": "2020-11-12T19:45:12.613154979Z",
            "meta": {
              "snapshot.json": {
                "length": 1278,
                "hashes": {
                  "sha256": "56c4ecc3b331f6154d9a5005f6e2978e4198cc8c3b79746c25a592043a2d83d4"
                },
                "version": 1604605512
              }
            }
          }
        }
        );

        let encoded = encode_result.unwrap();
        let expected: Vec<u8> = vec![
            123, 34, 115, 105, 103, 110, 101, 100, 34, 58, 123, 34, 95, 116, 121, 112, 101, 34, 58,
            34, 116, 105, 109, 101, 115, 116, 97, 109, 112, 34, 44, 34, 101, 120, 112, 105, 114,
            101, 115, 34, 58, 34, 50, 48, 50, 48, 45, 49, 49, 45, 49, 50, 84, 49, 57, 58, 52, 53,
            58, 49, 50, 46, 54, 49, 51, 49, 53, 52, 57, 55, 57, 90, 34, 44, 34, 109, 101, 116, 97,
            34, 58, 123, 34, 115, 110, 97, 112, 115, 104, 111, 116, 46, 106, 115, 111, 110, 34, 58,
            123, 34, 104, 97, 115, 104, 101, 115, 34, 58, 123, 34, 115, 104, 97, 50, 53, 54, 34,
            58, 34, 53, 54, 99, 52, 101, 99, 99, 51, 98, 51, 51, 49, 102, 54, 49, 53, 52, 100, 57,
            97, 53, 48, 48, 53, 102, 54, 101, 50, 57, 55, 56, 101, 52, 49, 57, 56, 99, 99, 56, 99,
            51, 98, 55, 57, 55, 52, 54, 99, 50, 53, 97, 53, 57, 50, 48, 52, 51, 97, 50, 100, 56,
            51, 100, 52, 34, 125, 44, 34, 108, 101, 110, 103, 116, 104, 34, 58, 49, 50, 55, 56, 44,
            34, 118, 101, 114, 115, 105, 111, 110, 34, 58, 49, 54, 48, 52, 54, 48, 53, 53, 49, 50,
            125, 125, 44, 34, 115, 112, 101, 99, 95, 118, 101, 114, 115, 105, 111, 110, 34, 58, 34,
            49, 46, 48, 46, 48, 34, 44, 34, 118, 101, 114, 115, 105, 111, 110, 34, 58, 49, 54, 48,
            52, 54, 48, 53, 53, 49, 50, 125, 125,
        ];
        assert_eq!(expected, encoded);
    }
}
