//! ASCII Armor.
//!
//! This module deals with ASCII Armored data (see [Section 6 of RFC
//! 4880]).
//!
//!   [Section 6 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-6
//!
//! # Scope
//!
//! This implements a subset of the ASCII Armor specification.  Not
//! supported multipart messages.
//!
//! # Memory allocations
//!
//! Both the reader and the writer allocate memory in the order of the
//! size of chunks read or written.
//!
//! # Examples
//!
//! ```rust, no_run
//! # fn main() -> sequoia_openpgp::Result<()> {
//! use sequoia_openpgp as openpgp;
//! use std::fs::File;
//! use openpgp::armor::{Reader, ReaderMode, Kind};
//!
//! let mut file = File::open("somefile.asc")?;
//! let mut r = Reader::from_reader(&mut file, ReaderMode::Tolerant(Some(Kind::File)));
//! # Ok(()) }
//! ```

use buffered_reader::BufferedReader;
use std::convert::TryFrom;
use std::fmt;
use std::io;
use std::io::{Cursor, Read, Write};
use std::io::{Result, Error, ErrorKind};
use std::path::Path;
use std::cmp;
use std::str;
use std::borrow::Cow;

#[cfg(test)]
use quickcheck::{Arbitrary, Gen};

use crate::packet::prelude::*;
use crate::packet::header::{BodyLength, CTBNew, CTBOld};
use crate::parse::Cookie;
use crate::serialize::MarshalInto;
use crate::{vec_resize, vec_truncate};

mod base64_utils;
use base64_utils::*;
mod crc;
use crc::Crc;

/// Whether to trace execution by default (on stderr).
const TRACE: bool = false;

/// The encoded output stream must be represented in lines of no more
/// than 76 characters each (see (see [RFC 4880, section
/// 6.3](https://tools.ietf.org/html/rfc4880#section-6.3).  GnuPG uses
/// 64.
pub(crate) const LINE_LENGTH: usize = 64;

const LINE_ENDING: &str = "\n";

/// Specifies the type of data (see [RFC 4880, section 6.2]).
///
/// [RFC 4880, section 6.2]: https://tools.ietf.org/html/rfc4880#section-6.2
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Kind {
    /// A generic OpenPGP message.  (Since its structure hasn't been
    /// validated, in this crate's terminology, this is just a
    /// `PacketPile`.)
    Message,
    /// A certificate.
    PublicKey,
    /// A transferable secret key.
    SecretKey,
    /// A detached signature.
    Signature,
    /// A generic file.  This is a GnuPG extension.
    File,
}
assert_send_and_sync!(Kind);

#[cfg(test)]
impl Arbitrary for Kind {
    fn arbitrary(g: &mut Gen) -> Self {
        use self::Kind::*;
        match u8::arbitrary(g) % 5 {
            0 => Message,
            1 => PublicKey,
            2 => SecretKey,
            3 => Signature,
            4 => File,
            _ => unreachable!(),
        }
    }
}

/// Specifies the kind of data as indicated by the label.
///
/// This is a non-public variant of `Kind` that is currently only used
/// for detecting the kind on consumption.
///
/// See also <https://gitlab.com/sequoia-pgp/sequoia/-/issues/672>.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Label {
    /// A generic OpenPGP message.  (Since its structure hasn't been
    /// validated, in this crate's terminology, this is just a
    /// `PacketPile`.)
    Message,
    /// A certificate.
    PublicKey,
    /// A transferable secret key.
    SecretKey,
    /// A detached signature.
    Signature,
    /// A message using the Cleartext Signature Framework.
    ///
    /// See [Section 7 of RFC 4880].
    ///
    ///   [Section 7 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-7
    CleartextSignature,
    /// A generic file.  This is a GnuPG extension.
    File,
}
assert_send_and_sync!(Label);

impl TryFrom<Label> for Kind {
    type Error = crate::Error;
    fn try_from(l: Label) -> std::result::Result<Self, Self::Error> {
        match l {
            Label::Message => Ok(Kind::Message),
            Label::PublicKey => Ok(Kind::PublicKey),
            Label::SecretKey => Ok(Kind::SecretKey),
            Label::Signature => Ok(Kind::Signature),
            Label::File => Ok(Kind::File),
            Label::CleartextSignature => Err(crate::Error::InvalidOperation(
                "armor::Kind cannot express cleartext signatures".into())),
        }
    }
}

impl Label {
    /// Detects the header returning the kind and length of the
    /// header.
    fn detect_header(blurb: &[u8]) -> Option<(Self, usize)> {
        let (leading_dashes, rest) = dash_prefix(blurb);

        // Skip over "BEGIN PGP "
        if ! rest.starts_with(b"BEGIN PGP ") {
            return None;
        }
        let rest = &rest[b"BEGIN PGP ".len()..];

        // Detect kind.
        let kind = if rest.starts_with(b"MESSAGE") {
            Label::Message
        } else if rest.starts_with(b"PUBLIC KEY BLOCK") {
            Label::PublicKey
        } else if rest.starts_with(b"PRIVATE KEY BLOCK") {
            Label::SecretKey
        } else if rest.starts_with(b"SIGNATURE") {
            Label::Signature
        } else if rest.starts_with(b"SIGNED MESSAGE") {
            Label::CleartextSignature
        } else if rest.starts_with(b"ARMORED FILE") {
            Label::File
        } else {
            return None;
        };

        let (trailing_dashes, _) = dash_prefix(&rest[kind.blurb().len()..]);
        Some((kind,
              leading_dashes.len()
              + b"BEGIN PGP ".len() + kind.blurb().len()
              + trailing_dashes.len()))
    }

    fn blurb(&self) -> &str {
        match self {
            Label::Message => "MESSAGE",
            Label::PublicKey => "PUBLIC KEY BLOCK",
            Label::SecretKey => "PRIVATE KEY BLOCK",
            Label::Signature => "SIGNATURE",
            Label::CleartextSignature => "SIGNED MESSAGE",
            Label::File => "ARMORED FILE",
        }
    }

}

impl Kind {
    /// Detects the footer returning length of the footer.
    fn detect_footer(&self, blurb: &[u8]) -> Option<usize> {
        let (leading_dashes, rest) = dash_prefix(blurb);

        // Skip over "END PGP "
        if ! rest.starts_with(b"END PGP ") {
            return None;
        }
        let rest = &rest[b"END PGP ".len()..];

        let ident = self.blurb().as_bytes();
        if ! rest.starts_with(ident) {
            return None;
        }

        let (trailing_dashes, _) = dash_prefix(&rest[ident.len()..]);
        Some(leading_dashes.len()
             + b"END PGP ".len() + ident.len()
             + trailing_dashes.len())
    }

    fn blurb(&self) -> &str {
        match self {
            Kind::Message => "MESSAGE",
            Kind::PublicKey => "PUBLIC KEY BLOCK",
            Kind::SecretKey => "PRIVATE KEY BLOCK",
            Kind::Signature => "SIGNATURE",
            Kind::File => "ARMORED FILE",
        }
    }

    fn begin(&self) -> String {
        format!("-----BEGIN PGP {}-----", self.blurb())
    }

    fn end(&self) -> String {
        format!("-----END PGP {}-----", self.blurb())
    }
}

/// A filter that applies ASCII Armor to the data written to it.
pub struct Writer<W: Write> {
    sink: W,
    kind: Kind,
    stash: Vec<u8>,
    column: usize,
    crc: Crc,
    header: Vec<u8>,
    dirty: bool,
    scratch: Vec<u8>,
}
assert_send_and_sync!(Writer<W> where W: Write);

impl<W: Write> Writer<W> {
    /// Constructs a new filter for the given type of data.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::io::{Read, Write, Cursor};
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::armor::{Writer, Kind};
    ///
    /// # fn main() -> std::io::Result<()> {
    /// let mut writer = Writer::new(Vec::new(), Kind::File)?;
    /// writer.write_all(b"Hello world!")?;
    /// let buffer = writer.finalize()?;
    /// assert_eq!(
    ///     String::from_utf8_lossy(&buffer),
    ///     "-----BEGIN PGP ARMORED FILE-----
    ///
    /// SGVsbG8gd29ybGQh
    /// =s4Gu
    /// -----END PGP ARMORED FILE-----
    /// ");
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(inner: W, kind: Kind) -> Result<Self> {
        Self::with_headers(inner, kind, Option::<(&str, &str)>::None)
    }

    /// Constructs a new filter for the given type of data.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::io::{Read, Write, Cursor};
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::armor::{Writer, Kind};
    ///
    /// # fn main() -> std::io::Result<()> {
    /// let mut writer = Writer::with_headers(Vec::new(), Kind::File,
    ///     vec![("Key", "Value")])?;
    /// writer.write_all(b"Hello world!")?;
    /// let buffer = writer.finalize()?;
    /// assert_eq!(
    ///     String::from_utf8_lossy(&buffer),
    ///     "-----BEGIN PGP ARMORED FILE-----
    /// Key: Value
    ///
    /// SGVsbG8gd29ybGQh
    /// =s4Gu
    /// -----END PGP ARMORED FILE-----
    /// ");
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_headers<I, K, V>(inner: W, kind: Kind, headers: I)
                                 -> Result<Self>
        where I: IntoIterator<Item = (K, V)>,
              K: AsRef<str>,
              V: AsRef<str>,
    {
        let mut w = Writer {
            sink: inner,
            kind,
            stash: Vec::<u8>::with_capacity(2),
            column: 0,
            crc: Crc::new(),
            header: Vec::with_capacity(128),
            dirty: false,
            scratch: vec![0; 4096],
        };

        {
            let mut cur = Cursor::new(&mut w.header);
            write!(&mut cur, "{}{}", kind.begin(), LINE_ENDING)?;

            for h in headers {
                write!(&mut cur, "{}: {}{}", h.0.as_ref(), h.1.as_ref(),
                       LINE_ENDING)?;
            }

            // A blank line separates the headers from the body.
            write!(&mut cur, "{}", LINE_ENDING)?;
        }

        Ok(w)
    }

