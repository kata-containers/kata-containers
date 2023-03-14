//! A `BufferedReader` is a super-powered `Read`er.
//!
//! Like the [`BufRead`] trait, the `BufferedReader` trait has an
//! internal buffer that is directly exposed to the user.  This design
//! enables two performance optimizations.  First, the use of an
//! internal buffer amortizes system calls.  Second, exposing the
//! internal buffer allows the user to work with data in place, which
//! avoids another copy.
//!
//! The [`BufRead`] trait, however, has a significant limitation for
//! parsers: the user of a [`BufRead`] object can't control the amount
//! of buffering.  This is essential for being able to conveniently
//! work with data in place, and being able to lookahead without
//! consuming data.  The result is that either the sizing has to be
//! handled by the instantiator of the [`BufRead`] object---assuming
//! the [`BufRead`] object provides such a mechanism---which is a
//! layering violation, or the parser has to fallback to buffering if
//! the internal buffer is too small, which eliminates most of the
//! advantages of the [`BufRead`] abstraction.  The `BufferedReader`
//! trait addresses this shortcoming by allowing the user to control
//! the size of the internal buffer.
//!
//! The `BufferedReader` trait also has some functionality,
//! specifically, a generic interface to work with a stack of
//! `BufferedReader` objects, that simplifies using multiple parsers
//! simultaneously.  This is helpful when one parser deals with
//! framing (e.g., something like [HTTP's chunk transfer encoding]),
//! and another decodes the actual objects.  It is also useful when
//! objects are nested.
//!
//! # Details
//!
//! Because the [`BufRead`] trait doesn't provide a mechanism for the
//! user to size the internal buffer, a parser can't generally be sure
//! that the internal buffer will be large enough to allow it to work
//! with all data in place.
//!
//! Using the standard [`BufRead`] implementation, [`BufReader`], the
//! instantiator can set the size of the internal buffer at creation
//! time.  Unfortunately, this mechanism is ugly, and not always
//! adequate.  First, the parser is typically not the instantiator.
//! Thus, the instantiator needs to know about the implementation
//! details of all of the parsers, which turns an implementation
//! detail into a cross-cutting concern.  Second, when working with
//! dynamically sized data, the maximum amount of the data that needs
//! to be worked with in place may not be known apriori, or the
//! maximum amount may be significantly larger than the typical
//! amount.  This leads to poorly sized buffers.
//!
//! Alternatively, the code that uses, but does not instantiate a
//! [`BufRead`] object, can be changed to stream the data, or to
//! fallback to reading the data into a local buffer if the internal
//! buffer is too small.  Both of these approaches increase code
//! complexity, and the latter approach is contrary to the
//! [`BufRead`]'s goal of reducing unnecessary copying.
//!
//! The `BufferedReader` trait solves this problem by allowing the
//! user to dynamically (i.e., at read time, not open time) ensure
//! that the internal buffer has a certain amount of data.
//!
//! The ability to control the size of the internal buffer is also
//! essential to straightforward support for speculative lookahead.
//! The reason that speculative lookahead with a [`BufRead`] object is
//! difficult is that speculative lookahead is /speculative/, i.e., if
//! the parser backtracks, the data that was read must not be
//! consumed.  Using a [`BufRead`] object, this is not possible if the
//! amount of lookahead is larger than the internal buffer.  That is,
//! if the amount of lookahead data is larger than the [`BufRead`]'s
//! internal buffer, the parser first has to `BufRead::consume`() some
//! data to be able to examine more data.  But, if the parser then
//! decides to backtrack, it has no way to return the unused data to
//! the [`BufRead`] object.  This forces the parser to manage a buffer
//! of read, but unconsumed data, which significantly complicates the
//! code.
//!
//! The `BufferedReader` trait also simplifies working with a stack of
//! `BufferedReader`s in two ways.  First, the `BufferedReader` trait
//! provides *generic* methods to access the underlying
//! `BufferedReader`.  Thus, even when dealing with a trait object, it
//! is still possible to recover the underlying `BufferedReader`.
//! Second, the `BufferedReader` provides a mechanism to associate
//! generic state with each `BufferedReader` via a cookie.  Although
//! it is possible to realize this functionality using a custom trait
//! that extends the `BufferedReader` trait and wraps existing
//! `BufferedReader` implementations, this approach eliminates a lot
//! of error-prone, boilerplate code.
//!
//! # Examples
//!
//! The following examples show not only how to use a
//! `BufferedReader`, but also better illustrate the aforementioned
//! limitations of a [`BufRead`]er.
//!
//! Consider a file consisting of a sequence of objects, which are
//! laid out as follows.  Each object has a two byte header that
//! indicates the object's size in bytes.  The object immediately
//! follows the header.  Thus, if we had two objects: "foobar" and
//! "xyzzy", in that order, the file would look like this:
//!
//! ```text
//! 0 6 f o o b a r 0 5 x y z z y
//! ```
//!
//! Here's how we might parse this type of file using a
//! `BufferedReader`:
//!
//! ```
//! use buffered_reader;
//! use buffered_reader::BufferedReader;
//!
//! fn parse_object(content: &[u8]) {
//!     // Parse the object.
//!     # let _ = content;
//! }
//!
//! # f(); fn f() -> Result<(), std::io::Error> {
//! # const FILENAME : &str = "/dev/null";
//! let mut br = buffered_reader::File::open(FILENAME)?;
//!
//! // While we haven't reached EOF (i.e., we can read at
//! // least one byte).
//! while br.data(1)?.len() > 0 {
//!     // Get the object's length.
//!     let len = br.read_be_u16()? as usize;
//!     // Get the object's content.
//!     let content = br.data_consume_hard(len)?;
//!
//!     // Parse the actual object using a real parser.  Recall:
//!     // `data_hard`() may return more than the requested amount (but
//!     // it will never return less).
//!     parse_object(&content[..len]);
//! }
//! # Ok(()) }
//! ```
//!
//! Note that `content` is actually a pointer to the
//! `BufferedReader`'s internal buffer.  Thus, getting some data
//! doesn't require copying the data into a local buffer, which is
//! often discarded immediately after the data is parsed.
//!
//! Further, `data`() (and the other related functions) are guaranteed
//! to return at least the requested amount of data.  There are two
//! exceptions: if an error occurs, or the end of the file is reached.
//! Thus, only the cases that actually need to be handled by the user
//! are actually exposed; there is no need to call something like
//! `read`() in a loop to ensure the whole object is available.
//!
//! Because reading is separate from consuming data, it is possible to
//! get a chunk of data, inspect it, and then consume only what is
//! needed.  As mentioned above, this is only possible with a
//! [`BufRead`] object if the internal buffer happens to be large
//! enough.  Using a `BufferedReader`, this is always possible,
//! assuming the data fits in memory.
//!
//! In our example, we actually have two parsers: one that deals with
//! the framing, and one for the actual objects.  The above code
//! buffers the objects in their entirety, and then passes a slice
//! containing the object to the object parser.  If the object parser
//! also worked with a `BufferedReader` object, then less buffering
//! will usually be needed, and the two parsers could run
//! simultaneously.  This is particularly useful when the framing is
//! more complicated like [HTTP's chunk transfer encoding].  Then,
//! when the object parser reads data, the frame parser is invoked
//! lazily.  This is done by implementing the `BufferedReader` trait
//! for the framing parser, and stacking the `BufferedReader`s.
//!
//! For our next example, we rewrite the previous code assuming that
//! the object parser reads from a `BufferedReader` object.  Since the
//! framing parser is really just a limit on the object's size, we
//! don't need to implement a special `BufferedReader`, but can use a
//! `Limitor` to impose an upper limit on the amount
//! that it can read.  After the object parser has finished, we drain
//! the object reader.  This pattern is particularly helpful when
//! individual objects that contain errors should be skipped.
//!
//! ```
//! use buffered_reader;
//! use buffered_reader::BufferedReader;
//!
//! fn parse_object<R: BufferedReader<()>>(br: &mut R) {
//!     // Parse the object.
//!     # let _ = br;
//! }
//!
//! # f(); fn f() -> Result<(), std::io::Error> {
//! # const FILENAME : &str = "/dev/null";
//! let mut br : Box<BufferedReader<()>>
//!     = Box::new(buffered_reader::File::open(FILENAME)?);
//!
//! // While we haven't reached EOF (i.e., we can read at
//! // least one byte).
//! while br.data(1)?.len() > 0 {
//!     // Get the object's length.
//!     let len = br.read_be_u16()? as u64;
//!
//!     // Set up a limit.
//!     br = Box::new(buffered_reader::Limitor::new(br, len));
//!
//!     // Parse the actual object using a real parser.
//!     parse_object(&mut br);
//!
//!     // If the parser didn't consume the whole object, e.g., due to
//!     // a parse error, drop the rest.
//!     br.drop_eof();
//!
//!     // Recover the framing parser's `BufferedReader`.
//!     br = br.into_inner().unwrap();
//! }
//! # Ok(()) }
//! ```
//!
//! Of particular note is the generic functionality for dealing with
//! stacked `BufferedReader`s: the `into_inner`() method is not bound
//! to the implementation, which is often not be available due to type
//! erasure, but is provided by the trait.
//!
//! In addition to utility `BufferedReader`s like the
//! `Limitor`, this crate also includes a few
//! general-purpose parsers, like the `Zip`
//! decompressor.
//!
//! [`BufRead`]: std::io::BufRead
//! [`BufReader`]: std::io::BufReader
//! [HTTP's chunk transfer encoding]: https://en.wikipedia.org/wiki/Chunked_transfer_encoding

