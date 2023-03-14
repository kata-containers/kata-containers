A `BufferedReader` is a super-powered `Read`er.

Like the [`BufRead`] trait, the `BufferedReader` trait has an
internal buffer that is directly exposed to the user.  This design
enables two performance optimizations.  First, the use of an
internal buffer amortizes system calls.  Second, exposing the
internal buffer allows the user to work with data in place, which
avoids another copy.

The [`BufRead`] trait, however, has a significant limitation for
parsers: the user of a [`BufRead`] object can't control the amount
of buffering.  This is essential for being able to conveniently
work with data in place, and being able to lookahead without
consuming data.  The result is that either the sizing has to be
handled by the instantiator of the [`BufRead`] object---assuming
the [`BufRead`] object provides such a mechanism---which is a
layering violation, or the parser has to fallback to buffering if
the internal buffer is too small, which eliminates most of the
advantages of the [`BufRead`] abstraction.  The `BufferedReader`
trait addresses this shortcoming by allowing the user to control
the size of the internal buffer.

The `BufferedReader` trait also has some functionality,
specifically, a generic interface to work with a stack of
`BufferedReader` objects, that simplifies using multiple parsers
simultaneously.  This is helpful when one parser deals with
framing (e.g., something like [HTTP's chunk transfer encoding]),
and another decodes the actual objects.  It is also useful when
objects are nested.

# Details

Because the [`BufRead`] trait doesn't provide a mechanism for the
user to size the internal buffer, a parser can't generally be sure
that the internal buffer will be large enough to allow it to work
with all data in place.

Using the standard [`BufRead`] implementation, [`BufReader`], the
instantiator can set the size of the internal buffer at creation
time.  Unfortunately, this mechanism is ugly, and not always
adequate.  First, the parser is typically not the instantiator.
Thus, the instantiator needs to know about the implementation
details of all of the parsers, which turns an implementation
detail into a cross-cutting concern.  Second, when working with
dynamically sized data, the maximum amount of the data that needs
to be worked with in place may not be known apriori, or the
maximum amount may be significantly larger than the typical
amount.  This leads to poorly sized buffers.

Alternatively, the code that uses, but does not instantiate a
[`BufRead`] object, can be changed to stream the data, or to
fallback to reading the data into a local buffer if the internal
buffer is too small.  Both of these approaches increase code
complexity, and the latter approach is contrary to the
[`BufRead`]'s goal of reducing unnecessary copying.

The `BufferedReader` trait solves this problem by allowing the
user to dynamically (i.e., at read time, not open time) ensure
that the internal buffer has a certain amount of data.

The ability to control the size of the internal buffer is also
essential to straightforward support for speculative lookahead.
The reason that speculative lookahead with a [`BufRead`] object is
difficult is that speculative lookahead is /speculative/, i.e., if
the parser backtracks, the data that was read must not be
consumed.  Using a [`BufRead`] object, this is not possible if the
amount of lookahead is larger than the internal buffer.  That is,
if the amount of lookahead data is larger than the [`BufRead`]'s
internal buffer, the parser first has to `BufRead::consume`() some
data to be able to examine more data.  But, if the parser then
decides to backtrack, it has no way to return the unused data to
the [`BufRead`] object.  This forces the parser to manage a buffer
of read, but unconsumed data, which significantly complicates the
code.

The `BufferedReader` trait also simplifies working with a stack of
`BufferedReader`s in two ways.  First, the `BufferedReader` trait
provides *generic* methods to access the underlying
`BufferedReader`.  Thus, even when dealing with a trait object, it
is still possible to recover the underlying `BufferedReader`.
Second, the `BufferedReader` provides a mechanism to associate
generic state with each `BufferedReader` via a cookie.  Although
it is possible to realize this functionality using a custom trait
that extends the `BufferedReader` trait and wraps existing
`BufferedReader` implementations, this approach eliminates a lot
of error-prone, boilerplate code.

[`BufRead`]: https://doc.rust-lang.org/stable/std/io/trait.BufRead.html
[`BufReader`]: https://doc.rust-lang.org/stable/std/io/struct.BufReader.html
[HTTP's chunk transfer encoding]: https://en.wikipedia.org/wiki/Chunked_transfer_encoding