    /// Returns a reference to the inner writer.
    pub fn get_ref(&self) -> &W {
        &self.sink
    }

    /// Returns a mutable reference to the inner writer.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.sink
    }

    fn finalize_headers(&mut self) -> Result<()> {
        if ! self.dirty {
            self.dirty = true;
            self.sink.write_all(&self.header)?;
            // Release memory.
            crate::vec_truncate(&mut self.header, 0);
            self.header.shrink_to_fit();
        }
        Ok(())
    }

    /// Writes the footer.
    ///
    /// This function needs to be called explicitly before the writer is dropped.
    pub fn finalize(mut self) -> Result<W> {
        if ! self.dirty {
            // No data was written to us, don't emit anything.
            return Ok(self.sink);
        }
        self.finalize_armor()?;
        Ok(self.sink)
    }

    /// Writes the footer.
    fn finalize_armor(&mut self) -> Result<()> {
        if ! self.dirty {
            // No data was written to us, don't emit anything.
            return Ok(());
        }
        self.finalize_headers()?;

        // Write any stashed bytes and pad.
        if !self.stash.is_empty() {
            self.sink.write_all(base64::encode_config(
                &self.stash, base64::STANDARD).as_bytes())?;
            self.column += 4;
        }

        // Inserts a line break if necessary.
        //
        // Unfortunately, we cannot use
        //self.linebreak()?;
        //
        // Therefore, we inline it here.  This is a bit sad.
        assert!(self.column <= LINE_LENGTH);
        if self.column == LINE_LENGTH {
            write!(self.sink, "{}", LINE_ENDING)?;
            self.column = 0;
        }

        if self.column > 0 {
            write!(self.sink, "{}", LINE_ENDING)?;
        }

        // 24-bit CRC
        let crc = self.crc.finalize();
        let bytes = &crc.to_be_bytes()[1..4];

        // CRC and footer.
        write!(self.sink, "={}{}{}{}",
               base64::encode_config(&bytes, base64::STANDARD_NO_PAD),
               LINE_ENDING, self.kind.end(), LINE_ENDING)?;

        self.dirty = false;
        crate::vec_truncate(&mut self.scratch, 0);
        Ok(())
    }

    /// Inserts a line break if necessary.
    fn linebreak(&mut self) -> Result<()> {
        assert!(self.column <= LINE_LENGTH);
        if self.column == LINE_LENGTH {
            write!(self.sink, "{}", LINE_ENDING)?;
            self.column = 0;
        }
        Ok(())
    }
}

impl<W: Write> Write for Writer<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.finalize_headers()?;
        assert!(self.dirty);

        // Update CRC on the unencoded data.
        self.crc.update(buf);

        let mut input = buf;
        let mut written = 0;

        // First of all, if there are stashed bytes, fill the stash
        // and encode it.  If writing out the stash fails below, we
        // might end up with a stash of size 3.
        assert!(self.stash.len() <= 3);
        if !self.stash.is_empty() {
            let missing = 3 - self.stash.len();
            let n = missing.min(input.len());
            self.stash.extend_from_slice(&input[..n]);
            input = &input[n..];
            written += n;
            if input.is_empty() {
                // We exhausted the input.  Return now, any stashed
                // bytes are encoded when finalizing the writer.
                return Ok(written);
            }
            assert_eq!(self.stash.len(), 3);

            // If this fails for some reason, and the caller retries
            // the write, we might end up with a stash of size 3.
            self.sink
                .write_all(base64::encode_config(
                    &self.stash, base64::STANDARD_NO_PAD).as_bytes())?;
            self.column += 4;
            self.linebreak()?;
            crate::vec_truncate(&mut self.stash, 0);
        }

        // Encode all whole blocks of 3 bytes.
        let n_blocks = input.len() / 3;
        let input_bytes = n_blocks * 3;
        if input_bytes > 0 {
            // Encrypt whole blocks.
            let encoded_bytes = n_blocks * 4;
            if self.scratch.len() < encoded_bytes {
                vec_resize(&mut self.scratch, encoded_bytes);
            }

            written += input_bytes;
            base64::encode_config_slice(&input[..input_bytes],
                                        base64::STANDARD_NO_PAD,
                                        &mut self.scratch[..encoded_bytes]);

            let mut n = 0;
            while ! self.scratch[n..encoded_bytes].is_empty() {
                let m = self.scratch[n..encoded_bytes].len()
                    .min(LINE_LENGTH - self.column);
                self.sink.write_all(&self.scratch[n..n + m])?;
                n += m;
                self.column += m;
                self.linebreak()?;
            }
        }

        // Stash rest for later.
        input = &input[input_bytes..];
        assert!(input.is_empty() || self.stash.is_empty());
        self.stash.extend_from_slice(input);
        written += input.len();

        assert_eq!(written, buf.len());
        Ok(written)
    }

    fn flush(&mut self) -> Result<()> {
        self.sink.flush()
    }
}

/// How an ArmorReader should act.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReaderMode {
    /// Makes the armor reader tolerant of simple errors.
    ///
    /// The armor reader will be tolerant of common formatting errors,
    /// such as incorrect line folding, but the armor header line
    /// (e.g., `----- BEGIN PGP MESSAGE -----`) and the footer must be
    /// intact.
    ///
    /// If a Kind is specified, then only ASCII Armor blocks with the
    /// appropriate header are recognized.
    ///
    /// This mode is appropriate when reading from a file.
    Tolerant(Option<Kind>),

    /// Makes the armor reader very tolerant of errors.
    ///
    /// Unlike in `Tolerant` mode, in this mode, the armor reader
    /// doesn't require an armor header line.  Instead, it examines
    /// chunks that look like valid base64 data, and attempts to parse
    /// them.
    ///
    /// Although this mode looks for OpenPGP fingerprints before
    /// invoking the full parser, due to the number of false
    /// positives, this mode of operation is CPU intense, particularly
    /// on large text files.  It is primarily appropriate when reading
    /// text that the user cut and pasted into a text area.
    VeryTolerant,
}
assert_send_and_sync!(ReaderMode);

/// A filter that strips ASCII Armor from a stream of data.
#[derive(Debug)]
pub struct Reader<'a> {
    // The following fields are the state of an embedded
    // buffered_reader::Generic.  We need to be able to access the
    // cookie in Self::initialize, therefore using
    // buffered_reader::Generic as we used to is no longer an option.
    //
    // XXX: Directly implement the BufferedReader protocol.  This may
    // actually simplify the code and reduce the required buffering.

    buffer: Option<Vec<u8>>,
    /// Currently unused buffer, a cache.
    unused_buffer: Option<Vec<u8>>,
    // The next byte to read in the buffer.
    cursor: usize,
    // The preferred chunk size.  This is just a hint.
    preferred_chunk_size: usize,
    // The wrapped reader.
    source: Box<dyn BufferedReader<Cookie> + 'a>,
    // Stashed error, if any.
    error: Option<Error>,
    /// Whether we hit EOF on the underlying reader.
    eof: bool,
    // The user settable cookie.
    cookie: Cookie,
    // End fields of the embedded generic reader.

    kind: Option<Kind>,
    mode: ReaderMode,
    decode_buffer: Vec<u8>,
    initialized: bool,
    headers: Vec<(String, String)>,
    finalized: bool,
    prefix: Vec<u8>,
    prefix_remaining: usize,

    /// Controls the transformation of messages using the Cleartext
    /// Signature Framework into inline signed messages.
    enable_csft: bool,

    /// State for the CSF transformer.
    csft: Option<CSFTransformer>,
}
assert_send_and_sync!(Reader<'_>);

// The default buffer size.
const DEFAULT_BUF_SIZE: usize = 8 * 1024;

impl<'a> fmt::Display for Reader<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "armor::Reader")
    }
}

impl Default for ReaderMode {
    fn default() -> Self {
        ReaderMode::Tolerant(None)
    }
}

/// State for transforming a message using the Cleartext Signature
/// Framework into an inline signed message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
enum CSFTransformer {
    OPS,
    Literal,
    Signatures,
}

impl Default for CSFTransformer {
    fn default() -> Self {
        CSFTransformer::OPS
    }
}

impl<'a> Reader<'a> {
    /// Constructs a new filter for the given type of data.
    ///
    /// This function is deprecated and will be removed in version
    /// 2.0. Please use [`Reader::from_reader`][].
    #[deprecated = "Use Reader::from_reader. `new` will be removed in version 2.0"]
    pub fn new<R, M>(inner: R, mode: M) -> Self
        where R: 'a + Read + Send + Sync,
              M: Into<Option<ReaderMode>>
    {
        Self::from_buffered_reader(
            Box::new(buffered_reader::Generic::with_cookie(inner, None,
                                                           Default::default())),
            mode, Default::default())
    }

