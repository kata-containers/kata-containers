//! Abstractions for creating [`io::Write`] instances.
//!
//! [`io::Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
use std::{
    fmt::Debug,
    io::{self, Write},
    sync::Arc,
};
use tracing_core::Metadata;

/// A type that can create [`io::Write`] instances.
///
/// `MakeWriter` is used by [`fmt::Subscriber`] or [`fmt::Layer`] to print
/// formatted text representations of [`Event`]s.
///
/// This trait is already implemented for function pointers and
/// immutably-borrowing closures that return an instance of [`io::Write`], such
/// as [`io::stdout`] and [`io::stderr`].
///
/// The [`MakeWriter::make_writer_for`] method takes [`Metadata`] describing a
/// span or event and returns a writer. `MakeWriter`s can optionally provide
/// implementations of this method with behaviors that differ based on the span
/// or event being written. For example, events at different [levels] might be
/// written to different output streams, or data from different [targets] might
/// be written to separate log files. When the `MakeWriter` has no custom
/// behavior based on metadata, the default implementation of `make_writer_for`
/// simply calls `self.make_writer()`, ignoring the metadata. Therefore, when
/// metadata _is_ available, callers should prefer to call `make_writer_for`,
/// passing in that metadata, so that the `MakeWriter` implementation can choose
/// the appropriate behavior.
///
/// # Examples
///
/// The simplest usage is to pass in a named function that returns a writer. For
/// example, to log all events to stderr, we could write:
/// ```
/// let subscriber = tracing_subscriber::fmt()
///     .with_writer(std::io::stderr)
///     .finish();
/// # drop(subscriber);
/// ```
///
/// Any function that returns a writer can be used:
///
/// ```
/// fn make_my_great_writer() -> impl std::io::Write {
///     // ...
///     # std::io::stdout()
/// }
///
/// let subscriber = tracing_subscriber::fmt()
///     .with_writer(make_my_great_writer)
///     .finish();
/// # drop(subscriber);
/// ```
///
/// A closure can be used to introduce arbitrary logic into how the writer is
/// created. Consider the (admittedly rather silly) example of sending every 5th
/// event to stderr, and all other events to stdout:
///
/// ```
/// use std::io;
/// use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
///
/// let n = AtomicUsize::new(0);
/// let subscriber = tracing_subscriber::fmt()
///     .with_writer(move || -> Box<dyn io::Write> {
///         if n.fetch_add(1, Relaxed) % 5 == 0 {
///             Box::new(io::stderr())
///         } else {
///             Box::new(io::stdout())
///        }
///     })
///     .finish();
/// # drop(subscriber);
/// ```
///
/// [`io::Write`]: std::io::Write
/// [`fmt::Layer`]: crate::fmt::Layer
/// [`fmt::Subscriber`]: crate::fmt::Subscriber
/// [`Event`]: tracing_core::event::Event
/// [`io::stdout`]: std::io::stdout()
/// [`io::stderr`]: std::io::stderr()
/// [mutex]: std::sync::Mutex
/// [`MakeWriter::make_writer_for`]: MakeWriter::make_writer_for
/// [`Metadata`]: tracing_core::Metadata
/// [levels]: tracing_core::Level
/// [targets]: tracing_core::Metadata::target
pub trait MakeWriter {
    /// The concrete [`io::Write`] implementation returned by [`make_writer`].
    ///
    /// [`io::Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
    /// [`make_writer`]: #tymethod.make_writer
    type Writer: io::Write;

    /// Returns an instance of [`Writer`].
    ///
    /// # Implementer notes
    ///
    /// [`fmt::Layer`] or [`fmt::Subscriber`] will call this method each time an event is recorded. Ensure any state
    /// that must be saved across writes is not lost when the [`Writer`] instance is dropped. If
    /// creating a [`io::Write`] instance is expensive, be sure to cache it when implementing
    /// [`MakeWriter`] to improve performance.
    ///
    /// [`Writer`]: #associatedtype.Writer
    /// [`fmt::Layer`]: crate::fmt::Layer
    /// [`fmt::Subscriber`]: crate::fmt::Subscriber
    /// [`io::Write`]: std::io::Write
    fn make_writer(&self) -> Self::Writer;