#![doc(html_favicon_url = "https://docs.sequoia-pgp.org/favicon.png")]
#![doc(html_logo_url = "https://docs.sequoia-pgp.org/logo.svg")]
#![warn(missing_docs)]

use std::io;
use std::io::{Error, ErrorKind};
use std::cmp;
use std::fmt;
use std::convert::TryInto;

#[macro_use]
mod macros;

mod generic;
mod memory;
mod limitor;
mod reserve;
mod dup;
mod eof;
mod adapter;
#[cfg(feature = "compression-deflate")]
mod decompress_deflate;
#[cfg(feature = "compression-bzip2")]
mod decompress_bzip2;

pub use self::generic::Generic;
pub use self::memory::Memory;
pub use self::limitor::Limitor;
pub use self::reserve::Reserve;
pub use self::dup::Dup;
pub use self::eof::EOF;
pub use self::adapter::Adapter;
#[cfg(feature = "compression-deflate")]
pub use self::decompress_deflate::Deflate;
#[cfg(feature = "compression-deflate")]
pub use self::decompress_deflate::Zlib;
#[cfg(feature = "compression-bzip2")]
pub use self::decompress_bzip2::Bzip;

// Common error type for file operations.
mod file_error;

// These are the different File implementations.  We
// include the modules unconditionally, so that we catch bitrot early.
#[allow(dead_code)]
mod file_generic;
#[allow(dead_code)]
#[cfg(unix)]
mod file_unix;