    /// Constructs a new `Reader` from the given `io::Read`er.
    ///
    /// [ASCII Armor], designed to protect OpenPGP data in transit,
    /// has been a source of problems if the armor structure is
    /// damaged.  For example, copying data manually from one program
    /// to another might introduce or drop newlines.
    ///
    /// By default, the reader operates in tolerant mode.  It will
    /// ignore common formatting errors but the header and footer
    /// lines must be intact.
    ///
    /// To select stricter mode, specify the kind argument for
    /// tolerant mode.  In this mode only ASCII Armor blocks with the
    /// appropriate header are recognized.
    ///
    /// There is also very tolerant mode that is appropriate when
    /// reading text that the user cut and pasted into a text area.
    /// This mode of operation is CPU intense, particularly on large
    /// text files.
    ///
    ///   [ASCII Armor]: https://tools.ietf.org/html/rfc4880#section-6.2
    ///
    /// # Examples
    ///
    /// ```
    /// use std::io::{self, Read};
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Message;
    /// use openpgp::armor::{Reader, ReaderMode};
    /// use openpgp::parse::Parse;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let data = "yxJiAAAAAABIZWxsbyB3b3JsZCE="; // base64 over literal data packet
    ///
    /// let mut cursor = io::Cursor::new(&data);
    /// let mut reader = Reader::from_reader(&mut cursor, ReaderMode::VeryTolerant);
    ///
    /// let mut buf = Vec::new();
    /// reader.read_to_end(&mut buf)?;
    ///
    /// let message = Message::from_bytes(&buf)?;
    /// assert_eq!(message.body().unwrap().body(),
    ///            b"Hello world!");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Or, in strict mode:
    ///
    /// ```
    /// use std::io::{self, Result, Read};
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::armor::{Reader, ReaderMode, Kind};
    ///
    /// # fn main() -> Result<()> {
    /// let data =
    ///     "-----BEGIN PGP ARMORED FILE-----
    ///
    ///      SGVsbG8gd29ybGQh
    ///      =s4Gu
    ///      -----END PGP ARMORED FILE-----";
    ///
    /// let mut cursor = io::Cursor::new(&data);
    /// let mut reader = Reader::from_reader(&mut cursor, ReaderMode::Tolerant(Some(Kind::File)));
    ///
    /// let mut content = String::new();
    /// reader.read_to_string(&mut content)?;
    /// assert_eq!(content, "Hello world!");
    /// assert_eq!(reader.kind(), Some(Kind::File));
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_reader<R, M>(reader: R, mode: M) -> Self
        where R: 'a + Read + Send + Sync,
              M: Into<Option<ReaderMode>>
    {
        Self::from_buffered_reader(
            Box::new(buffered_reader::Generic::with_cookie(reader, None,
                                                           Default::default())),
            mode, Default::default())
    }

    /// Creates a `Reader` from a file.
    pub fn from_file<P, M>(path: P, mode: M) -> Result<Self>
        where P: AsRef<Path>,
              M: Into<Option<ReaderMode>>
    {
        Ok(Self::from_buffered_reader(
            Box::new(buffered_reader::File::with_cookie(path,
                                                        Default::default())?),
            mode, Default::default()))
    }

    /// Creates a `Reader` from a buffer.
    pub fn from_bytes<M>(bytes: &'a [u8], mode: M) -> Self
        where M: Into<Option<ReaderMode>>
    {
        Self::from_buffered_reader(
            Box::new(buffered_reader::Memory::with_cookie(bytes,
                                                          Default::default())),
            mode, Default::default())
    }

    pub(crate) fn from_buffered_reader<M>(
        inner: Box<dyn BufferedReader<Cookie> + 'a>, mode: M, cookie: Cookie)
        -> Self
        where M: Into<Option<ReaderMode>>
    {
        Self::from_buffered_reader_csft(inner, mode.into(), cookie, false)
    }

    pub(crate) fn from_buffered_reader_csft(
        inner: Box<dyn BufferedReader<Cookie> + 'a>,
        mode: Option<ReaderMode>,
        cookie: Cookie,
        enable_csft: bool,
    )
        -> Self
    {
        let mode = mode.unwrap_or_default();

        Reader {
            // The embedded generic reader's fields.
            buffer: None,
            unused_buffer: None,
            cursor: 0,
            preferred_chunk_size: DEFAULT_BUF_SIZE,
            source: inner,
            error: None,
            eof: false,
            cookie,
            // End of the embedded generic reader's fields.

            kind: None,
            mode,
            decode_buffer: Vec::<u8>::with_capacity(1024),
            headers: Vec::new(),
            initialized: false,
            finalized: false,
            prefix: Vec::with_capacity(0),
            prefix_remaining: 0,
            enable_csft,
            csft: None,
        }
    }

    /// Returns the kind of data this reader is for.
    ///
    /// Useful if the kind of data is not known in advance.  If the
    /// header has not been encountered yet (try reading some data
    /// first!), this function returns None.
    pub fn kind(&self) -> Option<Kind> {
        self.kind
    }

    /// Returns the armored headers.
    ///
    /// The tuples contain a key and a value.
    ///
    /// Note: if a key occurs multiple times, then there are multiple
    /// entries in the vector with the same key; values with the same
    /// key are *not* combined.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::io::{self, Read};
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::armor::{Reader, ReaderMode, Kind};
    ///
    /// # fn main() -> std::io::Result<()> {
    /// let data =
    ///     "-----BEGIN PGP ARMORED FILE-----
    ///      First: value
    ///      Header: value
    ///
    ///      SGVsbG8gd29ybGQh
    ///      =s4Gu
    ///      -----END PGP ARMORED FILE-----";
    ///
    /// let mut cursor = io::Cursor::new(&data);
    /// let mut reader = Reader::from_reader(&mut cursor, ReaderMode::Tolerant(Some(Kind::File)));
    ///
    /// let mut content = String::new();
    /// reader.read_to_string(&mut content)?;
    /// assert_eq!(reader.headers()?,
    ///    &[("First".into(), "value".into()),
    ///      ("Header".into(), "value".into())]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn headers(&mut self) -> Result<&[(String, String)]> {
        self.initialize()?;
        Ok(&self.headers[..])
    }
}

impl<'a> Reader<'a> {
    /// Consumes the header if not already done.
    #[allow(clippy::nonminimal_bool)]
    fn initialize(&mut self) -> Result<()> {
        if self.initialized { return Ok(()) }

        // The range of the first 6 bits of a message is limited.
        // Save cpu cycles by only considering base64 data that starts
        // with one of those characters.
        lazy_static::lazy_static!{
            static ref START_CHARS_VERY_TOLERANT: Vec<u8> = {
                let mut valid_start = Vec::new();
                for &tag in &[ Tag::PKESK, Tag::SKESK,
                              Tag::OnePassSig, Tag::Signature,
                              Tag::PublicKey, Tag::SecretKey,
                              Tag::CompressedData, Tag::Literal,
                              Tag::Marker,
                ] {
                    let mut ctb = [ 0u8; 1 ];
                    let mut o = [ 0u8; 4 ];

                    CTBNew::new(tag).serialize_into(&mut ctb[..]).unwrap();
                    base64::encode_config_slice(&ctb[..], base64::STANDARD, &mut o[..]);
                    valid_start.push(o[0]);

                    CTBOld::new(tag, BodyLength::Full(0)).unwrap()
                        .serialize_into(&mut ctb[..]).unwrap();
                    base64::encode_config_slice(&ctb[..], base64::STANDARD, &mut o[..]);
                    valid_start.push(o[0]);
                }

                // Add all first bytes of Unicode characters from the
                // "Dash Punctuation" category.
                let mut b = [0; 4]; // Enough to hold any UTF-8 character.
                for d in dashes() {
                    d.encode_utf8(&mut b);
                    valid_start.push(b[0]);
                }

                // If there are no dashes at all, match on the BEGIN.
                valid_start.push(b'B');

                valid_start.sort_unstable();
                valid_start.dedup();
                valid_start
            };

            static ref START_CHARS_TOLERANT: Vec<u8> = {
                let mut valid_start = Vec::new();
                // Add all first bytes of Unicode characters from the
                // "Dash Punctuation" category.
                let mut b = [0; 4]; // Enough to hold any UTF-8 character.
                for d in dashes() {
                    d.encode_utf8(&mut b);
                    valid_start.push(b[0]);
                }

                // If there are no dashes at all, match on the BEGIN.
                valid_start.push(b'B');

                valid_start.sort_unstable();
                valid_start.dedup();
                valid_start
            };
        }

        // Look for the Armor Header Line, skipping any garbage in the
        // process.
        let mut found_blob = false;
        let start_chars = if self.mode != ReaderMode::VeryTolerant {
            &START_CHARS_TOLERANT[..]
        } else {
            &START_CHARS_VERY_TOLERANT[..]
        };

        let mut lines = 0;
        let mut prefix = Vec::new();
        let n = 'search: loop {
            if lines > 0 {
                // Find the start of the next line.
                self.source.drop_through(&[b'\n'], true)?;
                crate::vec_truncate(&mut prefix, 0);
            }
            lines += 1;

            // Ignore leading whitespace, etc.
            while matches!(self.source.data_hard(1)?[0],
                // Skip some whitespace (previously .is_ascii_whitespace())
                b' ' | b'\t' | b'\r' | b'\n' |
                // Also skip common quote characters
                b'>' | b'|' | b']' | b'}' )
            {
                let c = self.source.data(1)?[0];
                if c == b'\n' {
                    // We found a newline while walking whitespace, reset prefix
                    crate::vec_truncate(&mut prefix, 0);
                } else {
                    prefix.push(self.source.data_hard(1)?[0]);
                }
                self.source.consume(1);
            }

            // Don't bother if the first byte is not plausible.
            let start = self.source.data_hard(1)?[0];
            if !start_chars.binary_search(&start).is_ok()
            {
                self.source.consume(1);
                continue;
            }

            {
                let mut input = self.source.data(128)?;
                let n = input.len();

                if n == 0 {
                    return Err(
                        Error::new(ErrorKind::InvalidInput,
                                   "Reached EOF looking for Armor Header Line"));
                }
                if n > 128 {
                    input = &input[..128];
                }

                // Possible ASCII-armor header.
                if let Some((label, len)) = Label::detect_header(input) {
                    if label == Label::CleartextSignature && ! self.enable_csft
                    {
                        // We found a message using the Cleartext
                        // Signature Framework, but the CSF
                        // transformation is not enabled.  Continue
                        // searching until we find the bare signature.
                        continue 'search;
                    }

                    if label == Label::CleartextSignature && self.enable_csft
                    {
                        // Initialize the transformer.
                        self.csft = Some(CSFTransformer::default());

                        // Signal to the parser stack that the CSF
                        // transformation is happening.  This will be
                        // used by the HashedReader (specifically, in
                        // Cookie::processing_csf_message and
                        // Cookie::hash_update) to select the correct
                        // hashing method.
                        self.cookie.set_processing_csf_message();

                        // We'll be looking for the signature framing next.
                        self.kind = Some(Kind::Signature);
                        break 'search len;
                    }
                    let kind = Kind::try_from(label)
                        .expect("cleartext signature handled above");

                    let mut expected_kind = None;
                    if let ReaderMode::Tolerant(Some(kind)) = self.mode {
                        expected_kind = Some(kind);
                    }

                    if expected_kind == None {
                        // Found any!
                        self.kind = Some(kind);
                        break 'search len;
                    }

                    if expected_kind == Some(kind) {
                        // Found it!
                        self.kind = Some(kind);
                        break 'search len;
                    }
                }

                if self.mode == ReaderMode::VeryTolerant {
                    // The user did not specify what kind of data she
                    // wants.  We aggressively try to decode any data,
                    // even if we do not see a valid header.
                    if is_armored_pgp_blob(input) {
                        found_blob = true;
                        break 'search 0;
                    }
                }
            }
        };
        self.source.consume(n);