    /// Returns a [`Writer`] for writing data from the span or event described
    /// by the provided [`Metadata`].
    ///
    /// By default, this calls [`self.make_writer()`][make_writer], ignoring
    /// the provided metadata, but implementations can override this to provide
    /// metadata-specific behaviors.
    ///
    /// This method allows `MakeWriter` implementations to implement different
    /// behaviors based on the span or event being written. The `MakeWriter`
    /// type might return different writers based on the provided metadata, or
    /// might write some values to the writer before or after providing it to
    /// the caller.
    ///
    /// For example, we might want to write data from spans and events at the
    /// [`ERROR`] and [`WARN`] levels to `stderr`, and data from spans or events
    /// at lower levels to stdout:
    ///
    /// ```
    /// use std::io::{self, Stdout, Stderr};
    /// use tracing_subscriber::fmt::writer::MakeWriter;
    /// use tracing_core::{Metadata, Level};
    ///
    /// pub struct MyMakeWriter {}
    ///
    /// /// A lock on either stdout or stderr, depending on the verbosity level
    /// /// of the event being written.
    /// pub enum Stdio {
    ///     Stdout(Stdout),
    ///     Stderr(Stderr),
    /// }
    ///
    /// impl io::Write for Stdio {
    ///     fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    ///         match self {
    ///             Stdio::Stdout(io) => io.write(buf),
    ///             Stdio::Stderr(io) => io.write(buf),
    ///         }
    ///     }
    ///
    ///     fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
    ///         // ...
    ///         # match self {
    ///         #     Stdio::Stdout(io) => io.write_all(buf),
    ///         #     Stdio::Stderr(io) => io.write_all(buf),
    ///         # }
    ///     }
    ///
    ///     fn flush(&mut self) -> io::Result<()> {
    ///         // ...
    ///         # match self {
    ///         #     Stdio::Stdout(io) => io.flush(),
    ///         #     Stdio::Stderr(io) => io.flush(),
    ///         # }
    ///     }
    /// }
    ///
    /// impl MakeWriter for MyMakeWriter {
    ///     type Writer = Stdio;
    ///
    ///     fn make_writer(&self) -> Self::Writer {
    ///         // We must have an implementation of `make_writer` that makes
    ///         // a "default" writer without any configuring metadata. Let's
    ///         // just return stdout in that case.
    ///         Stdio::Stdout(io::stdout())
    ///     }
    ///
    ///     fn make_writer_for(&self, meta: &Metadata<'_>) -> Self::Writer {
    ///         // Here's where we can implement our special behavior. We'll
    ///         // check if the metadata's verbosity level is WARN or ERROR,
    ///         // and return stderr in that case.
    ///         if meta.level() <= &Level::WARN {
    ///             return Stdio::Stderr(io::stderr());
    ///         }
    ///
    ///         // Otherwise, we'll return stdout.
    ///         Stdio::Stdout(io::stdout())
    ///     }
    /// }
    /// ```
    ///
    /// [`Writer`]: MakeWriter::Writer
    /// [`Metadata`]: tracing_core::Metadata
    /// [make_writer]: MakeWriter::make_writer
    /// [`WARN`]: tracing_core::Level::WARN
    /// [`ERROR`]: tracing_core::Level::ERROR
    fn make_writer_for(&self, meta: &Metadata<'_>) -> Self::Writer {
        let _ = meta;
        self.make_writer()
    }
}

/// Extension trait adding combinators for working with types implementing
/// [`MakeWriter`].
///
/// This is not intended to be implemented directly for user-defined
/// [`MakeWriter`]s; instead, it should be imported when the desired methods are
/// used.
pub trait MakeWriterExt: MakeWriter {
    /// Wraps `self` and returns a [`MakeWriter`] that will only write output
    /// for events at or below the provided verbosity [`Level`]. For instance,
    /// `Level::TRACE` is considered to be _more verbose` than `Level::INFO`.
    ///
    /// Events whose level is more verbose than `level` will be ignored, and no
    /// output will be written.
    ///
    /// # Examples
    ///
    /// ```
    /// use tracing::Level;
    /// use tracing_subscriber::fmt::writer::MakeWriterExt;
    ///
    /// // Construct a writer that outputs events to `stderr` only if the span or
    /// // event's level is >= WARN (WARN and ERROR).
    /// let mk_writer = std::io::stderr.with_max_level(Level::WARN);
    ///
    /// tracing_subscriber::fmt().with_writer(mk_writer).init();
    /// ```
    ///
    /// Writing the `ERROR` and `WARN` levels to `stderr`, and everything else
    /// to `stdout`:
    ///
    /// ```
    /// # use tracing::Level;
    /// # use tracing_subscriber::fmt::writer::MakeWriterExt;
    ///
    /// let mk_writer = std::io::stderr
    ///     .with_max_level(Level::WARN)
    ///     .or_else(std::io::stdout);
    ///
    /// tracing_subscriber::fmt().with_writer(mk_writer).init();
    /// ```
    ///
    /// Writing the `ERROR` level to `stderr`, the `INFO` and `WARN` levels to
    /// `stdout`, and the `INFO` and DEBUG` levels to a file:
    ///
    /// ```
    /// # use tracing::Level;
    /// # use tracing_subscriber::fmt::writer::MakeWriterExt;
    /// use std::{sync::Arc, fs::File};
    /// # // don't actually create the file when running the tests.
    /// # fn docs() -> std::io::Result<()> {
    /// let debug_log = Arc::new(File::create("debug.log")?);
    ///
    /// let mk_writer = std::io::stderr
    ///     .with_max_level(Level::ERROR)
    ///     .or_else(std::io::stdout
    ///         .with_max_level(Level::INFO)
    ///         .and(debug_log.with_max_level(Level::DEBUG))
    ///     );
    ///
    /// tracing_subscriber::fmt().with_writer(mk_writer).init();
    /// # Ok(()) }
    /// ```
    ///
    /// [`Level`]: tracing_core::Level
    /// [`io::Write`]: std::io::Write
    fn with_max_level(self, level: tracing_core::Level) -> WithMaxLevel<Self>
    where
        Self: Sized,
    {
        WithMaxLevel::new(self, level)
    }