// Then, we select the appropriate version to re-export.
#[cfg(not(unix))]
pub use self::file_generic::File;
#[cfg(unix)]
pub use self::file_unix::File;

// The default buffer size.
const DEFAULT_BUF_SIZE: usize = 8 * 1024;

// On debug builds, Vec<u8>::truncate is very, very slow.  For
// instance, running the decrypt_test_stream test takes 51 seconds on
// my (Neal's) computer using Vec<u8>::truncate and <0.1 seconds using
// `unsafe { v.set_len(len); }`.
//
// The issue is that the compiler calls drop on every element that is
// dropped, even though a u8 doesn't have a drop implementation.  The
// compiler optimizes this away at high optimization levels, but those
// levels make debugging harder.
fn vec_truncate(v: &mut Vec<u8>, len: usize) {
    if cfg!(debug_assertions) {
        if len < v.len() {
            unsafe { v.set_len(len); }
        }
    } else {
        v.truncate(len);
    }
}

/// Like `Vec<u8>::resize`, but fast in debug builds.
fn vec_resize(v: &mut Vec<u8>, new_size: usize) {
    if v.len() < new_size {
        v.resize(new_size, 0);
    } else {
        vec_truncate(v, new_size);
    }
}

/// The generic `BufferReader` interface.
pub trait BufferedReader<C> : io::Read + fmt::Debug + fmt::Display + Send + Sync
  where C: fmt::Debug + Send + Sync
{
    /// Returns a reference to the internal buffer.
    ///
    /// Note: this returns the same data as `self.data(0)`, but it
    /// does so without mutably borrowing self:
    ///
    /// ```
    /// # f(); fn f() -> Result<(), std::io::Error> {
    /// use buffered_reader;
    /// use buffered_reader::BufferedReader;
    ///
    /// let mut br = buffered_reader::Memory::new(&b"0123456789"[..]);
    ///
    /// let first = br.data(10)?.len();
    /// let second = br.buffer().len();
    /// // `buffer` must return exactly what `data` returned.
    /// assert_eq!(first, second);
    /// # Ok(()) }
    /// ```
    fn buffer(&self) -> &[u8];

    /// Ensures that the internal buffer has at least `amount` bytes
    /// of data, and returns it.
    ///
    /// If the internal buffer contains less than `amount` bytes of
    /// data, the internal buffer is first filled.
    ///
    /// The returned slice will have *at least* `amount` bytes unless
    /// EOF has been reached or an error occurs, in which case the
    /// returned slice will contain the rest of the file.
    ///
    /// Errors are returned only when the internal buffer is empty.
    ///
    /// This function does not advance the cursor.  To advance the
    /// cursor, use `consume()`.
    ///
    /// Note: If the internal buffer already contains at least
    /// `amount` bytes of data, then `BufferedReader` implementations
    /// are guaranteed to simply return the internal buffer.  As such,
    /// multiple calls to `data` for the same `amount` will return the
    /// same slice.
    ///
    /// Further, `BufferedReader` implementations are guaranteed to
    /// not shrink the internal buffer.  Thus, once some data has been
    /// returned, it will always be returned until it is consumed.
    /// As such, the following must hold:
    ///
    /// If `BufferedReader` receives `EINTR` when `read`ing, it will
    /// automatically retry reading.
    ///
    /// ```
    /// # f(); fn f() -> Result<(), std::io::Error> {
    /// use buffered_reader;
    /// use buffered_reader::BufferedReader;
    ///
    /// let mut br = buffered_reader::Memory::new(&b"0123456789"[..]);
    ///
    /// let first = br.data(10)?.len();
    /// let second = br.data(5)?.len();
    /// // Even though less data is requested, the second call must
    /// // return the same slice as the first call.
    /// assert_eq!(first, second);
    /// # Ok(()) }
    /// ```
    fn data(&mut self, amount: usize) -> Result<&[u8], io::Error>;

    /// Like `data()`, but returns an error if there is not at least
    /// `amount` bytes available.
    ///
    /// `data_hard()` is a variant of `data()` that returns at least
    /// `amount` bytes of data or an error.  Thus, unlike `data()`,
    /// which will return less than `amount` bytes of data if EOF is
    /// encountered, `data_hard()` returns an error, specifically,
    /// `io::ErrorKind::UnexpectedEof`.
    ///
    /// # Examples
    ///
    /// ```
    /// # f(); fn f() -> Result<(), std::io::Error> {
    /// use buffered_reader;
    /// use buffered_reader::BufferedReader;
    ///
    /// let mut br = buffered_reader::Memory::new(&b"0123456789"[..]);
    ///
    /// // Trying to read more than there is available results in an error.
    /// assert!(br.data_hard(20).is_err());
    /// // Whereas with data(), everything through EOF is returned.
    /// assert_eq!(br.data(20)?.len(), 10);
    /// # Ok(()) }
    /// ```
    fn data_hard(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        let result = self.data(amount);
        if let Ok(buffer) = result {
            if buffer.len() < amount {
                return Err(Error::new(ErrorKind::UnexpectedEof,
                                      "unexpected EOF"));
            }
        }
        result
    }

    /// Returns all of the data until EOF.  Like `data()`, this does not
    /// actually consume the data that is read.
    ///
    /// In general, you shouldn't use this function as it can cause an
    /// enormous amount of buffering.  But, if you know that the
    /// amount of data is limited, this is acceptable.
    ///
    /// # Examples
    ///
    /// ```
    /// # f(); fn f() -> Result<(), std::io::Error> {
    /// use buffered_reader;
    /// use buffered_reader::BufferedReader;
    ///
    /// const AMOUNT : usize = 100 * 1024 * 1024;
    /// let buffer = vec![0u8; AMOUNT];
    /// let mut br = buffered_reader::Generic::new(&buffer[..], None);
    ///
    /// // Normally, only a small amount will be buffered.
    /// assert!(br.data(10)?.len() <= AMOUNT);
    ///
    /// // `data_eof` buffers everything.
    /// assert_eq!(br.data_eof()?.len(), AMOUNT);
    ///
    /// // Now that everything is buffered, buffer(), data(), and
    /// // data_hard() will also return everything.
    /// assert_eq!(br.buffer().len(), AMOUNT);
    /// assert_eq!(br.data(10)?.len(), AMOUNT);
    /// assert_eq!(br.data_hard(10)?.len(), AMOUNT);
    /// # Ok(()) }
    /// ```
    fn data_eof(&mut self) -> Result<&[u8], io::Error> {
        // Don't just read std::usize::MAX bytes at once.  The
        // implementation might try to actually allocate a buffer that
        // large!  Instead, try with increasingly larger buffers until
        // the read is (strictly) shorter than the specified size.
        let mut s = DEFAULT_BUF_SIZE;
        // We will break the loop eventually, because self.data(s)
        // must return a slice shorter than std::usize::MAX.
        loop {
            match self.data(s) {
                Ok(buffer) => {
                    if buffer.len() < s {
                        // We really want to do
                        //
                        //   return Ok(buffer);
                        //
                        // But, the borrower checker won't let us:
                        //
                        //  error[E0499]: cannot borrow `*self` as
                        //  mutable more than once at a time.
                        //
                        // Instead, we break out of the loop, and then
                        // call self.buffer().
                        s = buffer.len();
                        break;
                    } else {
                        s *= 2;
                    }
                }
                Err(err) =>
                    return Err(err),
            }
        }

        let buffer = self.buffer();
        assert_eq!(buffer.len(), s);
        Ok(buffer)
    }

    /// Consumes some of the data.
    ///
    /// This advances the internal cursor by `amount`.  It is an error
    /// to call this function to consume data that hasn't been
    /// returned by `data()` or a related function.
    ///
    /// Note: It is safe to call this function to consume more data
    /// than requested in a previous call to `data()`, but only if
    /// `data()` also returned that data.
    ///
    /// This function returns the internal buffer *including* the
    /// consumed data.  Thus, the `BufferedReader` implementation must
    /// continue to buffer the consumed data until the reference goes
    /// out of scope.
    ///
    /// # Examples
    ///
    /// ```
    /// # f(); fn f() -> Result<(), std::io::Error> {
    /// use buffered_reader;
    /// use buffered_reader::BufferedReader;
    ///
    /// const AMOUNT : usize = 100 * 1024 * 1024;
    /// let buffer = vec![0u8; AMOUNT];
    /// let mut br = buffered_reader::Generic::new(&buffer[..], None);
    ///
    /// let amount = {
    ///     // We want at least 1024 bytes, but we'll be happy with
    ///     // more or less.
    ///     let buffer = br.data(1024)?;
    ///     // Parse the data or something.
    ///     let used = buffer.len();
    ///     used
    /// };
    /// let buffer = br.consume(amount);
    /// # Ok(()) }
    /// ```
    fn consume(&mut self, amount: usize) -> &[u8];

    /// A convenience function that combines `data()` and `consume()`.
    ///
    /// If less than `amount` bytes are available, this function
    /// consumes what is available.
    ///
    /// Note: Due to lifetime issues, it is not possible to call
    /// `data()`, work with the returned buffer, and then call
    /// `consume()` in the same scope, because both `data()` and
    /// `consume()` take a mutable reference to the `BufferedReader`.
    /// This function makes this common pattern easier.
    ///
    /// # Examples
    ///
    /// ```
    /// # f(); fn f() -> Result<(), std::io::Error> {
    /// use buffered_reader;
    /// use buffered_reader::BufferedReader;
    ///
    /// let orig = b"0123456789";
    /// let mut br = buffered_reader::Memory::new(&orig[..]);
    ///
    /// // We need a new scope for each call to `data_consume()`, because
    /// // the `buffer` reference locks `br`.
    /// {
    ///     let buffer = br.data_consume(3)?;
    ///     assert_eq!(buffer, &orig[..buffer.len()]);
    /// }
    ///
    /// // Note that the cursor has advanced.
    /// {
    ///     let buffer = br.data_consume(3)?;
    ///     assert_eq!(buffer, &orig[3..3 + buffer.len()]);
    /// }
    ///
    /// // Like `data()`, `data_consume()` may return and consume less
    /// // than requested if there is no more data available.
    /// {
    ///     let buffer = br.data_consume(10)?;
    ///     assert_eq!(buffer, &orig[6..6 + buffer.len()]);
    /// }
    ///
    /// {
    ///     let buffer = br.data_consume(10)?;
    ///     assert_eq!(buffer.len(), 0);
    /// }
    /// # Ok(()) }
    /// ```
    fn data_consume(&mut self, amount: usize)
                    -> Result<&[u8], std::io::Error> {
        let amount = cmp::min(amount, self.data(amount)?.len());

        let buffer = self.consume(amount);
        assert!(buffer.len() >= amount);
        Ok(buffer)
    }

    /// A convenience function that effectively combines `data_hard()`
    /// and `consume()`.
    ///
    /// This function is identical to `data_consume()`, but internally
    /// uses `data_hard()` instead of `data()`.
    fn data_consume_hard(&mut self, amount: usize)
        -> Result<&[u8], io::Error>
    {
        let len = self.data_hard(amount)?.len();
        assert!(len >= amount);

        let buffer = self.consume(amount);
        assert!(buffer.len() >= amount);
        Ok(buffer)
    }

    /// Checks whether the end of the stream is reached.
    fn eof(&mut self) -> bool {
        self.data_hard(1).is_err()
    }

    /// Checks whether this reader is consummated.
    ///
    /// For most readers, this function will return true once the end
    /// of the stream is reached.  However, some readers are concerned
    /// with packet framing (e.g. the [`Limitor`]).  Those readers
    /// consider themselves consummated if the amount of data
    /// indicated by the packet frame is consumed.
    ///
    /// This allows us to detect truncation.  A packet is truncated,
    /// iff the end of the stream is reached, but the reader is not
    /// consummated.
    ///
    fn consummated(&mut self) -> bool {
        self.eof()
    }

    /// A convenience function for reading a 16-bit unsigned integer
    /// in big endian format.
    fn read_be_u16(&mut self) -> Result<u16, std::io::Error> {
        let input = self.data_consume_hard(2)?;
        // input holds at least 2 bytes, so this cannot fail.
        Ok(u16::from_be_bytes(input[..2].try_into().unwrap()))
    }

    /// A convenience function for reading a 32-bit unsigned integer
    /// in big endian format.
    fn read_be_u32(&mut self) -> Result<u32, std::io::Error> {
        let input = self.data_consume_hard(4)?;
        // input holds at least 4 bytes, so this cannot fail.
        Ok(u32::from_be_bytes(input[..4].try_into().unwrap()))
    }

    /// Reads until either `terminal` is encountered or EOF.
    ///
    /// Returns either a `&[u8]` terminating in `terminal` or the rest
    /// of the data, if EOF was encountered.
    ///
    /// Note: this function does *not* consume the data.
    ///
    /// # Examples
    ///
    /// ```
    /// # f(); fn f() -> Result<(), std::io::Error> {
    /// use buffered_reader;
    /// use buffered_reader::BufferedReader;
    ///
    /// let orig = b"0123456789";
    /// let mut br = buffered_reader::Memory::new(&orig[..]);
    ///
    /// {
    ///     let s = br.read_to(b'3')?;
    ///     assert_eq!(s, b"0123");
    /// }
    ///
    /// // `read_to()` doesn't consume the data.
    /// {
    ///     let s = br.read_to(b'5')?;
    ///     assert_eq!(s, b"012345");
    /// }
    ///
    /// // Even if there is more data in the internal buffer, only
    /// // the data through the match is returned.
    /// {
    ///     let s = br.read_to(b'1')?;
    ///     assert_eq!(s, b"01");
    /// }
    ///
    /// // If the terminal is not found, everything is returned...
    /// {
    ///     let s = br.read_to(b'A')?;
    ///     assert_eq!(s, orig);
    /// }
    ///
    /// // If we consume some data, the search starts at the cursor,
    /// // not the beginning of the file.
    /// br.consume(3);
    ///
    /// {
    ///     let s = br.read_to(b'5')?;
    ///     assert_eq!(s, b"345");
    /// }
    /// # Ok(()) }
    /// ```
    fn read_to(&mut self, terminal: u8) -> Result<&[u8], std::io::Error> {
        let mut n = 128;
        let len;

        loop {
            let data = self.data(n)?;

            if let Some(newline)
                = data.iter().position(|c| *c == terminal)
            {
                len = newline + 1;
                break;
            } else if data.len() < n {
                // EOF.
                len = data.len();
                break;
            } else {
                // Read more data.
                n = cmp::max(2 * n, data.len() + 1024);
            }
        }

        Ok(&self.buffer()[..len])
    }

    /// Discards the input until one of the bytes in terminals is
    /// encountered.
    ///
    /// The matching byte is not discarded.
    ///
    /// Returns the number of bytes discarded.
    ///
    /// The end of file is considered a match.
    ///
    /// `terminals` must be sorted.
    fn drop_until(&mut self, terminals: &[u8])
        -> Result<usize, std::io::Error>
    {
        // Make sure terminals is sorted.
        for t in terminals.windows(2) {
            assert!(t[0] <= t[1]);
        }

        let mut total = 0;
        let position = 'outer: loop {
            let len = {
                // Try self.buffer.  Only if it is empty, use
                // self.data.
                let buffer = if self.buffer().is_empty() {
                    self.data(DEFAULT_BUF_SIZE)?
                } else {
                    self.buffer()
                };

                if buffer.is_empty() {
                    break 'outer 0;
                }

                if let Some(position) = buffer.iter().position(
                    |c| terminals.binary_search(c).is_ok())
                {
                    break 'outer position;
                }

                buffer.len()
            };

            self.consume(len);
            total += len;
        };

        self.consume(position);
        Ok(total + position)
    }

    /// Discards the input until one of the bytes in `terminals` is
    /// encountered.
    ///
    /// The matching byte is also discarded.
    ///
    /// Returns the terminal byte and the number of bytes discarded.
    ///
    /// If match_eof is true, then the end of file is considered a
    /// match.  Otherwise, if the end of file is encountered, an error
    /// is returned.
    ///
    /// `terminals` must be sorted.
    fn drop_through(&mut self, terminals: &[u8], match_eof: bool)
        -> Result<(Option<u8>, usize), std::io::Error>
    {
        let dropped = self.drop_until(terminals)?;
        match self.data_consume(1) {
            Ok([]) if match_eof => Ok((None, dropped)),
            Ok([]) => Err(Error::new(ErrorKind::UnexpectedEof, "EOF")),
            Ok(rest) => Ok((Some(rest[0]), dropped + 1)),
            Err(err) => Err(err),
        }
    }

    /// Like `data_consume_hard()`, but returns the data in a
    /// caller-owned buffer.
    ///
    /// `BufferedReader` implementations may optimize this to avoid a
    /// copy by directly returning the internal buffer.
    fn steal(&mut self, amount: usize) -> Result<Vec<u8>, std::io::Error> {
        let mut data = self.data_consume_hard(amount)?;
        assert!(data.len() >= amount);
        if data.len() > amount {
            data = &data[..amount];
        }
        Ok(data.to_vec())
    }

    /// Like `steal()`, but instead of stealing a fixed number of
    /// bytes, steals all of the data until the end of file.
    fn steal_eof(&mut self) -> Result<Vec<u8>, std::io::Error> {
        let len = self.data_eof()?.len();
        let data = self.steal(len)?;
        Ok(data)
    }

    /// Like `steal_eof()`, but instead of returning the data, the
    /// data is discarded.
    ///
    /// On success, returns whether any data (i.e., at least one byte)
    /// was discarded.
    ///
    /// Note: whereas `steal_eof()` needs to buffer all of the data,
    /// this function reads the data a chunk at a time, and then
    /// discards it.  A consequence of this is that an error may occur
    /// after we have consumed some of the data.
    fn drop_eof(&mut self) -> Result<bool, std::io::Error> {
        let mut at_least_one_byte = false;
        loop {
            let n = self.data(DEFAULT_BUF_SIZE)?.len();
            at_least_one_byte |= n > 0;
            self.consume(n);
            if n < DEFAULT_BUF_SIZE {
                // EOF.
                break;
            }
        }

        Ok(at_least_one_byte)
    }

    /// A helpful debugging aid to pretty print a Buffered Reader stack.
    ///
    /// Uses the Buffered Readers' `fmt::Display` implementations.
    fn dump(&self, sink: &mut dyn std::io::Write) -> std::io::Result<()>
        where Self: std::marker::Sized
    {
        let mut i = 1;
        let mut reader: Option<&dyn BufferedReader<C>> = Some(self);
        while let Some(r) = reader {
            {
                let cookie = r.cookie_ref();
                writeln!(sink, "  {}. {}, {:?}", i, r, cookie)?;
            }
            reader = r.get_ref();
            i += 1;
        }
        Ok(())
    }

    /// Boxes the reader.
    #[allow(clippy::wrong_self_convention)]
    fn as_boxed<'a>(self) -> Box<dyn BufferedReader<C> + 'a>
        where Self: 'a + Sized
    {
        Box::new(self)
    }

    /// Returns the underlying reader, if any.
    ///
    /// To allow this to work with `BufferedReader` traits, it is
    /// necessary for `Self` to be boxed.
    ///
    /// This can lead to the following unusual code:
    ///
    /// ```text
    /// let inner = Box::new(br).into_inner();
    /// ```
    fn into_inner<'a>(self: Box<Self>) -> Option<Box<dyn BufferedReader<C> + 'a>>
        where Self: 'a;

    /// Returns a mutable reference to the inner `BufferedReader`, if
    /// any.
    ///
    /// It is a very bad idea to read any data from the inner
    /// `BufferedReader`, because this `BufferedReader` may have some
    /// data buffered.  However, this function can be useful to get
    /// the cookie.
    fn get_mut(&mut self) -> Option<&mut dyn BufferedReader<C>>;

    /// Returns a reference to the inner `BufferedReader`, if any.
    fn get_ref(&self) -> Option<&dyn BufferedReader<C>>;

    /// Sets the `BufferedReader`'s cookie and returns the old value.
    fn cookie_set(&mut self, cookie: C) -> C;

    /// Returns a reference to the `BufferedReader`'s cookie.
    fn cookie_ref(&self) -> &C;

    /// Returns a mutable reference to the `BufferedReader`'s cookie.
    fn cookie_mut(&mut self) -> &mut C;
}