        if found_blob {
            // Skip the rest of the initialization.
            self.initialized = true;
            self.prefix_remaining = prefix.len();
            self.prefix = prefix;
            return Ok(());
        }

        self.prefix = prefix;
        self.read_headers()
    }

    /// Reads headers and finishes the initialization.
    fn read_headers(&mut self) -> Result<()> {
        // We consumed the header above, but not any trailing
        // whitespace and the trailing new line.  We do that now.
        // Other data between the header and the new line are not
        // allowed.  But, instead of failing, we try to recover, by
        // stopping at the first non-whitespace character.
        let n = {
            let line = self.source.read_to(b'\n')?;
            line.iter().position(|&c| {
                !c.is_ascii_whitespace()
            }).unwrap_or(line.len())
        };
        self.source.consume(n);

        let next_prefix =
            &self.source.data_hard(self.prefix.len())?[..self.prefix.len()];
        if self.prefix != next_prefix {
            // If the next line doesn't start with the same prefix, we assume
            // it was garbage on the front and drop the prefix so long as it
            // was purely whitespace.  Any non-whitespace remains an error
            // while searching for the armor header if it's not repeated.
            if self.prefix.iter().all(|b| (*b as char).is_ascii_whitespace()) {
                crate::vec_truncate(&mut self.prefix, 0);
            } else {
                // Nope, we have actually failed to read this properly
                return Err(
                    Error::new(ErrorKind::InvalidInput,
                               "Inconsistent quoting of armored data"));
            }
        }

        // Read the key-value headers.
        let mut n = 0;
        // Sometimes, we find a truncated prefix.  In these cases, the
        // length is not prefix.len(), but this.
        let mut prefix_len = None;
        let mut lines = 0;
        loop {
            // Skip any known prefix on lines.
            //
            // IMPORTANT: We need to buffer the prefix so that we can
            // consume it here.  So at every point in this loop where
            // the control flow wraps around, we need to make sure
            // that we buffer the prefix in addition to the line.
            self.source.consume(
                prefix_len.take().unwrap_or_else(|| self.prefix.len()));

            self.source.consume(n);

            // Buffer the next line.
            let line = self.source.read_to(b'\n')?;
            n = line.len();
            lines += 1;

            let line = str::from_utf8(line);
            // Ignore---don't error out---lines that are not valid UTF8.
            if line.is_err() {
                // Buffer the next line and the prefix that is going
                // to be consumed in the next iteration.
                let next_prefix =
                    &self.source.data_hard(n + self.prefix.len())?
                        [n..n + self.prefix.len()];
                if self.prefix != next_prefix {
                    return Err(
                        Error::new(ErrorKind::InvalidInput,
                                   "Inconsistent quoting of armored data"));
                }
                continue;
            }

            let line = line.unwrap();

            // The line almost certainly ends with \n: the only reason
            // it couldn't is if we encountered EOF.  We need to strip
            // it.  But, if it ends with \r\n, then we also want to
            // strip the \r too.
            let line = if let Some(rest) = line.strip_suffix("\r\n") {
                // \r\n.
                rest
            } else if let Some(rest) = line.strip_suffix('\n') {
                // \n.
                rest
            } else {
                // EOF.
                line
            };

            /* Process headers.  */
            let key_value = line.splitn(2, ": ").collect::<Vec<&str>>();
            if key_value.len() == 1 {
                if line.trim_start().is_empty() {
                    // Empty line.
                    break;
                } else if lines == 1 {
                    // This is the first line and we don't have a
                    // key-value pair.  It seems more likely that
                    // we're just missing a newline and this invalid
                    // header is actually part of the body.
                    n = 0;
                    break;
                }
            } else {
                let key = key_value[0].trim_start();
                let value = key_value[1];

                self.headers.push((key.into(), value.into()));
            }

            // Buffer the next line and the prefix that is going to be
            // consumed in the next iteration.
            let next_prefix =
                &self.source.data_hard(n + self.prefix.len())?
                    [n..n + self.prefix.len()];

            // Sometimes, we find a truncated prefix.
            let l = common_prefix(&self.prefix, next_prefix);
            let full_prefix = l == self.prefix.len();
            if ! (full_prefix
                  // Truncation is okay if the rest of the prefix
                  // contains only whitespace.
                  || self.prefix[l..].iter().all(|c| c.is_ascii_whitespace()))
            {
                return Err(
                    Error::new(ErrorKind::InvalidInput,
                               "Inconsistent quoting of armored data"));
            }
            if ! full_prefix {
                // Make sure to only consume the truncated prefix in
                // the next loop iteration.
                prefix_len = Some(l);
            }
        }
        self.source.consume(n);

        self.initialized = true;
        self.prefix_remaining = self.prefix.len();
        Ok(())
    }
}

/// Computes the length of the common prefix.
fn common_prefix<A: AsRef<[u8]>, B: AsRef<[u8]>>(a: A, b: B) -> usize {
    a.as_ref().iter().zip(b.as_ref().iter()).take_while(|(a, b)| a == b).count()
}

impl<'a> Reader<'a> {
    fn read_armored_data(&mut self, buf: &mut [u8]) -> Result<usize> {
        let (consumed, decoded) = if !self.decode_buffer.is_empty() {
            // We have something buffered, use that.

            let amount = cmp::min(buf.len(), self.decode_buffer.len());
            buf[..amount].copy_from_slice(&self.decode_buffer[..amount]);
            crate::vec_drain_prefix(&mut self.decode_buffer, amount);

            (0, amount)
        } else {
            // We need to decode some data.  We consider three cases,
            // all a function of the size of `buf`:
            //
            //   - Tiny: if `buf` can hold less than three bytes, then
            //     we almost certainly have to double buffer: except
            //     at the very end, a base64 chunk consists of 3 bytes
            //     of data.
            //
            //     Note: this happens if the caller does `for c in
            //     Reader::from_reader(...).bytes() ...`.  Then it
            //     reads one byte of decoded data at a time.
            //
            //   - Small: if the caller only requests a few bytes at a
            //     time, we may as well double buffer to reduce
            //     decoding overhead.
            //
            //   - Large: if `buf` is large, we can decode directly
            //     into `buf` and avoid double buffering.  But,
            //     because we ignore whitespace, it is hard to
            //     determine exactly how much data to read to
            //     maximally fill `buf`.

            // We use 64, because ASCII-armor text usually contains 64
            // characters of base64 data per line, and this prevents
            // turning the borrow into an own.
            const THRESHOLD : usize = 64;

            let to_read =
                cmp::max(
                    // Tiny or small:
                    THRESHOLD + 2,

                    // Large: a heuristic:

                    base64_size(buf.len())
                    // Assume about 2 bytes of whitespace (crlf) per
                    // 64 character line.
                        + 2 * ((buf.len() + 63) / 64));

            let base64data = self.source.data(to_read)?;
            let base64data = if base64data.len() > to_read {
                &base64data[..to_read]
            } else {
                base64data
            };

            let (base64data, consumed, prefix_remaining)
                = base64_filter(Cow::Borrowed(base64data),
                                // base64_size rounds up, but we want
                                // to round down as we have to double
                                // buffer partial chunks.
                                cmp::max(THRESHOLD, buf.len() / 3 * 4),
                                self.prefix_remaining,
                                self.prefix.len());

            // We shouldn't have any partial chunks.
            assert_eq!(base64data.len() % 4, 0);

            let decoded = if base64data.len() / 4 * 3 > buf.len() {
                // We need to double buffer.  Decode into a vector.
                // (Note: the computed size *might* be a slight
                // overestimate, because the last base64 chunk may
                // include padding.)
                self.decode_buffer = base64::decode_config(
                    &base64data, base64::STANDARD)
                    .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;

                let copied = cmp::min(buf.len(), self.decode_buffer.len());
                buf[..copied].copy_from_slice(&self.decode_buffer[..copied]);
                crate::vec_drain_prefix(&mut self.decode_buffer, copied);

                copied
            } else {
                // We can decode directly into the caller-supplied
                // buffer.
                base64::decode_config_slice(
                    &base64data, base64::STANDARD, buf)
                    .map_err(|e| Error::new(ErrorKind::InvalidData, e))?
            };

            self.prefix_remaining = prefix_remaining;

            (consumed, decoded)
        };

        self.source.consume(consumed);
        if decoded == 0 {
            self.finalized = true;

            /* Look for CRC.  The CRC is optional.  */
            let consumed = {
                // Skip whitespace.
                while !self.source.data(1)?.is_empty()
                    && self.source.buffer()[0].is_ascii_whitespace()
                {
                    self.source.consume(1);
                }

                let data = self.source.data(5)?;
                let data = if data.len() > 5 {
                    &data[..5]
                } else {
                    data
                };

                if data.len() == 5
                    && data[0] == b'='
                    && data[1..5].iter().all(is_base64_char)
                {
                    /* Found.  */
                    5
                } else {
                    0
                }
            };
            self.source.consume(consumed);

            // Skip any expected prefix
            self.source.data_consume_hard(self.prefix.len())?;
            // Look for a footer.
            let consumed = {
                // Skip whitespace.
                while !self.source.data(1)?.is_empty()
                    && self.source.buffer()[0].is_ascii_whitespace()
                {
                    self.source.consume(1);
                }

                // If we had a header, we require a footer.
                if let Some(kind) = self.kind {
                    let footer_lookahead = 128; // Why not.
                    let got = self.source.data(footer_lookahead)?;
                    let got = if got.len() > footer_lookahead {
                        &got[..footer_lookahead]
                    } else {
                        got
                    };
                    if let Some(footer_len) = kind.detect_footer(got) {
                        footer_len
                    } else {
                        return Err(Error::new(ErrorKind::InvalidInput,
                                              "Invalid ASCII Armor footer."));
                    }
                } else {
                    0
                }
            };
            self.source.consume(consumed);
        }

        Ok(decoded)
    }