    /// Wraps `self` and returns a [`MakeWriter`] that will only write output
    /// for events at or above the provided verbosity [`Level`].
    ///
    /// Events whose level is less verbose than `level` will be ignored, and no
    /// output will be written.
    ///
    /// # Examples
    ///
    /// ```
    /// use tracing::Level;
    /// use tracing_subscriber::fmt::writer::MakeWriterExt;
    ///
    /// // Construct a writer that outputs events to `stdout` only if the span or
    /// // event's level is <= DEBUG (DEBUG and TRACE).
    /// let mk_writer = std::io::stdout.with_min_level(Level::DEBUG);
    ///
    /// tracing_subscriber::fmt().with_writer(mk_writer).init();
    /// ```
    /// This can be combined with [`MakeWriterExt::with_max_level`] to write
    /// only within a range of levels:
    ///
    /// ```
    /// # use tracing::Level;
    /// # use tracing_subscriber::fmt::writer::MakeWriterExt;
    /// // Only write the `DEBUG` and `INFO` levels to stdout.
    /// let mk_writer = std::io::stdout
    ///     .with_max_level(Level::DEBUG)
    ///     .with_min_level(Level::INFO)
    ///     // Write the `WARN` and `ERROR` levels to stderr.
    ///     .and(std::io::stderr.with_min_level(Level::WARN));
    ///
    /// tracing_subscriber::fmt().with_writer(mk_writer).init();
    /// ```
    /// [`Level`]: tracing_core::Level
    /// [`io::Write`]: std::io::Write
    fn with_min_level(self, level: tracing_core::Level) -> WithMinLevel<Self>
    where
        Self: Sized,
    {
        WithMinLevel::new(self, level)
    }