/// A generic implementation of `std::io::Read::read` appropriate for
/// any `BufferedReader` implementation.
///
/// This function implements the `std::io::Read::read` method in terms
/// of the `data_consume` method.  We can't use the `io::std::Read`
/// interface, because the `BufferedReader` may have buffered some
/// data internally (in which case a read will not return the buffered
/// data, but the following data).
///
/// This implementation is generic.  When deriving a `BufferedReader`,
/// you can include the following:
///
/// ```text
/// impl<'a, T: BufferedReader> std::io::Read for XXX<'a, T> {
///     fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
///         return buffered_reader_generic_read_impl(self, buf);
///     }
/// }
/// ```
///
/// It would be nice if we could do:
///
/// ```text
/// impl <T: BufferedReader> std::io::Read for T { ... }
/// ```
///
/// but, alas, Rust doesn't like that ("error\[E0119\]: conflicting
/// implementations of trait `std::io::Read` for type `&mut _`").
pub fn buffered_reader_generic_read_impl<T: BufferedReader<C>, C: fmt::Debug + Sync + Send>
        (bio: &mut T, buf: &mut [u8]) -> Result<usize, io::Error> {
    bio
        .data_consume(buf.len())
        .map(|inner| {
            let amount = cmp::min(buf.len(), inner.len());
            buf[0..amount].copy_from_slice(&inner[0..amount]);
            amount
        })
}