    fn read_clearsigned_message(&mut self, buf: &mut [u8]) -> Result<usize> {
        // XXX: We're not terribly concerned with performance at this
        // point, there is room for improvement.

        use std::collections::HashSet;
        use crate::{
            types::{DataFormat, HashAlgorithm, SignatureType},
            serialize::Serialize,
        };

        assert!(self.csft.is_some());
        if self.decode_buffer.is_empty() {
            match self.csft.as_ref().expect("CSFT has been initialized") {
                CSFTransformer::OPS => {
                    // Determine the set of hash algorithms.
                    let mut algos: HashSet<HashAlgorithm> = self.headers.iter()
                        .filter(|(key, _value)| key == "Hash")
                        .flat_map(|(_key, value)| {
                            value.split(',')
                                .filter_map(|hash| hash.parse().ok())
                        }).collect();

                    if algos.is_empty() {
                        // The default is MD5.
                        #[allow(deprecated)]
                        algos.insert(HashAlgorithm::MD5);
                    }

                    // Now create an OPS packet for every algorithm.
                    let count = algos.len();
                    for (i, &algo) in algos.iter().enumerate() {
                        let mut ops = OnePassSig3::new(SignatureType::Text);
                        ops.set_hash_algo(algo);
                        ops.set_last(i + 1 == count);
                        Packet::from(ops).serialize(&mut self.decode_buffer)
                            .expect("writing to vec does not fail");
                    }

                    // We will let the caller consume the buffer.
                    // Once drained, we start decoding the message.
                    self.csft = Some(CSFTransformer::Literal);
                },

                CSFTransformer::Literal => {
                    // XXX: We should create a partial-body encoded
                    // literal packet, but for now we construct the
                    // whole packet in core.

                    let mut text = Vec::new();
                    loop {
                        let prefixed_line = self.source.read_to(b'\n')?;

                        if prefixed_line.is_empty() {
                            // Truncated?
                            break;
                        }

                        // Treat lines shorter than the prefix as
                        // empty lines.
                        let n = prefixed_line.len().min(self.prefix.len());
                        let prefix = &prefixed_line[..n];
                        let mut line = &prefixed_line[n..];

                        // Check that we see the correct prefix.
                        let l = common_prefix(&self.prefix, prefix);
                        let full_prefix = l == self.prefix.len();
                        if ! (full_prefix
                              // Truncation is okay if the rest of the prefix
                              // contains only whitespace.
                              || self.prefix[l..].iter().all(
                                  |c| c.is_ascii_whitespace()))
                        {
                            return Err(
                                Error::new(ErrorKind::InvalidInput,
                                           "Inconsistent quoting of \
                                            armored data"));
                        }

                        let (dashes, rest) = dash_prefix(line);
                        if dashes.len() > 2 // XXX: heuristic...
                            && rest.starts_with(b"BEGIN PGP SIGNATURE")
                        {
                            // We reached the end of the signed
                            // message.  Consuming this line and break
                            // the loop.
                            let l = prefixed_line.len();
                            self.source.consume(l);
                            break;
                        }

                        // Undo the dash-escaping.
                        if line.starts_with(b"- ") {
                            line = &line[2..];
                        }

                        // Trim trailing whitespace according to
                        // Section 7.1 of RFC4880, i.e. "spaces (0x20)
                        // and tabs (0x09)".  We do this here, because
                        // we transform the CSF message into an inline
                        // signed message, which does not make a
                        // distinction between the literal text and
                        // the signed text (modulo the newline
                        // normalization).

                        // First, split off the line ending.
                        let crlf_line_end = line.ends_with(b"\r\n");
                        line = &line[..line.len().saturating_sub(
                            if crlf_line_end { 2 } else { 1 })];

                        // Now, trim whitespace off the line.
                        while Some(&b' ') == line.last()
                            || Some(&b'\t') == line.last()
                        {
                            line = &line[..line.len().saturating_sub(1)];
                        }

                        text.extend_from_slice(line);
                        if crlf_line_end {
                            text.extend_from_slice(&b"\r\n"[..]);
                        } else {
                            text.extend_from_slice(&b"\n"[..]);
                        }

                        // Finally, consume this line.
                        let l = prefixed_line.len();
                        self.source.consume(l);
                    }

                    // Now, we have the whole text.
                    let mut literal = Literal::new(DataFormat::Text);
                    literal.set_body(text);
                    Packet::from(literal).serialize(&mut self.decode_buffer)
                        .expect("writing to vec does not fail");

                    // We will let the caller consume the buffer.
                    // Once drained, we start streaming the
                    // signatures.
                    self.csft = Some(CSFTransformer::Signatures);
                },

                CSFTransformer::Signatures => {
                    // Drop transformer to revert to normal armor
                    // reader.
                    self.csft = None;

                    // Consume any headers.
                    self.read_headers()?;

                    // Then start streaming the signatures.  We call
                    // this function explicitly once, but next time
                    // the caller reads, it will shortcut to that
                    // function.
                    return self.read_armored_data(buf);
                },
            }
        }

        let amount = cmp::min(buf.len(), self.decode_buffer.len());
        buf[..amount].copy_from_slice(&self.decode_buffer[..amount]);
        crate::vec_drain_prefix(&mut self.decode_buffer, amount);
        Ok(amount)
    }

    /// The io::Read interface that the embedded generic reader uses
    /// to implement the BufferedReader protocol.
    fn do_read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if ! self.initialized {
            self.initialize()?;
        }

        if buf.is_empty() {
            // Short-circuit here.  Otherwise, we copy 0 bytes into
            // the buffer, which means we decoded 0 bytes, and we
            // wrongfully assume that we reached the end of the
            // armored block.
            return Ok(0);
        }

        if self.finalized {
            assert_eq!(self.decode_buffer.len(), 0);
            return Ok(0);
        }

        if self.csft.is_some() {
            self.read_clearsigned_message(buf)
        } else {
            self.read_armored_data(buf)
        }
    }

    /// Return the buffer.  Ensure that it contains at least `amount`
    /// bytes.
    // XXX: This is a verbatim copy of
    // buffered_reader::Generic::data_helper, the only modification is
    // that it uses the above do_read function.
    fn data_helper(&mut self, amount: usize, hard: bool, and_consume: bool)
                   -> io::Result<&[u8]> {
        tracer!(TRACE, "armor::Reader::data_helper");
        t!("amount: {}, hard: {}, and_consume: {} (cursor: {}, buffer: {:?})",
           amount, hard, and_consume,
           self.cursor,
           self.buffer.as_ref().map(|buffer| buffer.len()));

        // See if there is an error from the last invocation.
        if let Some(e) = self.error.take() {
            t!("Returning stashed error: {}", e);
            return Err(e);
        }

        if let Some(ref buffer) = self.buffer {
            // We have a buffer.  Make sure `cursor` is sane.
            assert!(self.cursor <= buffer.len());
        } else {
            // We don't have a buffer.  Make sure cursor is 0.
            assert_eq!(self.cursor, 0);
        }

        let amount_buffered
            = self.buffer.as_ref().map(|b| b.len() - self.cursor).unwrap_or(0);
        if amount > amount_buffered {
            // The caller wants more data than we have readily
            // available.  Read some more.

            let capacity : usize = cmp::max(cmp::max(
                DEFAULT_BUF_SIZE,
                2 * self.preferred_chunk_size), amount);

            let mut buffer_new = self.unused_buffer.take()
                .map(|mut v| {
                    vec_resize(&mut v, capacity);
                    v
                })
                .unwrap_or_else(|| vec![0u8; capacity]);

            let mut amount_read = 0;
            while amount_buffered + amount_read < amount {
                t!("Have {} bytes, need {} bytes",
                   amount_buffered + amount_read, amount);

                if self.eof {
                    t!("Hit EOF on the underlying reader, don't poll again.");
                    break;
                }

                match self.do_read(&mut buffer_new
                                   [amount_buffered + amount_read..]) {
                    Ok(read) => {
                        t!("Read {} bytes", read);
                        if read == 0 {
                            self.eof = true;
                            break;
                        } else {
                            amount_read += read;
                            continue;
                        }
                    },
                    Err(ref err) if err.kind() == ErrorKind::Interrupted =>
                        continue,
                    Err(err) => {
                        // Don't return yet, because we may have
                        // actually read something.
                        self.error = Some(err);
                        break;
                    },
                }
            }

            if amount_read > 0 {
                // We read something.
                if let Some(ref buffer) = self.buffer {
                    // We need to copy in the old data.
                    buffer_new[0..amount_buffered]
                        .copy_from_slice(
                            &buffer[self.cursor..self.cursor + amount_buffered]);
                }

                vec_truncate(&mut buffer_new, amount_buffered + amount_read);

                self.unused_buffer = self.buffer.take();
                self.buffer = Some(buffer_new);
                self.cursor = 0;
            }
        }

        let amount_buffered
            = self.buffer.as_ref().map(|b| b.len() - self.cursor).unwrap_or(0);

        if self.error.is_some() {
            t!("Encountered an error: {}", self.error.as_ref().unwrap());
            // An error occurred.  If we have enough data to fulfill
            // the caller's request, then don't return the error.
            if hard && amount > amount_buffered {
                t!("Not enough data to fulfill request, returning error");
                return Err(self.error.take().unwrap());
            }
            if !hard && amount_buffered == 0 {
                t!("No data data buffered, returning error");
                return Err(self.error.take().unwrap());
            }
        }

        if hard && amount_buffered < amount {
            t!("Unexpected EOF");
            Err(Error::new(ErrorKind::UnexpectedEof, "EOF"))
        } else if amount == 0 || amount_buffered == 0 {
            t!("Returning zero-length slice");
            Ok(&b""[..])
        } else {
            let buffer = self.buffer.as_ref().unwrap();
            if and_consume {
                let amount_consumed = cmp::min(amount_buffered, amount);
                self.cursor += amount_consumed;
                assert!(self.cursor <= buffer.len());
                t!("Consuming {} bytes, returning {} bytes",
                   amount_consumed,
                   buffer[self.cursor-amount_consumed..].len());
                Ok(&buffer[self.cursor-amount_consumed..])
            } else {
                t!("Returning {} bytes",
                   buffer[self.cursor..].len());
                Ok(&buffer[self.cursor..])
            }
        }
    }
}