    /// Wraps `self` with a predicate that takes a span or event's [`Metadata`]
    /// and returns a `bool`. The returned [`MakeWriter`]'s
    /// [`MakeWriter::make_writer_for`][mwf] method will check the predicate to
    /// determine if  a writer should be produced for a given span or event.
    ///
    /// If the predicate returns `false`, the wrapped [`MakeWriter`]'s
    /// [`make_writer_for`][mwf] will return [`OptionalWriter::none`][own].
    /// Otherwise, it calls the wrapped [`MakeWriter`]'s
    /// [`make_writer_for`][mwf] method, and returns the produced writer.
    ///
    /// This can be used to filter an output based on arbitrary [`Metadata`]
    /// parameters.
    ///
    /// # Examples
    ///
    /// Writing events with a specific target to an HTTP access log, and other
    /// events to stdout:
    ///
    /// ```
    /// use tracing_subscriber::fmt::writer::MakeWriterExt;
    /// use std::{sync::Arc, fs::File};
    /// # // don't actually create the file when running the tests.
    /// # fn docs() -> std::io::Result<()> {
    /// let access_log = Arc::new(File::create("access.log")?);
    ///
    /// let mk_writer = access_log
    ///     // Only write events with the target "http::access_log" to the
    ///     // access log file.
    ///     .with_filter(|meta| meta.target() == "http::access_log")
    ///     // Write events with all other targets to stdout.
    ///     .or_else(std::io::stdout);
    ///
    /// tracing_subscriber::fmt().with_writer(mk_writer).init();
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Conditionally enabling or disabling a log file:
    /// ```
    /// use tracing_subscriber::fmt::writer::MakeWriterExt;
    /// use std::{
    ///     sync::{Arc, atomic::{AtomicBool, Ordering}},
    ///     fs::File,
    /// };
    ///
    /// static DEBUG_LOG_ENABLED: AtomicBool = AtomicBool::new(false);
    ///
    /// # // don't actually create the file when running the tests.
    /// # fn docs() -> std::io::Result<()> {
    /// // Create the debug log file
    /// let debug_file = Arc::new(File::create("debug.log")?)
    ///     // Enable the debug log only if the flag is enabled.
    ///     .with_filter(|_| DEBUG_LOG_ENABLED.load(Ordering::Acquire));
    ///
    /// // Always write to stdout
    /// let mk_writer = std::io::stdout
    ///     // Write to the debug file if it's enabled
    ///     .and(debug_file);
    ///
    /// tracing_subscriber::fmt().with_writer(mk_writer).init();
    ///
    /// // ...
    ///
    /// // Later, we can toggle on or off the debug log file.
    /// DEBUG_LOG_ENABLED.store(true, Ordering::Release);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`Metadata`]: tracing_core::Metadata
    /// [mwf]: MakeWriter::make_writer_for
    /// [own]: EitherWriter::none
    fn with_filter<F>(self, filter: F) -> WithFilter<Self, F>
    where
        Self: Sized,
        F: Fn(&Metadata<'_>) -> bool,
    {
        WithFilter::new(self, filter)
    }

    /// Combines `self` with another type implementing [`MakeWriter`], returning
    /// a new [`MakeWriter`] that produces [writers] that write to *both*
    /// outputs.
    ///
    /// If writing to either writer returns an error, the returned writer will
    /// return that error. However, both writers will still be written to before
    /// the error is returned, so it is possible for one writer to fail while
    /// the other is written to successfully.
    ///
    /// # Examples
    ///
    /// ```
    /// use tracing_subscriber::fmt::writer::MakeWriterExt;
    ///
    /// // Construct a writer that outputs events to `stdout` *and* `stderr`.
    /// let mk_writer = std::io::stdout.and(std::io::stderr);
    ///
    /// tracing_subscriber::fmt().with_writer(mk_writer).init();
    /// ```
    ///
    /// `and` can be used in conjunction with filtering combinators. For
    /// example, if we want to write to a number of outputs depending on the
    /// level of an event, we could write:
    ///
    /// ```
    /// use tracing::Level;
    /// # use tracing_subscriber::fmt::writer::MakeWriterExt;
    /// use std::{sync::Arc, fs::File};
    /// # // don't actually create the file when running the tests.
    /// # fn docs() -> std::io::Result<()> {
    /// let debug_log = Arc::new(File::create("debug.log")?);
    ///
    /// // Write everything to the debug log.
    /// let mk_writer = debug_log
    ///     // Write the `ERROR` and `WARN` levels to stderr.
    ///     .and(std::io::stderr.with_max_level(Level::WARN))
    ///     // Write `INFO` to `stdout`.
    ///     .and(std::io::stdout
    ///         .with_max_level(Level::INFO)
    ///         .with_min_level(Level::INFO)
    ///     );
    ///
    /// tracing_subscriber::fmt().with_writer(mk_writer).init();
    /// # Ok(()) }
    /// ```
    ///
    /// [writers]: std::io::Write
    fn and<B>(self, other: B) -> Tee<Self, B>
    where
        Self: Sized,
        B: MakeWriter + Sized,
    {
        Tee::new(self, other)
    }

    /// Combines `self` with another type implementing [`MakeWriter`], returning
    /// a new [`MakeWriter`] that calls `other`'s [`make_writer`] if `self`'s
    /// `make_writer` returns [`OptionalWriter::none`][own].
    ///
    /// # Examples
    ///
    /// ```
    /// use tracing::Level;
    /// use tracing_subscriber::fmt::writer::MakeWriterExt;
    ///
    /// // Produces a writer that writes to `stderr` if the level is >= WARN,
    /// // or returns `OptionalWriter::none()` otherwise.
    /// let stderr = std::io::stderr.with_max_level(Level::WARN);
    ///
    /// // If the `stderr` `MakeWriter` is disabled by the max level filter,
    /// // write to stdout instead:
    /// let mk_writer = stderr.or_else(std::io::stdout);
    ///
    /// tracing_subscriber::fmt().with_writer(mk_writer).init();
    /// ```
    ///
    /// [`make_writer`]: MakeWriter::make_writer
    /// [own]: EitherWriter::none
    fn or_else<W, B>(self, other: B) -> OrElse<Self, B>
    where
        Self: MakeWriter<Writer = OptionalWriter<W>> + Sized,
        B: MakeWriter + Sized,
        W: Write,
    {
        OrElse::new(self, other)
    }
}

/// A writer intended to support [`libtest`'s output capturing][capturing] for use in unit tests.
///
/// `TestWriter` is used by [`fmt::Subscriber`] or [`fmt::Layer`] to enable capturing support.
///
/// `cargo test` can only capture output from the standard library's [`print!`] macro. See
/// [`libtest`'s output capturing][capturing] for more details about output capturing.
///
/// Writing to [`io::stdout`] and [`io::stderr`] produces the same results as using
/// [`libtest`'s `--nocapture` option][nocapture] which may make the results look unreadable.
///
/// [`fmt::Subscriber`]: ../struct.Subscriber.html
/// [`fmt::Layer`]: ../struct.Layer.html
/// [capturing]: https://doc.rust-lang.org/book/ch11-02-running-tests.html#showing-function-output
/// [nocapture]: https://doc.rust-lang.org/cargo/commands/cargo-test.html
/// [`io::stdout`]: https://doc.rust-lang.org/std/io/fn.stdout.html
/// [`io::stderr`]: https://doc.rust-lang.org/std/io/fn.stderr.html
/// [`print!`]: https://doc.rust-lang.org/std/macro.print.html
#[derive(Default, Debug)]
pub struct TestWriter {
    _p: (),
}

/// A writer that erases the specific [`io::Write`] and [`MakeWriter`] types being used.
///
/// This is useful in cases where the concrete type of the writer cannot be known
/// until runtime.
///
/// # Examples
///
/// A function that returns a [`Subscriber`] that will write to either stdout or stderr:
///
/// ```rust
/// # use tracing::Subscriber;
/// # use tracing_subscriber::fmt::writer::BoxMakeWriter;
///
/// fn dynamic_writer(use_stderr: bool) -> impl Subscriber {
///     let writer = if use_stderr {
///         BoxMakeWriter::new(std::io::stderr)
///     } else {
///         BoxMakeWriter::new(std::io::stdout)
///     };
///
///     tracing_subscriber::fmt().with_writer(writer).finish()
/// }
/// ```
///
/// [`Subscriber`]: tracing::Subscriber
/// [`io::Write`]: std::io::Write
pub struct BoxMakeWriter {
    inner: Box<dyn MakeWriter<Writer = Box<dyn Write + 'static>> + Send + Sync>,
    name: &'static str,
}

/// A [writer] that is one of two types implementing [`io::Write`][writer].
///
/// This may be used by [`MakeWriter`] implementations that may conditionally
/// return one of two writers.
///
/// [writer]: std::io::Write
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum EitherWriter<A, B> {
    /// A writer of type `A`.
    A(A),
    /// A writer of type `B`.
    B(B),
}

/// A [writer] which may or may not be enabled.
///
/// This may be used by [`MakeWriter`] implementations that wish to
/// conditionally enable or disable the returned writer based on a span or
/// event's [`Metadata`].
///
/// [writer]: std::io::Write
pub type OptionalWriter<T> = EitherWriter<T, std::io::Sink>;

/// A [`MakeWriter`] combinator that only returns an enabled [writer] for spans
/// and events with metadata at or below a specified verbosity [`Level`].
///
/// This is returned by the [`MakeWriterExt::with_max_level`] method. See the
/// method documentation for details.
///
/// [writer]: std::io::Write
/// [`Level`]: tracing_core::Level
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct WithMaxLevel<M> {
    make: M,
    level: tracing_core::Level,
}

/// A [`MakeWriter`] combinator that only returns an enabled [writer] for spans
/// and events with metadata at or above a specified verbosity [`Level`].
///
/// This is returned by the [`MakeWriterExt::with_min_level`] method. See the
/// method documentation for details.
///
/// [writer]: std::io::Write
/// [`Level`]: tracing_core::Level
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct WithMinLevel<M> {
    make: M,
    level: tracing_core::Level,
}

/// A [`MakeWriter`] combinator that wraps a [`MakeWriter`] with a predicate for
/// span and event [`Metadata`], so that the [`MakeWriter::make_writer_for`]
/// method returns [`OptionalWriter::some`][ows] when the predicate returns `true`,
/// and [`OptionalWriter::none`][own] when the predicate returns `false`.
///
/// This is returned by the [`MakeWriterExt::with_filter`] method. See the
/// method documentation for details.
///
/// [`Metadata`]: tracing_core::Metadata
/// [ows]: EitherWriter::some
/// [own]: EitherWriter::none
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct WithFilter<M, F> {
    make: M,
    filter: F,
}

/// Combines a [`MakeWriter`] that returns an [`OptionalWriter`] with another
/// [`MakeWriter`], so that the second [`MakeWriter`] is used when the first
/// [`MakeWriter`] returns [`OptionalWriter::none`][own].
///
/// This is returned by the [`MakeWriterExt::or_else] method. See the
/// method documentation for details.
///
/// [own]: EitherWriter::none
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct OrElse<A, B> {
    inner: A,
    or_else: B,
}

/// Combines two types implementing [`MakeWriter`] (or [`std::io::Write`]) to
/// produce a writer that writes to both [`MakeWriter`]'s returned writers.
///
/// This is returned by the [`MakeWriterExt::and`] method. See the method
/// documentation for details.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Tee<A, B> {
    a: A,
    b: B,
}