/// Make a `Box<BufferedReader>` look like a BufferedReader.
impl <'a, C: fmt::Debug + Sync + Send> BufferedReader<C> for Box<dyn BufferedReader<C> + 'a> {
    fn buffer(&self) -> &[u8] {
        return self.as_ref().buffer();
    }

    fn data(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        return self.as_mut().data(amount);
    }

    fn data_hard(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        return self.as_mut().data_hard(amount);
    }

    fn data_eof(&mut self) -> Result<&[u8], io::Error> {
        return self.as_mut().data_eof();
    }

    fn consume(&mut self, amount: usize) -> &[u8] {
        return self.as_mut().consume(amount);
    }

    fn data_consume(&mut self, amount: usize)
                    -> Result<&[u8], std::io::Error> {
        return self.as_mut().data_consume(amount);
    }

    fn data_consume_hard(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        return self.as_mut().data_consume_hard(amount);
    }

    fn consummated(&mut self) -> bool {
        self.as_mut().consummated()
    }

    fn read_be_u16(&mut self) -> Result<u16, std::io::Error> {
        return self.as_mut().read_be_u16();
    }

    fn read_be_u32(&mut self) -> Result<u32, std::io::Error> {
        return self.as_mut().read_be_u32();
    }

    fn read_to(&mut self, terminal: u8) -> Result<&[u8], std::io::Error>
    {
        return self.as_mut().read_to(terminal);
    }