impl io::Read for Reader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        buffered_reader::buffered_reader_generic_read_impl(self, buf)
    }
}

impl BufferedReader<Cookie> for Reader<'_> {
    fn buffer(&self) -> &[u8] {
        if let Some(ref buffer) = self.buffer {
            &buffer[self.cursor..]
        } else {
            &b""[..]
        }
    }

    fn data(&mut self, amount: usize) -> Result<&[u8]> {
        self.data_helper(amount, false, false)
    }

    fn data_hard(&mut self, amount: usize) -> Result<&[u8]> {
        self.data_helper(amount, true, false)
    }

    fn consume(&mut self, amount: usize) -> &[u8] {
        // println!("Generic.consume({}) \
        //           (cursor: {}, buffer: {:?})",
        //          amount, self.cursor,
        //          if let Some(ref buffer) = self.buffer { Some(buffer.len()) }
        //          else { None });

        // The caller can't consume more than is buffered!
        if let Some(ref buffer) = self.buffer {
            assert!(self.cursor <= buffer.len());
            assert!(amount <= buffer.len() - self.cursor,
                    "buffer contains just {} bytes, but you are trying to \
                    consume {} bytes.  Did you forget to call data()?",
                    buffer.len() - self.cursor, amount);

            self.cursor += amount;
            return &self.buffer.as_ref().unwrap()[self.cursor - amount..];
        } else {
            assert_eq!(amount, 0);
            &b""[..]
        }
    }

    fn data_consume(&mut self, amount: usize) -> Result<&[u8]> {
        self.data_helper(amount, false, true)
    }

    fn data_consume_hard(&mut self, amount: usize) -> Result<&[u8]> {
        self.data_helper(amount, true, true)
    }

    fn get_mut(&mut self) -> Option<&mut dyn BufferedReader<Cookie>> {
        Some(&mut self.source)
    }

    fn get_ref(&self) -> Option<&dyn BufferedReader<Cookie>> {
        Some(&self.source)
    }

    fn into_inner<'b>(self: Box<Self>)
                      -> Option<Box<dyn BufferedReader<Cookie> + 'b>>
        where Self: 'b {
        Some(self.source)
    }

    fn cookie_set(&mut self, cookie: Cookie) -> Cookie {
        std::mem::replace(&mut self.cookie, cookie)
    }

    fn cookie_ref(&self) -> &Cookie {
        &self.cookie
    }

    fn cookie_mut(&mut self) -> &mut Cookie {
        &mut self.cookie
    }
}

/// Returns all character from Unicode's "Dash Punctuation" category.
fn dashes() -> impl Iterator<Item = char> {
    ['\u{002D}', // - (Hyphen-Minus)
     '\u{058A}', // ֊ (Armenian Hyphen)
     '\u{05BE}', // ־ (Hebrew Punctuation Maqaf)
     '\u{1400}', // ᐀ (Canadian Syllabics Hyphen)
     '\u{1806}', // ᠆ (Mongolian Todo Soft Hyphen)
     '\u{2010}', // ‐ (Hyphen)
     '\u{2011}', // ‑ (Non-Breaking Hyphen)
     '\u{2012}', // ‒ (Figure Dash)
     '\u{2013}', // – (En Dash)
     '\u{2014}', // — (Em Dash)
     '\u{2015}', // ― (Horizontal Bar)
     '\u{2E17}', // ⸗ (Double Oblique Hyphen)
     '\u{2E1A}', // ⸚ (Hyphen with Diaeresis)
     '\u{2E3A}', // ⸺ (Two-Em Dash)
     '\u{2E3B}', // ⸻ (Three-Em Dash)
     '\u{2E40}', // ⹀ (Double Hyphen)
     '\u{301C}', // 〜 (Wave Dash)
     '\u{3030}', // 〰 (Wavy Dash)
     '\u{30A0}', // ゠ (Katakana-Hiragana Double Hyphen)
     '\u{FE31}', // ︱ (Presentation Form For Vertical Em Dash)
     '\u{FE32}', // ︲ (Presentation Form For Vertical En Dash)
     '\u{FE58}', // ﹘ (Small Em Dash)
     '\u{FE63}', // ﹣ (Small Hyphen-Minus)
     '\u{FF0D}', // － (Fullwidth Hyphen-Minus)
    ].iter().cloned()
}

/// Splits the given slice into a prefix of dashes and the rest.
///
/// Accepts any character from Unicode's "Dash Punctuation" category.
/// Assumes that the prefix containing the dashes is ASCII or UTF-8.
fn dash_prefix(d: &[u8]) -> (&[u8], &[u8]) {
    // First, compute a valid UTF-8 prefix.
    let p = match std::str::from_utf8(d) {
        Ok(u) => u,
        Err(e) => std::str::from_utf8(&d[..e.valid_up_to()])
            .expect("valid up to this point"),
    };
    let mut prefix_len = 0;
    for c in p.chars() {
        // Keep going while we see characters from the Category "Dash
        // Punctuation".
        match c {
            '\u{002D}' // - (Hyphen-Minus)
                | '\u{058A}' // ֊ (Armenian Hyphen)
                | '\u{05BE}' // ־ (Hebrew Punctuation Maqaf)
                | '\u{1400}' // ᐀ (Canadian Syllabics Hyphen)
                | '\u{1806}' // ᠆ (Mongolian Todo Soft Hyphen)
                | '\u{2010}' // ‐ (Hyphen)
                | '\u{2011}' // ‑ (Non-Breaking Hyphen)
                | '\u{2012}' // ‒ (Figure Dash)
                | '\u{2013}' // – (En Dash)
                | '\u{2014}' // — (Em Dash)
                | '\u{2015}' // ― (Horizontal Bar)
                | '\u{2E17}' // ⸗ (Double Oblique Hyphen)
                | '\u{2E1A}' // ⸚ (Hyphen with Diaeresis)
                | '\u{2E3A}' // ⸺ (Two-Em Dash)
                | '\u{2E3B}' // ⸻ (Three-Em Dash)
                | '\u{2E40}' // ⹀ (Double Hyphen)
                | '\u{301C}' // 〜 (Wave Dash)
                | '\u{3030}' // 〰 (Wavy Dash)
                | '\u{30A0}' // ゠ (Katakana-Hiragana Double Hyphen)
                | '\u{FE31}' // ︱ (Presentation Form For Vertical Em Dash)
                | '\u{FE32}' // ︲ (Presentation Form For Vertical En Dash)
                | '\u{FE58}' // ﹘ (Small Em Dash)
                | '\u{FE63}' // ﹣ (Small Hyphen-Minus)
                | '\u{FF0D}' // － (Fullwidth Hyphen-Minus)
              => prefix_len += c.len_utf8(),
            _ => break,
        }
    }

    (&d[..prefix_len], &d[prefix_len..])
}

#[cfg(test)]
mod test {
    use std::io::{Cursor, Read, Write};
    use super::Kind;
    use super::Writer;

    macro_rules! t {
        ( $path: expr ) => {
            include_bytes!(concat!("../tests/data/armor/", $path))
        }
    }
    macro_rules! vectors {
        ( $prefix: expr, $suffix: expr ) => {
            &[t!(concat!($prefix, "-0", $suffix)),
              t!(concat!($prefix, "-1", $suffix)),
              t!(concat!($prefix, "-2", $suffix)),
              t!(concat!($prefix, "-3", $suffix)),
              t!(concat!($prefix, "-47", $suffix)),
              t!(concat!($prefix, "-48", $suffix)),
              t!(concat!($prefix, "-49", $suffix)),
              t!(concat!($prefix, "-50", $suffix)),
              t!(concat!($prefix, "-51", $suffix))]
        }
    }

    const TEST_BIN: &[&[u8]] = vectors!("test", ".bin");
    const TEST_ASC: &[&[u8]] = vectors!("test", ".asc");
    const LITERAL_BIN: &[&[u8]] = vectors!("literal", ".bin");
    const LITERAL_ASC: &[&[u8]] = vectors!("literal", ".asc");
    const LITERAL_NO_HEADER_ASC: &[&[u8]] =
        vectors!("literal", "-no-header.asc");
    const LITERAL_NO_HEADER_WITH_CHKSUM_ASC: &[&[u8]] =
        vectors!("literal", "-no-header-with-chksum.asc");
    const LITERAL_NO_NEWLINES_ASC: &[&[u8]] =
        vectors!("literal", "-no-newlines.asc");

