//! Utilities for formatting, printing, and user communication.

use crate::Error;
use crate::Result;

/// Converts buffers to and from hexadecimal numbers.
pub mod hex {
    use std::io;

    use crate::Result;

    /// Encodes the given buffer as hexadecimal number.
    pub fn encode<B: AsRef<[u8]>>(buffer: B) -> String {
        super::to_hex(buffer.as_ref(), false)
    }

    /// Encodes the given buffer as hexadecimal number with spaces.
    pub fn encode_pretty<B: AsRef<[u8]>>(buffer: B) -> String {
        super::to_hex(buffer.as_ref(), true)
    }

    /// Decodes the given hexadecimal number.
    pub fn decode<H: AsRef<str>>(hex: H) -> Result<Vec<u8>> {
        super::from_hex(hex.as_ref(), false)
    }

    /// Decodes the given hexadecimal number, ignoring whitespace.
    pub fn decode_pretty<H: AsRef<str>>(hex: H) -> Result<Vec<u8>> {
        super::from_hex(hex.as_ref(), true)
    }

    /// Dumps binary data, like `hd(1)`.
    pub fn dump<W: io::Write, B: AsRef<[u8]>>(sink: W, data: B)
                                              -> io::Result<()> {
        Dumper::new(sink, "").write_ascii(data)
    }

    /// Writes annotated hex dumps, like hd(1).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp::fmt::hex;
    ///
    /// let mut dumper = hex::Dumper::new(Vec::new(), "");
    /// dumper.write(&[0x89, 0x01, 0x33], "frame")?;
    /// dumper.write(&[0x04], "version")?;
    /// dumper.write(&[0x00], "type")?;
    ///
    /// let buf = dumper.into_inner();
    /// assert_eq!(
    ///     ::std::str::from_utf8(&buf[..])?,
    ///     "00000000  89 01 33                                           frame\n\
    ///      00000003           04                                        version\n\
    ///      00000004              00                                     type\n\
    ///      ");
    /// # Ok(()) }
    /// ```
    pub struct Dumper<W: io::Write> {
        inner: W,
        indent: String,
        offset: usize,
    }

    assert_send_and_sync!(Dumper<W> where W: io::Write);

    impl<W: io::Write> Dumper<W> {
        /// Creates a new dumper.
        ///
        /// The dump is written to `inner`.  Every line is indented with
        /// `indent`.
        pub fn new<I: AsRef<str>>(inner: W, indent: I) -> Self {
            Dumper {
                inner,
                indent: indent.as_ref().into(),
                offset: 0,
            }
        }

        /// Returns the inner writer.
        pub fn into_inner(self) -> W {
            self.inner
        }

        /// Writes a chunk of data.
        ///
        /// The `msg` is printed at the end of the first line.
        pub fn write<B, M>(&mut self, buf: B, msg: M) -> io::Result<()>
            where B: AsRef<[u8]>,
                  M: AsRef<str>,
        {
            let mut first = true;
            self.write_labeled(buf.as_ref(), move |_, _| {
                if first {
                    first = false;
                    Some(msg.as_ref().into())
                } else {
                    None
                }
            })
        }

        /// Writes a chunk of data with ASCII-representation.
        ///
        /// This produces output similar to `hd(1)`.
        pub fn write_ascii<B>(&mut self, buf: B) -> io::Result<()>
            where B: AsRef<[u8]>,
        {
            self.write_labeled(buf, |offset, data| {
                let mut l = String::new();
                for _ in 0..offset {
                    l.push(' ');
                }
                for &c in data {
                    l.push(if c < 32 {
                        '.'
                    } else if c < 128 {
                        c.into()
                    } else {
                        '.'
                    })
                }
                Some(l)
            })
        }