    fn steal(&mut self, amount: usize) -> Result<Vec<u8>, std::io::Error> {
        return self.as_mut().steal(amount);
    }

    fn steal_eof(&mut self) -> Result<Vec<u8>, std::io::Error> {
        return self.as_mut().steal_eof();
    }

    fn drop_eof(&mut self) -> Result<bool, std::io::Error> {
        return self.as_mut().drop_eof();
    }

    fn get_mut(&mut self) -> Option<&mut dyn BufferedReader<C>> {
        // Strip the outer box.
        self.as_mut().get_mut()
    }

    fn get_ref(&self) -> Option<&dyn BufferedReader<C>> {
        // Strip the outer box.
        self.as_ref().get_ref()
    }

    fn as_boxed<'b>(self) -> Box<dyn BufferedReader<C> + 'b>
        where Self: 'b
    {
        self
    }

    fn into_inner<'b>(self: Box<Self>) -> Option<Box<dyn BufferedReader<C> + 'b>>
            where Self: 'b {
        // Strip the outer box.
        (*self).into_inner()
    }

    fn cookie_set(&mut self, cookie: C) -> C {
        self.as_mut().cookie_set(cookie)
    }

    fn cookie_ref(&self) -> &C {
        self.as_ref().cookie_ref()
    }

    fn cookie_mut(&mut self) -> &mut C {
        self.as_mut().cookie_mut()
    }
}