    #[test]
    fn enarmor() {
        for (i, (bin, asc)) in TEST_BIN.iter().zip(TEST_ASC.iter()).enumerate()
        {
            eprintln!("Test {}", i);
            let mut w =
                Writer::new(Vec::new(), Kind::File).unwrap();
            w.write(&[]).unwrap();  // Avoid zero-length optimization.
            w.write_all(bin).unwrap();
            let buf = w.finalize().unwrap();
            assert_eq!(String::from_utf8_lossy(&buf),
                       String::from_utf8_lossy(asc));
        }
    }

    #[test]
    fn enarmor_bytewise() {
        for (bin, asc) in TEST_BIN.iter().zip(TEST_ASC.iter()) {
            let mut w = Writer::new(Vec::new(), Kind::File).unwrap();
            w.write(&[]).unwrap();  // Avoid zero-length optimization.
            for b in bin.iter() {
                w.write(&[*b]).unwrap();
            }
            let buf = w.finalize().unwrap();
            assert_eq!(String::from_utf8_lossy(&buf),
                       String::from_utf8_lossy(asc));
        }
    }

    #[test]
    fn drop_writer() {
        // No ASCII frame shall be emitted if the writer is dropped
        // unused.
        assert!(Writer::new(Vec::new(), Kind::File).unwrap()
                .finalize().unwrap().is_empty());

        // However, if the user insists, we will encode a zero-byte
        // string.
        let mut w = Writer::new(Vec::new(), Kind::File).unwrap();
        w.write(&[]).unwrap();
        let buf = w.finalize().unwrap();
        assert_eq!(
            &buf[..],
            &b"-----BEGIN PGP ARMORED FILE-----\n\
               \n\
               =twTO\n\
               -----END PGP ARMORED FILE-----\n"[..]);
    }

    use super::{Reader, ReaderMode};

    #[test]
    fn dearmor_robust() {
        for (i, reference) in LITERAL_BIN.iter().enumerate() {
            for test in &[LITERAL_ASC[i],
                          LITERAL_NO_HEADER_WITH_CHKSUM_ASC[i],
                          LITERAL_NO_HEADER_ASC[i],
                          LITERAL_NO_NEWLINES_ASC[i]] {
                let mut r = Reader::from_reader(Cursor::new(test),
                                        ReaderMode::VeryTolerant);
                let mut dearmored = Vec::<u8>::new();
                r.read_to_end(&mut dearmored).unwrap();

                assert_eq!(&dearmored, reference);
            }
        }
    }

    #[test]
    fn dearmor_binary() {
        for bin in TEST_BIN.iter() {
            let mut r = Reader::from_reader(
                Cursor::new(bin), ReaderMode::Tolerant(Some(Kind::Message)));
            let mut buf = [0; 5];
            let e = r.read(&mut buf);
            assert!(e.is_err());
        }
    }

    #[test]
    fn dearmor_wrong_kind() {
        let mut r = Reader::from_reader(
            Cursor::new(&include_bytes!("../tests/data/armor/test-0.asc")[..]),
            ReaderMode::Tolerant(Some(Kind::Message)));
        let mut buf = [0; 5];
        let e = r.read(&mut buf);
        assert!(e.is_err());
    }

    #[test]
    fn dearmor_wrong_crc() {
        let mut r = Reader::from_reader(
            Cursor::new(
                &include_bytes!("../tests/data/armor/test-0.bad-crc.asc")[..]),
            ReaderMode::Tolerant(Some(Kind::File)));
        let mut buf = [0; 5];
        let e = r.read(&mut buf);
        // Quoting RFC4880++:
        //
        // > An implementation MUST NOT reject an OpenPGP object when
        // > the CRC24 footer is present, missing, malformed, or
        // > disagrees with the computed CRC24 sum.
        assert!(e.is_ok());
    }

    #[test]
    fn dearmor_wrong_footer() {
        let mut r = Reader::from_reader(
            Cursor::new(
                &include_bytes!("../tests/data/armor/test-2.bad-footer.asc")[..]
            ),
            ReaderMode::Tolerant(Some(Kind::File)));
        let mut read = 0;
        loop {
            let mut buf = [0; 5];
            match r.read(&mut buf) {
                Ok(0) => panic!("Reached EOF, but expected an error!"),
                Ok(r) => read += r,
                Err(_) => break,
            }
        }
        assert!(read <= 2);
    }

    #[test]
    fn dearmor_no_crc() {
        let mut r = Reader::from_reader(
            Cursor::new(
                &include_bytes!("../tests/data/armor/test-1.no-crc.asc")[..]),
            ReaderMode::Tolerant(Some(Kind::File)));
        let mut buf = [0; 5];
        let e = r.read(&mut buf);
        assert!(e.unwrap() == 1 && buf[0] == 0xde);
    }

    #[test]
    fn dearmor_with_header() {
        let mut r = Reader::from_reader(
            Cursor::new(
                &include_bytes!("../tests/data/armor/test-3.with-headers.asc")[..]
            ),
            ReaderMode::Tolerant(Some(Kind::File)));
        assert_eq!(r.headers().unwrap(),
                   &[("Comment".into(), "Some Header".into()),
                     ("Comment".into(), "Another one".into())]);
        let mut buf = [0; 5];
        let e = r.read(&mut buf);
        assert!(e.is_ok());
        assert_eq!(e.unwrap(), 3);
        assert_eq!(&buf[..3], TEST_BIN[3]);
    }

    #[test]
    fn dearmor_any() {
        let mut r = Reader::from_reader(
            Cursor::new(
                &include_bytes!("../tests/data/armor/test-3.with-headers.asc")[..]
            ),
            ReaderMode::VeryTolerant);
        let mut buf = [0; 5];
        let e = r.read(&mut buf);
        assert_eq!(r.kind(), Some(Kind::File));
        assert!(e.is_ok());
        assert_eq!(e.unwrap(), 3);
        assert_eq!(&buf[..3], TEST_BIN[3]);
    }

    #[test]
    fn dearmor_with_garbage() {
        let armored =
            include_bytes!("../tests/data/armor/test-3.with-headers.asc");
        // Slap some garbage in front and make sure it still reads ok.
        let mut b: Vec<u8> = "Some\ngarbage\nlines\n\t\r  ".into();
        b.extend_from_slice(armored);
        let mut r = Reader::from_reader(Cursor::new(b), ReaderMode::VeryTolerant);
        let mut buf = [0; 5];
        let e = r.read(&mut buf);
        assert_eq!(r.kind(), Some(Kind::File));
        assert!(e.is_ok());
        assert_eq!(e.unwrap(), 3);
        assert_eq!(&buf[..3], TEST_BIN[3]);

        // Again, but this time add a non-whitespace character in the
        // line of the header.
        let mut b: Vec<u8> = "Some\ngarbage\nlines\n\t.\r  ".into();
        b.extend_from_slice(armored);
        let mut r = Reader::from_reader(Cursor::new(b), ReaderMode::VeryTolerant);
        let mut buf = [0; 5];
        let e = r.read(&mut buf);
        assert!(e.is_err());
    }

    #[test]
    fn dearmor() {
        for (bin, asc) in TEST_BIN.iter().zip(TEST_ASC.iter()) {
            let mut r = Reader::from_reader(
                Cursor::new(asc),
                ReaderMode::Tolerant(Some(Kind::File)));
            let mut dearmored = Vec::<u8>::new();
            r.read_to_end(&mut dearmored).unwrap();

            assert_eq!(&dearmored, bin);
        }
    }

    #[test]
    fn dearmor_bytewise() {
        for (bin, asc) in TEST_BIN.iter().zip(TEST_ASC.iter()) {
            let r = Reader::from_reader(
                Cursor::new(asc),
                ReaderMode::Tolerant(Some(Kind::File)));
            let mut dearmored = Vec::<u8>::new();
            for c in r.bytes() {
                dearmored.push(c.unwrap());
            }

            assert_eq!(&dearmored, bin);
        }
    }

    #[test]
    fn dearmor_yuge() {
        let yuge_key = crate::tests::key("yuge-key-so-yuge-the-yugest.asc");
        let mut r = Reader::from_reader(Cursor::new(yuge_key),
                                ReaderMode::VeryTolerant);
        let mut dearmored = Vec::<u8>::new();
        r.read_to_end(&mut dearmored).unwrap();

        let r = Reader::from_reader(Cursor::new(yuge_key),
                            ReaderMode::VeryTolerant);
        let mut dearmored = Vec::<u8>::new();
        for c in r.bytes() {
            dearmored.push(c.unwrap());
        }
    }

    #[test]
    fn dearmor_quoted() {
        let mut r = Reader::from_reader(
            Cursor::new(
                &include_bytes!("../tests/data/armor/test-3.with-headers-quoted.asc")[..]
            ),
            ReaderMode::VeryTolerant);
        let mut buf = [0; 5];
        let e = r.read(&mut buf);
        assert_eq!(r.kind(), Some(Kind::File));
        assert!(e.is_ok());
        assert_eq!(e.unwrap(), 3);
        assert_eq!(&buf[..3], TEST_BIN[3]);
    }

    #[test]
    fn dearmor_quoted_stripped() {
        let mut r = Reader::from_reader(
            Cursor::new(
                &include_bytes!("../tests/data/armor/test-3.with-headers-quoted-stripped.asc")[..]
            ),
            ReaderMode::VeryTolerant);
        let mut buf = [0; 5];
        let e = r.read(&mut buf);
        assert_eq!(r.kind(), Some(Kind::File));
        assert!(e.is_ok());
        assert_eq!(e.unwrap(), 3);
        assert_eq!(&buf[..3], TEST_BIN[3]);
    }