        /// Writes a chunk of data.
        ///
        /// For each line, the given function is called to compute a
        /// label that printed at the end of the first line.  The
        /// functions first argument is the offset in the current line
        /// (0..16), the second the slice of the displayed data.
        pub fn write_labeled<B, L>(&mut self, buf: B, mut labeler: L)
                                -> io::Result<()>
            where B: AsRef<[u8]>,
                  L: FnMut(usize, &[u8]) -> Option<String>,
        {
            let buf = buf.as_ref();
            let mut first_label_offset = self.offset % 16;

            write!(self.inner, "{}{:08x} ", self.indent, self.offset)?;
            for i in 0 .. self.offset % 16 {
                if i != 7 {
                    write!(self.inner, "   ")?;
                } else {
                    write!(self.inner, "    ")?;
                }
            }

            let mut offset_printed = true;
            let mut data_start = 0;
            for (i, c) in buf.iter().enumerate() {
                if ! offset_printed {
                    write!(self.inner,
                           "\n{}{:08x} ", self.indent, self.offset)?;
                    offset_printed = true;
                }

                write!(self.inner, " {:02x}", c)?;
                self.offset += 1;
                match self.offset % 16 {
                    0 => {
                        if let Some(msg) = labeler(
                            first_label_offset, &buf[data_start..i + 1])
                        {
                            write!(self.inner, "   {}", msg)?;
                            // Only the first label is offset.
                            first_label_offset = 0;
                        }
                        data_start = i + 1;
                        offset_printed = false;
                    },
                    8 => write!(self.inner, " ")?,
                    _ => (),
                }
            }

            if let Some(msg) = labeler(
                first_label_offset, &buf[data_start..])
            {
                for i in self.offset % 16 .. 16 {
                    if i != 7 {
                        write!(self.inner, "   ")?;
                    } else {
                        write!(self.inner, "    ")?;
                    }
                }

                write!(self.inner, "   {}", msg)?;
            }
            writeln!(self.inner)?;
            Ok(())
        }
    }
}

/// A helpful debugging function.
#[allow(dead_code)]
pub(crate) fn to_hex(s: &[u8], pretty: bool) -> String {
    use std::fmt::Write;

    let mut result = String::new();
    for (i, b) in s.iter().enumerate() {
        // Add spaces every four digits to make the output more
        // readable.
        if pretty && i > 0 && i % 2 == 0 {
            write!(&mut result, " ").unwrap();
        }
        write!(&mut result, "{:02X}", b).unwrap();
    }
    result
}

/// A helpful function for converting a hexadecimal string to binary.
/// This function skips whitespace if `pretty` is set.
pub(crate) fn from_hex(hex: &str, pretty: bool) -> Result<Vec<u8>> {
    const BAD: u8 = 255u8;
    const X: u8 = b'x';

    let mut nibbles = hex.chars().filter_map(|x| {
        match x {
            '0' => Some(0u8),
            '1' => Some(1u8),
            '2' => Some(2u8),
            '3' => Some(3u8),
            '4' => Some(4u8),
            '5' => Some(5u8),
            '6' => Some(6u8),
            '7' => Some(7u8),
            '8' => Some(8u8),
            '9' => Some(9u8),
            'a' | 'A' => Some(10u8),
            'b' | 'B' => Some(11u8),
            'c' | 'C' => Some(12u8),
            'd' | 'D' => Some(13u8),
            'e' | 'E' => Some(14u8),
            'f' | 'F' => Some(15u8),
            'x' | 'X' if pretty => Some(X),
            _ if pretty && x.is_whitespace() => None,
            _ => Some(BAD),
        }
    }).collect::<Vec<u8>>();

    if pretty && nibbles.len() >= 2 && nibbles[0] == 0 && nibbles[1] == X {
        // Drop '0x' prefix.
        nibbles.remove(0);
        nibbles.remove(0);
    }

    if nibbles.iter().any(|&b| b == BAD || b == X) {
        // Not a hex character.
        return
            Err(Error::InvalidArgument("Invalid characters".into()).into());
    }

    // We need an even number of nibbles.
    if nibbles.len() % 2 != 0 {
        return
            Err(Error::InvalidArgument("Odd number of nibbles".into()).into());
    }

    let bytes = nibbles.chunks(2).map(|nibbles| {
        (nibbles[0] << 4) | nibbles[1]
    }).collect::<Vec<u8>>();

    Ok(bytes)
}