impl<F, W> MakeWriter for F
where
    F: Fn() -> W,
    W: io::Write,
{
    type Writer = W;

    fn make_writer(&self) -> Self::Writer {
        (self)()
    }
}

impl<W> MakeWriter for Arc<W>
where
    for<'a> &'a W: io::Write,
{
    type Writer = ArcWriter<W>;
    fn make_writer(&self) -> Self::Writer {
        ArcWriter(self.clone())
    }
}

/// Implements [`std::io::Write`] for an [`Arc`]<W> where `&W: Write`.
///
/// This is an implementation detail of the [`MakeWriter`] impl for [`Arc`].
#[derive(Clone, Debug)]
pub struct ArcWriter<W>(Arc<W>);

// === impl TestWriter ===

impl TestWriter {
    /// Returns a new `TestWriter` with the default configuration.
    pub fn new() -> Self {
        Self::default()
    }
}

impl io::Write for TestWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let out_str = String::from_utf8_lossy(buf);
        print!("{}", out_str);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl MakeWriter for TestWriter {
    type Writer = Self;

    fn make_writer(&self) -> Self::Writer {
        Self::default()
    }
}

impl BoxMakeWriter {
    /// Constructs a `BoxMakeWriter` wrapping a type implementing [`MakeWriter`].
    ///
    /// [`MakeWriter`]: trait.MakeWriter.html
    pub fn new<M>(make_writer: M) -> Self
    where
        M: MakeWriter + Send + Sync + 'static,
        M::Writer: Write + 'static,
    {
        Self {
            inner: Box::new(Boxed(make_writer)),
            name: std::any::type_name::<M>(),
        }
    }
}

impl Debug for BoxMakeWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("BoxMakeWriter")
            .field(&format_args!("<{}>", self.name))
            .finish()
    }
}

impl MakeWriter for BoxMakeWriter {
    type Writer = Box<dyn Write>;

    fn make_writer(&self) -> Self::Writer {
        self.inner.make_writer()
    }

    fn make_writer_for(&self, meta: &Metadata<'_>) -> Self::Writer {
        self.inner.make_writer_for(meta)
    }
}

struct Boxed<M>(M);

impl<M> MakeWriter for Boxed<M>
where
    M: MakeWriter,
    M::Writer: Write + 'static,
{
    type Writer = Box<dyn Write>;

    fn make_writer(&self) -> Self::Writer {
        let w = self.0.make_writer();
        Box::new(w)
    }

    fn make_writer_for(&self, meta: &Metadata<'_>) -> Self::Writer {
        let w = self.0.make_writer_for(meta);
        Box::new(w)
    }
}

// === impl EitherWriter ===

impl<A, B> io::Write for EitherWriter<A, B>
where
    A: io::Write,
    B: io::Write,
{
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            EitherWriter::A(a) => a.write(buf),
            EitherWriter::B(b) => b.write(buf),
        }
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        match self {
            EitherWriter::A(a) => a.flush(),
            EitherWriter::B(b) => b.flush(),
        }
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        match self {
            EitherWriter::A(a) => a.write_vectored(bufs),
            EitherWriter::B(b) => b.write_vectored(bufs),
        }
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        match self {
            EitherWriter::A(a) => a.write_all(buf),
            EitherWriter::B(b) => b.write_all(buf),
        }
    }

    #[inline]
    fn write_fmt(&mut self, fmt: std::fmt::Arguments<'_>) -> io::Result<()> {
        match self {
            EitherWriter::A(a) => a.write_fmt(fmt),
            EitherWriter::B(b) => b.write_fmt(fmt),
        }
    }
}

impl<T> OptionalWriter<T> {
    /// Returns a [disabled writer].
    ///
    /// Any bytes written to the returned writer are discarded.
    ///
    /// This is equivalent to returning [`Option::None`].
    ///
    /// [disabled writer]: std::io::sink
    #[inline]
    pub fn none() -> Self {
        EitherWriter::B(std::io::sink())
    }