    #[test]
    fn dearmor_quoted_a_lot() {
        let mut r = Reader::from_reader(
            Cursor::new(
                &include_bytes!("../tests/data/armor/test-3.with-headers-quoted-a-lot.asc")[..]
            ),
            ReaderMode::VeryTolerant);
        let mut buf = [0; 5];
        let e = r.read(&mut buf);
        assert_eq!(r.kind(), Some(Kind::File));
        assert!(e.is_ok());
        assert_eq!(e.unwrap(), 3);
        assert_eq!(&buf[..3], TEST_BIN[3]);
    }

    #[test]
    fn dearmor_quoted_badly() {
        let mut r = Reader::from_reader(
            Cursor::new(
                &include_bytes!("../tests/data/armor/test-3.with-headers-quoted-badly.asc")[..]
            ),
            ReaderMode::VeryTolerant);
        let mut buf = [0; 5];
        let e = r.read(&mut buf);
        assert!(e.is_err());
    }

    quickcheck! {
        fn roundtrip(kind: Kind, payload: Vec<u8>) -> bool {
            if payload.is_empty() {
                // Empty payloads do not emit an armor framing unless
                // one does an explicit empty write (and .write_all()
                // does not).
                return true;
            }

            let mut w = Writer::new(Vec::new(), kind).unwrap();
            w.write_all(&payload).unwrap();
            let encoded = w.finalize().unwrap();

            let mut recovered = Vec::new();
            Reader::from_reader(Cursor::new(&encoded),
                        ReaderMode::Tolerant(Some(kind)))
                .read_to_end(&mut recovered)
                .unwrap();

            let mut recovered_any = Vec::new();
            Reader::from_reader(Cursor::new(&encoded), ReaderMode::VeryTolerant)
                .read_to_end(&mut recovered_any)
                .unwrap();

            payload == recovered && payload == recovered_any
        }
    }

    /// Tests issue #404, zero-sized reads break reader.
    ///
    /// See: https://gitlab.com/sequoia-pgp/sequoia/-/issues/404
    #[test]
    fn zero_sized_read() {
        let mut r = Reader::from_bytes(crate::tests::file("armor/test-1.asc"),
                                       None);
        let mut buf = Vec::new();
        r.read(&mut buf).unwrap();
        r.read(&mut buf).unwrap();
    }

    /// Crash in armor parser due to indexing not aligned with UTF-8
    /// characters.
    ///
    /// See: https://gitlab.com/sequoia-pgp/sequoia/-/issues/515
    #[test]
    fn issue_515() {
        let data = [63, 9, 45, 10, 45, 10, 45, 45, 45, 45, 45, 66, 69,
                    71, 73, 78, 32, 80, 71, 80, 32, 77, 69, 83, 83,
                    65, 71, 69, 45, 45, 45, 45, 45, 45, 152, 152, 152,
                    152, 152, 152, 255, 29, 152, 152, 152, 152, 152,
                    152, 152, 152, 152, 152, 10, 91, 45, 10, 45, 14,
                    0, 36, 0, 0, 30, 122, 4, 2, 204, 152];

        let mut reader = Reader::from_bytes(&data[..], None);
        let mut buf = Vec::new();
        // `data` is malformed, expect an error.
        reader.read_to_end(&mut buf).unwrap_err();
    }

    /// Crash in armor parser due to improper use of the buffered
    /// reader protocol when consuming quoting prefix.
    ///
    /// See: https://gitlab.com/sequoia-pgp/sequoia/-/issues/516
    #[test]
    fn issue_516() {
        let data = [
            144, 32, 19, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 125, 13, 125,
            125, 93, 125, 125, 93, 125, 13, 13, 125, 125, 45, 45, 45,
            45, 45, 66, 69, 71, 73, 78, 32, 80, 71, 80, 32, 77, 69,
            83, 83, 65, 71, 69, 45, 45, 45, 45, 45, 125, 13, 125,
            125, 93, 125, 125, 93, 125, 13, 13, 125, 125, 45, 0, 0,
            0, 0, 0, 0, 0, 0, 125, 205, 21, 1, 21, 21, 21, 1, 1, 1,
            1, 21, 149, 21, 21, 21, 21, 32, 4, 141, 141, 141, 141,
            202, 74, 11, 125, 8, 21, 50, 50, 194, 48, 147, 93, 174,
            23, 23, 23, 23, 23, 23, 147, 147, 147, 23, 23, 23, 23,
            23, 23, 48, 125, 125, 93, 125, 13, 125, 125, 125, 93,
            125, 125, 13, 13, 125, 125, 13, 13, 93, 125, 13, 125, 45,
            125, 125, 45, 45, 66, 69, 71, 73, 78, 32, 80, 71, 45, 45,
            125, 10, 45, 45, 0, 0, 10, 45, 45, 210, 10, 0, 0, 87, 0,
            0, 0, 150, 10, 0, 0, 241, 87, 45, 0, 0, 121, 121, 10, 10,
            21, 58];
        let mut reader = Reader::from_bytes(&data[..], None);
        let mut buf = Vec::new();
        // `data` is malformed, expect an error.
        reader.read_to_end(&mut buf).unwrap_err();
    }

    /// Crash in armor parser due to improper use of the buffered
    /// reader protocol when consuming quoting prefix.
    ///
    /// See: https://gitlab.com/sequoia-pgp/sequoia/-/issues/517
    #[test]
    fn issue_517() {
        let data = [13, 45, 45, 45, 45, 45, 66, 69, 71, 73, 78, 32, 80,
                    71, 80, 32, 77, 69, 83, 83, 65, 71, 69, 45, 45, 45,
                    45, 45, 10, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13,
                    13, 13, 139];
        let mut reader = Reader::from_bytes(&data[..], None);
        let mut buf = Vec::new();
        // `data` is malformed, expect an error.
        reader.read_to_end(&mut buf).unwrap_err();
    }

    #[test]
    fn common_prefix() {
        use super::common_prefix as cp;
        assert_eq!(cp("", ""), 0);
        assert_eq!(cp("a", ""), 0);
        assert_eq!(cp("", "a"), 0);
        assert_eq!(cp("a", "a"), 1);
        assert_eq!(cp("aa", "a"), 1);
        assert_eq!(cp("a", "aa"), 1);
        assert_eq!(cp("ac", "ab"), 1);
    }

    /// A certificate was mangled turning -- into n-dash, --- into
    /// m-dash.  Fun with Unicode.
    #[test]
    fn issue_610() {
        let mut buf = Vec::new();
        // First, we now accept any dash character, not only '-'.
        let mut reader = Reader::from_bytes(
            crate::tests::file("armor/test-3.unicode-dashes.asc"), None);
        reader.read_to_end(&mut buf).unwrap();

        // Second, the transformation changed the number of dashes.
        let mut reader = Reader::from_bytes(
            crate::tests::file("armor/test-3.unbalanced-dashes.asc"), None);
        reader.read_to_end(&mut buf).unwrap();

        // Third, as it is not about the dashes, we even accept none.
        let mut reader = Reader::from_bytes(
            crate::tests::file("armor/test-3.no-dashes.asc"), None);
        reader.read_to_end(&mut buf).unwrap();
    }

    /// Tests the transformation of a cleartext signed message into a
    /// signed message.
    ///
    /// This test is merely concerned with the transformation, not
    /// with the signature verification.
    #[test]
    fn cleartext_signed_message() -> crate::Result<()> {
        use crate::{
            Packet,
            parse::Parse,
            types::HashAlgorithm,
        };

        fn f<R>(clearsig: &[u8], reference: R) -> crate::Result<()>
        where R: AsRef<[u8]>
        {
            let mut reader = Reader::from_buffered_reader_csft(
                Box::new(buffered_reader::Memory::with_cookie(
                    clearsig, Default::default())),
                None, Default::default(), true);

            let mut buf = Vec::new();
            reader.read_to_end(&mut buf)?;

            let message = crate::Message::from_bytes(&buf)?;
            assert_eq!(message.children().count(), 3);

            // First, an one-pass-signature packet.
            if let Some(Packet::OnePassSig(ops)) = message.path_ref(&[0]) {
                assert_eq!(ops.hash_algo(), HashAlgorithm::SHA256);
            } else {
                panic!("expected an OPS packet");
            }

            // A literal packet.
            assert_eq!(message.body().unwrap().body(), reference.as_ref());

            // And, the signature.
            if let Some(Packet::Signature(sig)) = message.path_ref(&[2]) {
                assert_eq!(sig.hash_algo(), HashAlgorithm::SHA256);
            } else {
                panic!("expected an signature packet");
            }

            // If we parse it without enabling the CSF transformation,
            // we should only find the signature.
            let mut reader = Reader::from_buffered_reader_csft(
                Box::new(buffered_reader::Memory::with_cookie(
                    clearsig, Default::default())),
                None, Default::default(), false);

            let mut buf = Vec::new();
            reader.read_to_end(&mut buf)?;

            let pp = crate::PacketPile::from_bytes(&buf)?;
            assert_eq!(pp.children().count(), 1);

            // The signature.
            if let Some(Packet::Signature(sig)) = pp.path_ref(&[0]) {
                assert_eq!(sig.hash_algo(), HashAlgorithm::SHA256);
            } else {
                panic!("expected an signature packet");
            }
            Ok(())
        }

        f(crate::tests::message("a-problematic-poem.txt.cleartext.sig"),
          crate::tests::message("a-problematic-poem.txt"))?;
        f(crate::tests::message("a-cypherpunks-manifesto.txt.cleartext.sig"),
          {
              // The transformation process trims trailing whitespace,
              // and the manifesto has a trailing whitespace right at
              // the end.
              let mut manifesto = crate::tests::manifesto().to_vec();
              let ws_at = manifesto.len() - 2;
              let ws = manifesto.remove(ws_at);
              assert_eq!(ws, b' ');
              manifesto
          })?;
        Ok(())
    }
}