// The file was created as follows:
//
//   for i in $(seq 0 9999); do printf "%04d\n" $i; done > buffered-reader-test.txt
#[cfg(test)]
fn buffered_reader_test_data_check<'a, T: BufferedReader<C> + 'a, C: fmt::Debug + Sync + Send>(bio: &mut T) {
    use std::str;

    for i in 0 .. 10000 {
        let consumed = {
            // Each number is 4 bytes plus a newline character.
            let d = bio.data_hard(5);
            if d.is_err() {
                println!("Error for i == {}: {:?}", i, d);
            }
            let d = d.unwrap();
            assert!(d.len() >= 5);
            assert_eq!(format!("{:04}\n", i), str::from_utf8(&d[0..5]).unwrap());

            5
        };

        bio.consume(consumed);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn buffered_reader_eof_test() {
        let data : &[u8] = include_bytes!("buffered-reader-test.txt");

        // Make sure data_eof works.
        {
            let mut bio = Memory::new(data);
            let amount = {
                bio.data_eof().unwrap().len()
            };
            bio.consume(amount);
            assert_eq!(bio.data(1).unwrap().len(), 0);
        }

        // Try it again with a limitor.
        {
            let bio = Memory::new(data);
            let mut bio2 = Limitor::new(
                bio, (data.len() / 2) as u64);
            let amount = {
                bio2.data_eof().unwrap().len()
            };
            assert_eq!(amount, data.len() / 2);
            bio2.consume(amount);
            assert_eq!(bio2.data(1).unwrap().len(), 0);
        }
    }

    #[cfg(test)]
    fn buffered_reader_read_test_aux<'a, T: BufferedReader<C> + 'a, C: fmt::Debug + Sync + Send>
        (mut bio: T, data: &[u8]) {
        let mut buffer = [0; 99];

        // Make sure the test file has more than buffer.len() bytes
        // worth of data.
        assert!(buffer.len() < data.len());

        // The number of reads we'll have to perform.
        let iters = (data.len() + buffer.len() - 1) / buffer.len();
        // Iterate more than the number of required reads to check
        // what happens when we try to read beyond the end of the
        // file.
        for i in 1..iters + 2 {
            let data_start = (i - 1) * buffer.len();

            // We don't want to just check that read works in
            // isolation.  We want to be able to mix .read and .data
            // calls.
            {
                let result = bio.data(buffer.len());
                let buffer = result.unwrap();
                if !buffer.is_empty() {
                    assert_eq!(buffer,
                               &data[data_start..data_start + buffer.len()]);
                }
            }

            // Now do the actual read.
            let result = bio.read(&mut buffer[..]);
            let got = result.unwrap();
            if got > 0 {
                assert_eq!(&buffer[0..got],
                           &data[data_start..data_start + got]);
            }

            if i > iters {
                // We should have read everything.
                assert!(got == 0);
            } else if i == iters {
                // The last read.  This may be less than buffer.len().
                // But it should include at least one byte.
                assert!(0 < got);
                assert!(got <= buffer.len());
            } else {
                assert_eq!(got, buffer.len());
            }
        }
    }

    #[test]
    fn buffered_reader_read_test() {
        let data : &[u8] = include_bytes!("buffered-reader-test.txt");

        {
            let bio = Memory::new(data);
            buffered_reader_read_test_aux (bio, data);
        }

        {
            use std::path::PathBuf;
            use std::fs::File;

            let path : PathBuf = [env!("CARGO_MANIFEST_DIR"),
                                  "src",
                                  "buffered-reader-test.txt"]
                .iter().collect();

            let mut f = File::open(&path).expect(&path.to_string_lossy());
            let bio = Generic::new(&mut f, None);
            buffered_reader_read_test_aux (bio, data);
        }
    }

    #[test]
    fn drop_until() {
        let data : &[u8] = &b"abcd"[..];
        let mut reader = Memory::new(data);

        // Matches the 'a' at 0 and consumes 0 bytes.
        assert_eq!(reader.drop_until(b"ab").unwrap(), 0);
        // Matches the 'b' at 1 and consumes 1 byte.
        assert_eq!(reader.drop_until(b"bc").unwrap(), 1);
        // Matches the 'b' at 1 and consumes 0 bytes.
        assert_eq!(reader.drop_until(b"ab").unwrap(), 0);
        // Matches the 'd' at 4 and consumes 2 bytes.
        assert_eq!(reader.drop_until(b"de").unwrap(), 2);
        // Matches nothing, consuming the last 1 byte.
        assert_eq!(reader.drop_until(b"e").unwrap(), 1);
        // Matches nothing, consuming nothing.
        assert_eq!(reader.drop_until(b"e").unwrap(), 0);
    }

    #[test]
    fn drop_through() {
        let data : &[u8] = &b"abcd"[..];
        let mut reader = Memory::new(data);

        // Matches the 'a' at 0 and consumes 1 byte.
        assert_eq!(reader.drop_through(b"ab", false).unwrap(),
                   (Some(b'a'), 1));
        // Matches the 'b' at 1 and consumes 1 byte.
        assert_eq!(reader.drop_through(b"ab", false).unwrap(),
                   (Some(b'b'), 1));
        // Matches the 'd' at 4 and consumes 2 byte.
        assert_eq!(reader.drop_through(b"def", false).unwrap(),
                   (Some(b'd'), 2));
        // Doesn't match (eof).
        assert!(reader.drop_through(b"def", false).is_err());
        // Matches EOF.
        assert!(reader.drop_through(b"def", true).unwrap().0.is_none());
    }
}