    /// Returns an enabled writer of type `T`.
    ///
    /// This is equivalent to returning [`Option::Some`].
    #[inline]
    pub fn some(t: T) -> Self {
        EitherWriter::A(t)
    }
}

impl<T> From<Option<T>> for OptionalWriter<T> {
    #[inline]
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(writer) => Self::some(writer),
            None => Self::none(),
        }
    }
}

// === impl WithMaxLevel ===

impl<M> WithMaxLevel<M> {
    /// Wraps the provided [`MakeWriter`] with a maximum [`Level`], so that it
    /// returns [`OptionalWriter::none`][own] for spans and events whose level is
    /// more verbose than the maximum level.
    ///
    /// See [`MakeWriterExt::with_max_level`] for details.
    ///
    /// [`Level`]: tracing_core::Level
    /// [own]: EitherWriter::none
    pub fn new(make: M, level: tracing_core::Level) -> Self {
        Self { make, level }
    }
}

impl<M: MakeWriter> MakeWriter for WithMaxLevel<M> {
    type Writer = OptionalWriter<M::Writer>;

    #[inline]
    fn make_writer(&self) -> Self::Writer {
        // If we don't know the level, assume it's disabled.
        OptionalWriter::none()
    }

    #[inline]
    fn make_writer_for(&self, meta: &Metadata<'_>) -> Self::Writer {
        if meta.level() <= &self.level {
            return OptionalWriter::some(self.make.make_writer_for(meta));
        }
        OptionalWriter::none()
    }
}

// === impl WithMinLevel ===

impl<M> WithMinLevel<M> {
    /// Wraps the provided [`MakeWriter`] with a minimum [`Level`], so that it
    /// returns [`OptionalWriter::none`][own] for spans and events whose level is
    /// less verbose than the maximum level.
    ///
    /// See [`MakeWriterExt::with_min_level`] for details.
    ///
    /// [`Level`]: tracing_core::Level
    /// [own]: EitherWriter::none
    pub fn new(make: M, level: tracing_core::Level) -> Self {
        Self { make, level }
    }
}

impl<M: MakeWriter> MakeWriter for WithMinLevel<M> {
    type Writer = OptionalWriter<M::Writer>;

    #[inline]
    fn make_writer(&self) -> Self::Writer {
        // If we don't know the level, assume it's disabled.
        OptionalWriter::none()
    }

    #[inline]
    fn make_writer_for(&self, meta: &Metadata<'_>) -> Self::Writer {
        if meta.level() >= &self.level {
            return OptionalWriter::some(self.make.make_writer_for(meta));
        }
        OptionalWriter::none()
    }
}

// ==== impl WithFilter ===

impl<M, F> WithFilter<M, F> {
    /// Wraps `make` with the provided `filter`, returning a [`MakeWriter`] that
    /// will call `make.make_writer_for()` when `filter` returns `true` for a
    /// span or event's [`Metadata`], and returns a [`sink`] otherwise.
    ///
    /// See [`MakeWriterExt::with_filter`] for details.
    ///
    /// [`Metadata`]: tracing_core::Metadata
    /// [`sink`]: std::io::sink
    pub fn new(make: M, filter: F) -> Self
    where
        F: Fn(&Metadata<'_>) -> bool,
    {
        Self { make, filter }
    }
}

impl<M, F> MakeWriter for WithFilter<M, F>
where
    M: MakeWriter,
    F: Fn(&Metadata<'_>) -> bool,
{
    type Writer = OptionalWriter<M::Writer>;

    #[inline]
    fn make_writer(&self) -> Self::Writer {
        OptionalWriter::some(self.make.make_writer())
    }

    #[inline]
    fn make_writer_for(&self, meta: &Metadata<'_>) -> Self::Writer {
        if (self.filter)(meta) {
            OptionalWriter::some(self.make.make_writer_for(meta))
        } else {
            OptionalWriter::none()
        }
    }
}

// === impl Tee ===

impl<A, B> Tee<A, B> {
    /// Combines two types implementing [`MakeWriter`], returning
    /// a new [`MakeWriter`] that produces [writers] that write to *both*
    /// outputs.
    ///
    /// See the documentation for [`MakeWriterExt::and`] for details.
    ///
    /// [writers]: std::io::Write
    pub fn new(a: A, b: B) -> Self {
        Self { a, b }
    }
}

impl<A, B> MakeWriter for Tee<A, B>
where
    A: MakeWriter,
    B: MakeWriter,
{
    type Writer = Tee<A::Writer, B::Writer>;

    #[inline]
    fn make_writer(&self) -> Self::Writer {
        Tee::new(self.a.make_writer(), self.b.make_writer())
    }

    #[inline]
    fn make_writer_for(&self, meta: &Metadata<'_>) -> Self::Writer {
        Tee::new(self.a.make_writer_for(meta), self.b.make_writer_for(meta))
    }
}

macro_rules! impl_tee {
    ($self_:ident.$f:ident($($arg:ident),*)) => {
        {
            let res_a = $self_.a.$f($($arg),*);
            let res_b = $self_.b.$f($($arg),*);
            (res_a?, res_b?)
        }
    }
}

impl<A, B> io::Write for Tee<A, B>
where
    A: io::Write,
    B: io::Write,
{
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let (a, b) = impl_tee!(self.write(buf));
        Ok(std::cmp::max(a, b))
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        impl_tee!(self.flush());
        Ok(())
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        let (a, b) = impl_tee!(self.write_vectored(bufs));
        Ok(std::cmp::max(a, b))
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        impl_tee!(self.write_all(buf));
        Ok(())
    }

    #[inline]
    fn write_fmt(&mut self, fmt: std::fmt::Arguments<'_>) -> io::Result<()> {
        impl_tee!(self.write_fmt(fmt));
        Ok(())
    }
}

// === impl OrElse ===

impl<A, B> OrElse<A, B> {
    /// Combines
    pub fn new<W>(inner: A, or_else: B) -> Self
    where
        A: MakeWriter<Writer = OptionalWriter<W>>,
        B: MakeWriter,
        W: Write,
    {
        Self { inner, or_else }
    }
}

impl<A, B, W> MakeWriter for OrElse<A, B>
where
    A: MakeWriter<Writer = OptionalWriter<W>>,
    B: MakeWriter,
    W: io::Write,
{
    type Writer = EitherWriter<W, B::Writer>;

    #[inline]
    fn make_writer(&self) -> Self::Writer {
        match self.inner.make_writer() {
            EitherWriter::A(writer) => EitherWriter::A(writer),
            EitherWriter::B(_) => EitherWriter::B(self.or_else.make_writer()),
        }
    }

    #[inline]
    fn make_writer_for(&self, meta: &Metadata<'_>) -> Self::Writer {
        match self.inner.make_writer_for(meta) {
            EitherWriter::A(writer) => EitherWriter::A(writer),
            EitherWriter::B(_) => EitherWriter::B(self.or_else.make_writer_for(meta)),
        }
    }
}

// === impl ArcWriter ===

impl<W> io::Write for ArcWriter<W>
where
    for<'a> &'a W: io::Write,
{
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self.0).write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        (&*self.0).flush()
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        (&*self.0).write_vectored(bufs)
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        (&*self.0).write_all(buf)
    }

    #[inline]
    fn write_fmt(&mut self, fmt: std::fmt::Arguments<'_>) -> io::Result<()> {
        (&*self.0).write_fmt(fmt)
    }
}