/// Formats the given time using ISO 8601.
///
/// This is a no-dependency, best-effort mechanism.  If the given time
/// is not representable using unsigned UNIX time, we return the debug
/// formatting.
pub(crate) fn time(t: &std::time::SystemTime) -> String {
    // Actually use a chrono dependency for WASM since there's no strftime
    // (except for WASI).
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))] {
        chrono::DateTime::<chrono::Utc>::from(t.clone())
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string()
    }
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))] {
        extern "C" {
            fn strftime(
                s: *mut libc::c_char,
                max: libc::size_t,
                format: *const libc::c_char,
                tm: *const libc::tm,
            ) -> usize;
        }

        let t = match t.duration_since(std::time::UNIX_EPOCH) {
            Ok(t) => t.as_secs() as libc::time_t,
            Err(_) => return format!("{:?}", t),
        };
        let fmt = b"%Y-%m-%dT%H:%M:%SZ\x00";
        assert_eq!(b"2020-03-26T10:08:10Z\x00".len(), 21);
        let mut s = [0u8; 21];

        unsafe {
            let mut tm: libc::tm = std::mem::zeroed();

            #[cfg(unix)]
            libc::gmtime_r(&t, &mut tm);
            #[cfg(windows)]
            libc::gmtime_s(&mut tm, &t);

            strftime(s.as_mut_ptr() as *mut libc::c_char,
                     s.len(),
                     fmt.as_ptr() as *const libc::c_char,
                     &tm);
        }

        std::ffi::CStr::from_bytes_with_nul(&s)
            .expect("strftime nul terminates string")
            .to_string_lossy().into()
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn from_hex() {
        use super::from_hex as fh;
        assert_eq!(fh("", false).ok(), Some(vec![]));
        assert_eq!(fh("0", false).ok(), None);
        assert_eq!(fh("00", false).ok(), Some(vec![0x00]));
        assert_eq!(fh("09", false).ok(), Some(vec![0x09]));
        assert_eq!(fh("0f", false).ok(), Some(vec![0x0f]));
        assert_eq!(fh("99", false).ok(), Some(vec![0x99]));
        assert_eq!(fh("ff", false).ok(), Some(vec![0xff]));
        assert_eq!(fh("000", false).ok(), None);
        assert_eq!(fh("0000", false).ok(), Some(vec![0x00, 0x00]));
        assert_eq!(fh("0009", false).ok(), Some(vec![0x00, 0x09]));
        assert_eq!(fh("000f", false).ok(), Some(vec![0x00, 0x0f]));
        assert_eq!(fh("0099", false).ok(), Some(vec![0x00, 0x99]));
        assert_eq!(fh("00ff", false).ok(), Some(vec![0x00, 0xff]));
        assert_eq!(fh("\t\n\x0c\r ", false).ok(), None);
        assert_eq!(fh("a", false).ok(), None);
        assert_eq!(fh("0x", false).ok(), None);
        assert_eq!(fh("0x0", false).ok(), None);
        assert_eq!(fh("0x00", false).ok(), None);
    }

    #[test]
    fn from_pretty_hex() {
        use super::from_hex as fh;
        assert_eq!(fh(" ", true).ok(), Some(vec![]));
        assert_eq!(fh(" 0", true).ok(), None);
        assert_eq!(fh(" 00", true).ok(), Some(vec![0x00]));
        assert_eq!(fh(" 09", true).ok(), Some(vec![0x09]));
        assert_eq!(fh(" 0f", true).ok(), Some(vec![0x0f]));
        assert_eq!(fh(" 99", true).ok(), Some(vec![0x99]));
        assert_eq!(fh(" ff", true).ok(), Some(vec![0xff]));
        assert_eq!(fh(" 00 0", true).ok(), None);
        assert_eq!(fh(" 00 00", true).ok(), Some(vec![0x00, 0x00]));
        assert_eq!(fh(" 00 09", true).ok(), Some(vec![0x00, 0x09]));
        assert_eq!(fh(" 00 0f", true).ok(), Some(vec![0x00, 0x0f]));
        assert_eq!(fh(" 00 99", true).ok(), Some(vec![0x00, 0x99]));
        assert_eq!(fh(" 00 ff", true).ok(), Some(vec![0x00, 0xff]));
        assert_eq!(fh("\t\n\x0c\r ", true).ok(), Some(vec![]));
        // Fancy Unicode spaces are ok too:
        assert_eq!(fh("     23", true).ok(), Some(vec![0x23]));
        assert_eq!(fh("a", true).ok(), None);
        assert_eq!(fh(" 0x", true).ok(), Some(vec![]));
        assert_eq!(fh(" 0x0", true).ok(), None);
        assert_eq!(fh(" 0x00", true).ok(), Some(vec![0x00]));
    }

    quickcheck! {
        fn hex_roundtrip(data: Vec<u8>) -> bool {
            let hex = super::to_hex(&data, false);
            data == super::from_hex(&hex, false).unwrap()
        }
    }

    quickcheck! {
        fn pretty_hex_roundtrip(data: Vec<u8>) -> bool {
            let hex = super::to_hex(&data, true);
            data == super::from_hex(&hex, true).unwrap()
        }
    }

    #[test]
    fn hex_dumper() {
        use super::hex::Dumper;

        let mut dumper = Dumper::new(Vec::new(), "III");
        dumper.write(&[0x89, 0x01, 0x33], "frame").unwrap();
        let buf = dumper.into_inner();
        assert_eq!(
            ::std::str::from_utf8(&buf[..]).unwrap(),
            "III00000000  \
             89 01 33                                           \
             frame\n");

        let mut dumper = Dumper::new(Vec::new(), "III");
        dumper.write(&[0x89, 0x01, 0x33, 0x89, 0x01, 0x33, 0x89, 0x01], "frame")
            .unwrap();
        let buf = dumper.into_inner();
        assert_eq!(
            ::std::str::from_utf8(&buf[..]).unwrap(),
            "III00000000  \
             89 01 33 89 01 33 89 01                            \
             frame\n");

        let mut dumper = Dumper::new(Vec::new(), "III");
        dumper.write(&[0x89, 0x01, 0x33, 0x89, 0x01, 0x33, 0x89, 0x01,
                       0x89, 0x01, 0x33, 0x89, 0x01, 0x33, 0x89, 0x01], "frame")
            .unwrap();
        let buf = dumper.into_inner();
        assert_eq!(
            ::std::str::from_utf8(&buf[..]).unwrap(),
            "III00000000  \
             89 01 33 89 01 33 89 01  89 01 33 89 01 33 89 01   \
             frame\n");

        let mut dumper = Dumper::new(Vec::new(), "III");
        dumper.write(&[0x89, 0x01, 0x33, 0x89, 0x01, 0x33, 0x89, 0x01,
                       0x89, 0x01, 0x33, 0x89, 0x01, 0x33, 0x89, 0x01,
                       0x89, 0x01, 0x33, 0x89, 0x01, 0x33, 0x89, 0x01,
                       0x89, 0x01, 0x33, 0x89, 0x01, 0x33, 0x89, 0x01], "frame")
            .unwrap();
        let buf = dumper.into_inner();
        assert_eq!(
            ::std::str::from_utf8(&buf[..]).unwrap(),
            "III00000000  \
             89 01 33 89 01 33 89 01  89 01 33 89 01 33 89 01   \
             frame\n\
             III00000010  \
             89 01 33 89 01 33 89 01  89 01 33 89 01 33 89 01\n");

        let mut dumper = Dumper::new(Vec::new(), "");
        dumper.write(&[0x89, 0x01, 0x33], "frame").unwrap();
        dumper.write(&[0x04], "version").unwrap();
        dumper.write(&[0x00], "type").unwrap();
        let buf = dumper.into_inner();
        assert_eq!(
            ::std::str::from_utf8(&buf[..]).unwrap(),
            "00000000  89 01 33                                           \
             frame\n\
             00000003           04                                        \
             version\n\
             00000004              00                                     \
             type\n\
             ");
    }

    #[test]
    fn time() {
        use super::time;
        use crate::types::Timestamp;
        let t = |epoch| -> std::time::SystemTime {
            Timestamp::from(epoch).into()
        };
        assert_eq!(&time(&t(1585217290)), "2020-03-26T10:08:10Z");
    }
}