// === blanket impls ===

impl<M> MakeWriterExt for M where M: MakeWriter {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::fmt::format::Format;
    use crate::fmt::test::{MockMakeWriter, MockWriter};
    use crate::fmt::Subscriber;
    use lazy_static::lazy_static;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    };
    use tracing::{debug, error, info, trace, warn, Level};
    use tracing_core::dispatcher::{self, Dispatch};

    fn test_writer<T>(make_writer: T, msg: &str, buf: &Mutex<Vec<u8>>)
    where
        T: MakeWriter + Send + Sync + 'static,
    {
        let subscriber = {
            #[cfg(feature = "ansi")]
            {
                let f = Format::default().without_time().with_ansi(false);
                Subscriber::builder()
                    .event_format(f)
                    .with_writer(make_writer)
                    .finish()
            }
            #[cfg(not(feature = "ansi"))]
            {
                let f = Format::default().without_time();
                Subscriber::builder()
                    .event_format(f)
                    .with_writer(make_writer)
                    .finish()
            }
        };
        let dispatch = Dispatch::from(subscriber);

        dispatcher::with_default(&dispatch, || {
            error!("{}", msg);
        });

        let expected = format!("ERROR {}: {}\n", module_path!(), msg);
        let actual = String::from_utf8(buf.try_lock().unwrap().to_vec()).unwrap();
        assert!(actual.contains(expected.as_str()));
    }

    fn has_lines(buf: &Mutex<Vec<u8>>, msgs: &[(tracing::Level, &str)]) {
        let actual = String::from_utf8(buf.try_lock().unwrap().to_vec()).unwrap();
        let mut expected_lines = msgs.iter();
        for line in actual.lines() {
            let line = dbg!(line).trim();
            let (level, msg) = expected_lines
                .next()
                .unwrap_or_else(|| panic!("expected no more lines, but got: {:?}", line));
            let expected = format!("{} {}: {}", level, module_path!(), msg);
            assert_eq!(line, expected.as_str());
        }
    }

    #[test]
    fn custom_writer_closure() {
        lazy_static! {
            static ref BUF: Mutex<Vec<u8>> = Mutex::new(vec![]);
        }

        let make_writer = || MockWriter::new(&BUF);
        let msg = "my custom writer closure error";
        test_writer(make_writer, msg, &BUF);
    }

    #[test]
    fn custom_writer_struct() {
        lazy_static! {
            static ref BUF: Mutex<Vec<u8>> = Mutex::new(vec![]);
        }

        let make_writer = MockMakeWriter::new(&BUF);
        let msg = "my custom writer struct error";
        test_writer(make_writer, msg, &BUF);
    }

    #[test]
    fn combinators_level_filters() {
        lazy_static! {
            static ref INFO_BUF: Mutex<Vec<u8>> = Mutex::new(vec![]);
            static ref DEBUG_BUF: Mutex<Vec<u8>> = Mutex::new(vec![]);
            static ref WARN_BUF: Mutex<Vec<u8>> = Mutex::new(vec![]);
            static ref ERR_BUF: Mutex<Vec<u8>> = Mutex::new(vec![]);
        }

        let info = MockMakeWriter::new(&INFO_BUF);
        let debug = MockMakeWriter::new(&DEBUG_BUF);
        let warn = MockMakeWriter::new(&WARN_BUF);
        let err = MockMakeWriter::new(&ERR_BUF);

        let make_writer = info
            .with_max_level(Level::INFO)
            .and(debug.with_max_level(Level::DEBUG))
            .and(warn.with_max_level(Level::WARN))
            .and(err.with_max_level(Level::ERROR));

        let c = {
            #[cfg(feature = "ansi")]
            let f = Format::default().without_time().with_ansi(false);
            #[cfg(not(feature = "ansi"))]
            let f = Format::default().without_time();
            Subscriber::builder()
                .event_format(f)
                .with_writer(make_writer)
                .with_max_level(Level::TRACE)
                .finish()
        };

        let _s = tracing::subscriber::set_default(c);

        trace!("trace");
        debug!("debug");
        info!("info");
        warn!("warn");
        error!("error");

        let all_lines = [
            (Level::TRACE, "trace"),
            (Level::DEBUG, "debug"),
            (Level::INFO, "info"),
            (Level::WARN, "warn"),
            (Level::ERROR, "error"),
        ];

        println!("max level debug");
        has_lines(&DEBUG_BUF, &all_lines[1..]);

        println!("max level info");
        has_lines(&INFO_BUF, &all_lines[2..]);

        println!("max level warn");
        has_lines(&WARN_BUF, &all_lines[3..]);

        println!("max level error");
        has_lines(&ERR_BUF, &all_lines[4..]);
    }

    #[test]
    fn combinators_or_else() {
        lazy_static! {
            static ref SOME_BUF: Mutex<Vec<u8>> = Mutex::new(vec![]);
            static ref OR_ELSE_BUF: Mutex<Vec<u8>> = Mutex::new(vec![]);
        }

        let some = MockMakeWriter::new(&SOME_BUF);
        let or_else = MockMakeWriter::new(&OR_ELSE_BUF);

        let return_some = AtomicBool::new(true);
        let make_writer = move || {
            if return_some.swap(false, Ordering::Relaxed) {
                OptionalWriter::some(some.make_writer())
            } else {
                OptionalWriter::none()
            }
        };
        let make_writer = make_writer.or_else(or_else);
        let c = {
            #[cfg(feature = "ansi")]
            let f = Format::default().without_time().with_ansi(false);
            #[cfg(not(feature = "ansi"))]
            let f = Format::default().without_time();
            Subscriber::builder()
                .event_format(f)
                .with_writer(make_writer)
                .with_max_level(Level::TRACE)
                .finish()
        };

        let _s = tracing::subscriber::set_default(c);
        info!("hello");
        info!("world");
        info!("goodbye");

        has_lines(&SOME_BUF, &[(Level::INFO, "hello")]);
        has_lines(
            &OR_ELSE_BUF,
            &[(Level::INFO, "world"), (Level::INFO, "goodbye")],
        );
    }

    #[test]
    fn combinators_or_else_chain() {
        lazy_static! {
            static ref INFO_BUF: Mutex<Vec<u8>> = Mutex::new(vec![]);
            static ref DEBUG_BUF: Mutex<Vec<u8>> = Mutex::new(vec![]);
            static ref WARN_BUF: Mutex<Vec<u8>> = Mutex::new(vec![]);
            static ref ERR_BUF: Mutex<Vec<u8>> = Mutex::new(vec![]);
        }

        let info = MockMakeWriter::new(&INFO_BUF);
        let debug = MockMakeWriter::new(&DEBUG_BUF);
        let warn = MockMakeWriter::new(&WARN_BUF);
        let err = MockMakeWriter::new(&ERR_BUF);

        let make_writer = err.with_max_level(Level::ERROR).or_else(
            warn.with_max_level(Level::WARN).or_else(
                info.with_max_level(Level::INFO)
                    .or_else(debug.with_max_level(Level::DEBUG)),
            ),
        );

        let c = {
            #[cfg(feature = "ansi")]
            let f = Format::default().without_time().with_ansi(false);
            #[cfg(not(feature = "ansi"))]
            let f = Format::default().without_time();
            Subscriber::builder()
                .event_format(f)
                .with_writer(make_writer)
                .with_max_level(Level::TRACE)
                .finish()
        };

        let _s = tracing::subscriber::set_default(c);

        trace!("trace");
        debug!("debug");
        info!("info");
        warn!("warn");
        error!("error");

        println!("max level debug");
        has_lines(&DEBUG_BUF, &[(Level::DEBUG, "debug")]);

        println!("max level info");
        has_lines(&INFO_BUF, &[(Level::INFO, "info")]);

        println!("max level warn");
        has_lines(&WARN_BUF, &[(Level::WARN, "warn")]);

        println!("max level error");
        has_lines(&ERR_BUF, &[(Level::ERROR, "error")]);
    }

    #[test]
    fn combinators_and() {
        lazy_static! {
            static ref A_BUF: Mutex<Vec<u8>> = Mutex::new(vec![]);
            static ref B_BUF: Mutex<Vec<u8>> = Mutex::new(vec![]);
        }

        let a = MockMakeWriter::new(&A_BUF);
        let b = MockMakeWriter::new(&B_BUF);

        let lines = &[(Level::INFO, "hello"), (Level::INFO, "world")];

        let make_writer = a.and(b);
        let c = {
            #[cfg(feature = "ansi")]
            let f = Format::default().without_time().with_ansi(false);
            #[cfg(not(feature = "ansi"))]
            let f = Format::default().without_time();
            Subscriber::builder()
                .event_format(f)
                .with_writer(make_writer)
                .with_max_level(Level::TRACE)
                .finish()
        };

        let _s = tracing::subscriber::set_default(c);
        info!("hello");
        info!("world");

        has_lines(&A_BUF, &lines[..]);
        has_lines(&B_BUF, &lines[..]);
    }
}
