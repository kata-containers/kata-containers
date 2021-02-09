use crate::alloc::Box;
use crate::backtrace::Backtrace;
use crate::chain::Chain;
use crate::{Error, StdError};
use core::any::TypeId;
use core::fmt::{self, Debug, Display};
use core::mem::{self, ManuallyDrop};
use core::ptr::{self, NonNull};

#[cfg(feature = "std")]
use core::ops::{Deref, DerefMut};

impl Error {
    /// Create a new error object from any error type.
    ///
    /// The error type must be threadsafe and `'static`, so that the `Error`
    /// will be as well.
    ///
    /// If the error type does not provide a backtrace, a backtrace will be
    /// created here to ensure that a backtrace exists.
    #[cfg(feature = "std")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "std")))]
    pub fn new<E>(error: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        let backtrace = backtrace_if_absent!(error);
        Error::from_std(error, backtrace)
    }

    /// Create a new error object from a printable error message.
    ///
    /// If the argument implements std::error::Error, prefer `Error::new`
    /// instead which preserves the underlying error's cause chain and
    /// backtrace. If the argument may or may not implement std::error::Error
    /// now or in the future, use `anyhow!(err)` which handles either way
    /// correctly.
    ///
    /// `Error::msg("...")` is equivalent to `anyhow!("...")` but occasionally
    /// convenient in places where a function is preferable over a macro, such
    /// as iterator or stream combinators:
    ///
    /// ```
    /// # mod ffi {
    /// #     pub struct Input;
    /// #     pub struct Output;
    /// #     pub async fn do_some_work(_: Input) -> Result<Output, &'static str> {
    /// #         unimplemented!()
    /// #     }
    /// # }
    /// #
    /// # use ffi::{Input, Output};
    /// #
    /// use anyhow::{Error, Result};
    /// use futures::stream::{Stream, StreamExt, TryStreamExt};
    ///
    /// async fn demo<S>(stream: S) -> Result<Vec<Output>>
    /// where
    ///     S: Stream<Item = Input>,
    /// {
    ///     stream
    ///         .then(ffi::do_some_work) // returns Result<Output, &str>
    ///         .map_err(Error::msg)
    ///         .try_collect()
    ///         .await
    /// }
    /// ```
    pub fn msg<M>(message: M) -> Self
    where
        M: Display + Debug + Send + Sync + 'static,
    {
        Error::from_adhoc(message, backtrace!())
    }

    #[cfg(feature = "std")]
    pub(crate) fn from_std<E>(error: E, backtrace: Option<Backtrace>) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        let vtable = &ErrorVTable {
            object_drop: object_drop::<E>,
            object_ref: object_ref::<E>,
            object_mut: object_mut::<E>,
            object_boxed: object_boxed::<E>,
            object_downcast: object_downcast::<E>,
            object_drop_rest: object_drop_front::<E>,
        };

        // Safety: passing vtable that operates on the right type E.
        unsafe { Error::construct(error, vtable, backtrace) }
    }

    pub(crate) fn from_adhoc<M>(message: M, backtrace: Option<Backtrace>) -> Self
    where
        M: Display + Debug + Send + Sync + 'static,
    {
        use crate::wrapper::MessageError;
        let error: MessageError<M> = MessageError(message);
        let vtable = &ErrorVTable {
            object_drop: object_drop::<MessageError<M>>,
            object_ref: object_ref::<MessageError<M>>,
            #[cfg(feature = "std")]
            object_mut: object_mut::<MessageError<M>>,
            object_boxed: object_boxed::<MessageError<M>>,
            object_downcast: object_downcast::<M>,
            object_drop_rest: object_drop_front::<M>,
        };

        // Safety: MessageError is repr(transparent) so it is okay for the
        // vtable to allow casting the MessageError<M> to M.
        unsafe { Error::construct(error, vtable, backtrace) }
    }

    pub(crate) fn from_display<M>(message: M, backtrace: Option<Backtrace>) -> Self
    where
        M: Display + Send + Sync + 'static,
    {
        use crate::wrapper::DisplayError;
        let error: DisplayError<M> = DisplayError(message);
        let vtable = &ErrorVTable {
            object_drop: object_drop::<DisplayError<M>>,
            object_ref: object_ref::<DisplayError<M>>,
            #[cfg(feature = "std")]
            object_mut: object_mut::<DisplayError<M>>,
            object_boxed: object_boxed::<DisplayError<M>>,
            object_downcast: object_downcast::<M>,
            object_drop_rest: object_drop_front::<M>,
        };

        // Safety: DisplayError is repr(transparent) so it is okay for the
        // vtable to allow casting the DisplayError<M> to M.
        unsafe { Error::construct(error, vtable, backtrace) }
    }

    #[cfg(feature = "std")]
    pub(crate) fn from_context<C, E>(context: C, error: E, backtrace: Option<Backtrace>) -> Self
    where
        C: Display + Send + Sync + 'static,
        E: StdError + Send + Sync + 'static,
    {
        let error: ContextError<C, E> = ContextError { context, error };

        let vtable = &ErrorVTable {
            object_drop: object_drop::<ContextError<C, E>>,
            object_ref: object_ref::<ContextError<C, E>>,
            object_mut: object_mut::<ContextError<C, E>>,
            object_boxed: object_boxed::<ContextError<C, E>>,
            object_downcast: context_downcast::<C, E>,
            object_drop_rest: context_drop_rest::<C, E>,
        };

        // Safety: passing vtable that operates on the right type.
        unsafe { Error::construct(error, vtable, backtrace) }
    }

    #[cfg(feature = "std")]
    pub(crate) fn from_boxed(
        error: Box<dyn StdError + Send + Sync>,
        backtrace: Option<Backtrace>,
    ) -> Self {
        use crate::wrapper::BoxedError;
        let error = BoxedError(error);
        let vtable = &ErrorVTable {
            object_drop: object_drop::<BoxedError>,
            object_ref: object_ref::<BoxedError>,
            object_mut: object_mut::<BoxedError>,
            object_boxed: object_boxed::<BoxedError>,
            object_downcast: object_downcast::<Box<dyn StdError + Send + Sync>>,
            object_drop_rest: object_drop_front::<Box<dyn StdError + Send + Sync>>,
        };

        // Safety: BoxedError is repr(transparent) so it is okay for the vtable
        // to allow casting to Box<dyn StdError + Send + Sync>.
        unsafe { Error::construct(error, vtable, backtrace) }
    }

    // Takes backtrace as argument rather than capturing it here so that the
    // user sees one fewer layer of wrapping noise in the backtrace.
    //
    // Unsafe because the given vtable must have sensible behavior on the error
    // value of type E.
    unsafe fn construct<E>(
        error: E,
        vtable: &'static ErrorVTable,
        backtrace: Option<Backtrace>,
    ) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        let inner = Box::new(ErrorImpl {
            vtable,
            backtrace,
            _object: error,
        });
        // Erase the concrete type of E from the compile-time type system. This
        // is equivalent to the safe unsize coersion from Box<ErrorImpl<E>> to
        // Box<ErrorImpl<dyn StdError + Send + Sync + 'static>> except that the
        // result is a thin pointer. The necessary behavior for manipulating the
        // underlying ErrorImpl<E> is preserved in the vtable provided by the
        // caller rather than a builtin fat pointer vtable.
        let erased = mem::transmute::<Box<ErrorImpl<E>>, Box<ErrorImpl<()>>>(inner);
        let inner = ManuallyDrop::new(erased);
        Error { inner }
    }

    /// Wrap the error value with additional context.
    ///
    /// For attaching context to a `Result` as it is propagated, the
    /// [`Context`][crate::Context] extension trait may be more convenient than
    /// this function.
    ///
    /// The primary reason to use `error.context(...)` instead of
    /// `result.context(...)` via the `Context` trait would be if the context
    /// needs to depend on some data held by the underlying error:
    ///
    /// ```
    /// # use std::fmt::{self, Debug, Display};
    /// #
    /// # type T = ();
    /// #
    /// # impl std::error::Error for ParseError {}
    /// # impl Debug for ParseError {
    /// #     fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    /// #         unimplemented!()
    /// #     }
    /// # }
    /// # impl Display for ParseError {
    /// #     fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    /// #         unimplemented!()
    /// #     }
    /// # }
    /// #
    /// use anyhow::Result;
    /// use std::fs::File;
    /// use std::path::Path;
    ///
    /// struct ParseError {
    ///     line: usize,
    ///     column: usize,
    /// }
    ///
    /// fn parse_impl(file: File) -> Result<T, ParseError> {
    ///     # const IGNORE: &str = stringify! {
    ///     ...
    ///     # };
    ///     # unimplemented!()
    /// }
    ///
    /// pub fn parse(path: impl AsRef<Path>) -> Result<T> {
    ///     let file = File::open(&path)?;
    ///     parse_impl(file).map_err(|error| {
    ///         let context = format!(
    ///             "only the first {} lines of {} are valid",
    ///             error.line, path.as_ref().display(),
    ///         );
    ///         anyhow::Error::new(error).context(context)
    ///     })
    /// }
    /// ```
    pub fn context<C>(self, context: C) -> Self
    where
        C: Display + Send + Sync + 'static,
    {
        let error: ContextError<C, Error> = ContextError {
            context,
            error: self,
        };

        let vtable = &ErrorVTable {
            object_drop: object_drop::<ContextError<C, Error>>,
            object_ref: object_ref::<ContextError<C, Error>>,
            #[cfg(feature = "std")]
            object_mut: object_mut::<ContextError<C, Error>>,
            object_boxed: object_boxed::<ContextError<C, Error>>,
            object_downcast: context_chain_downcast::<C>,
            object_drop_rest: context_chain_drop_rest::<C>,
        };

        // As the cause is anyhow::Error, we already have a backtrace for it.
        let backtrace = None;

        // Safety: passing vtable that operates on the right type.
        unsafe { Error::construct(error, vtable, backtrace) }
    }

    /// Get the backtrace for this Error.
    ///
    /// Backtraces are only available on the nightly channel. Tracking issue:
    /// [rust-lang/rust#53487][tracking].
    ///
    /// In order for the backtrace to be meaningful, one of the two environment
    /// variables `RUST_LIB_BACKTRACE=1` or `RUST_BACKTRACE=1` must be defined
    /// and `RUST_LIB_BACKTRACE` must not be `0`. Backtraces are somewhat
    /// expensive to capture in Rust, so we don't necessarily want to be
    /// capturing them all over the place all the time.
    ///
    /// - If you want panics and errors to both have backtraces, set
    ///   `RUST_BACKTRACE=1`;
    /// - If you want only errors to have backtraces, set
    ///   `RUST_LIB_BACKTRACE=1`;
    /// - If you want only panics to have backtraces, set `RUST_BACKTRACE=1` and
    ///   `RUST_LIB_BACKTRACE=0`.
    ///
    /// [tracking]: https://github.com/rust-lang/rust/issues/53487
    #[cfg(backtrace)]
    pub fn backtrace(&self) -> &Backtrace {
        self.inner.backtrace()
    }

    /// An iterator of the chain of source errors contained by this Error.
    ///
    /// This iterator will visit every error in the cause chain of this error
    /// object, beginning with the error that this error object was created
    /// from.
    ///
    /// # Example
    ///
    /// ```
    /// use anyhow::Error;
    /// use std::io;
    ///
    /// pub fn underlying_io_error_kind(error: &Error) -> Option<io::ErrorKind> {
    ///     for cause in error.chain() {
    ///         if let Some(io_error) = cause.downcast_ref::<io::Error>() {
    ///             return Some(io_error.kind());
    ///         }
    ///     }
    ///     None
    /// }
    /// ```
    #[cfg(feature = "std")]
    pub fn chain(&self) -> Chain {
        self.inner.chain()
    }

    /// The lowest level cause of this error &mdash; this error's cause's
    /// cause's cause etc.
    ///
    /// The root cause is the last error in the iterator produced by
    /// [`chain()`][Error::chain].
    #[cfg(feature = "std")]
    pub fn root_cause(&self) -> &(dyn StdError + 'static) {
        self.chain().last().unwrap()
    }

    /// Returns true if `E` is the type held by this error object.
    ///
    /// For errors with context, this method returns true if `E` matches the
    /// type of the context `C` **or** the type of the error on which the
    /// context has been attached. For details about the interaction between
    /// context and downcasting, [see here].
    ///
    /// [see here]: trait.Context.html#effect-on-downcasting
    pub fn is<E>(&self) -> bool
    where
        E: Display + Debug + Send + Sync + 'static,
    {
        self.downcast_ref::<E>().is_some()
    }

    /// Attempt to downcast the error object to a concrete type.
    pub fn downcast<E>(self) -> Result<E, Self>
    where
        E: Display + Debug + Send + Sync + 'static,
    {
        let target = TypeId::of::<E>();
        unsafe {
            // Use vtable to find NonNull<()> which points to a value of type E
            // somewhere inside the data structure.
            let addr = match (self.inner.vtable.object_downcast)(&self.inner, target) {
                Some(addr) => addr,
                None => return Err(self),
            };

            // Prepare to read E out of the data structure. We'll drop the rest
            // of the data structure separately so that E is not dropped.
            let outer = ManuallyDrop::new(self);

            // Read E from where the vtable found it.
            let error = ptr::read(addr.cast::<E>().as_ptr());

            // Read Box<ErrorImpl<()>> from self. Can't move it out because
            // Error has a Drop impl which we want to not run.
            let inner = ptr::read(&outer.inner);
            let erased = ManuallyDrop::into_inner(inner);

            // Drop rest of the data structure outside of E.
            (erased.vtable.object_drop_rest)(erased, target);

            Ok(error)
        }
    }

    /// Downcast this error object by reference.
    ///
    /// # Example
    ///
    /// ```
    /// # use anyhow::anyhow;
    /// # use std::fmt::{self, Display};
    /// # use std::task::Poll;
    /// #
    /// # #[derive(Debug)]
    /// # enum DataStoreError {
    /// #     Censored(()),
    /// # }
    /// #
    /// # impl Display for DataStoreError {
    /// #     fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    /// #         unimplemented!()
    /// #     }
    /// # }
    /// #
    /// # impl std::error::Error for DataStoreError {}
    /// #
    /// # const REDACTED_CONTENT: () = ();
    /// #
    /// # let error = anyhow!("...");
    /// # let root_cause = &error;
    /// #
    /// # let ret =
    /// // If the error was caused by redaction, then return a tombstone instead
    /// // of the content.
    /// match root_cause.downcast_ref::<DataStoreError>() {
    ///     Some(DataStoreError::Censored(_)) => Ok(Poll::Ready(REDACTED_CONTENT)),
    ///     None => Err(error),
    /// }
    /// # ;
    /// ```
    pub fn downcast_ref<E>(&self) -> Option<&E>
    where
        E: Display + Debug + Send + Sync + 'static,
    {
        let target = TypeId::of::<E>();
        unsafe {
            // Use vtable to find NonNull<()> which points to a value of type E
            // somewhere inside the data structure.
            let addr = (self.inner.vtable.object_downcast)(&self.inner, target)?;
            Some(&*addr.cast::<E>().as_ptr())
        }
    }

    /// Downcast this error object by mutable reference.
    pub fn downcast_mut<E>(&mut self) -> Option<&mut E>
    where
        E: Display + Debug + Send + Sync + 'static,
    {
        let target = TypeId::of::<E>();
        unsafe {
            // Use vtable to find NonNull<()> which points to a value of type E
            // somewhere inside the data structure.
            let addr = (self.inner.vtable.object_downcast)(&self.inner, target)?;
            Some(&mut *addr.cast::<E>().as_ptr())
        }
    }
}

#[cfg(feature = "std")]
impl<E> From<E> for Error
where
    E: StdError + Send + Sync + 'static,
{
    fn from(error: E) -> Self {
        let backtrace = backtrace_if_absent!(error);
        Error::from_std(error, backtrace)
    }
}

#[cfg(feature = "std")]
impl Deref for Error {
    type Target = dyn StdError + Send + Sync + 'static;

    fn deref(&self) -> &Self::Target {
        self.inner.error()
    }
}

#[cfg(feature = "std")]
impl DerefMut for Error {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.error_mut()
    }
}

impl Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        self.inner.display(formatter)
    }
}

impl Debug for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        self.inner.debug(formatter)
    }
}

impl Drop for Error {
    fn drop(&mut self) {
        unsafe {
            // Read Box<ErrorImpl<()>> from self.
            let inner = ptr::read(&self.inner);
            let erased = ManuallyDrop::into_inner(inner);

            // Invoke the vtable's drop behavior.
            (erased.vtable.object_drop)(erased);
        }
    }
}

struct ErrorVTable {
    object_drop: unsafe fn(Box<ErrorImpl<()>>),
    object_ref: unsafe fn(&ErrorImpl<()>) -> &(dyn StdError + Send + Sync + 'static),
    #[cfg(feature = "std")]
    object_mut: unsafe fn(&mut ErrorImpl<()>) -> &mut (dyn StdError + Send + Sync + 'static),
    object_boxed: unsafe fn(Box<ErrorImpl<()>>) -> Box<dyn StdError + Send + Sync + 'static>,
    object_downcast: unsafe fn(&ErrorImpl<()>, TypeId) -> Option<NonNull<()>>,
    object_drop_rest: unsafe fn(Box<ErrorImpl<()>>, TypeId),
}

// Safety: requires layout of *e to match ErrorImpl<E>.
unsafe fn object_drop<E>(e: Box<ErrorImpl<()>>) {
    // Cast back to ErrorImpl<E> so that the allocator receives the correct
    // Layout to deallocate the Box's memory.
    let unerased = mem::transmute::<Box<ErrorImpl<()>>, Box<ErrorImpl<E>>>(e);
    drop(unerased);
}

// Safety: requires layout of *e to match ErrorImpl<E>.
unsafe fn object_drop_front<E>(e: Box<ErrorImpl<()>>, target: TypeId) {
    // Drop the fields of ErrorImpl other than E as well as the Box allocation,
    // without dropping E itself. This is used by downcast after doing a
    // ptr::read to take ownership of the E.
    let _ = target;
    let unerased = mem::transmute::<Box<ErrorImpl<()>>, Box<ErrorImpl<ManuallyDrop<E>>>>(e);
    drop(unerased);
}

// Safety: requires layout of *e to match ErrorImpl<E>.
unsafe fn object_ref<E>(e: &ErrorImpl<()>) -> &(dyn StdError + Send + Sync + 'static)
where
    E: StdError + Send + Sync + 'static,
{
    // Attach E's native StdError vtable onto a pointer to self._object.
    &(*(e as *const ErrorImpl<()> as *const ErrorImpl<E>))._object
}

// Safety: requires layout of *e to match ErrorImpl<E>.
#[cfg(feature = "std")]
unsafe fn object_mut<E>(e: &mut ErrorImpl<()>) -> &mut (dyn StdError + Send + Sync + 'static)
where
    E: StdError + Send + Sync + 'static,
{
    // Attach E's native StdError vtable onto a pointer to self._object.
    &mut (*(e as *mut ErrorImpl<()> as *mut ErrorImpl<E>))._object
}

// Safety: requires layout of *e to match ErrorImpl<E>.
unsafe fn object_boxed<E>(e: Box<ErrorImpl<()>>) -> Box<dyn StdError + Send + Sync + 'static>
where
    E: StdError + Send + Sync + 'static,
{
    // Attach ErrorImpl<E>'s native StdError vtable. The StdError impl is below.
    mem::transmute::<Box<ErrorImpl<()>>, Box<ErrorImpl<E>>>(e)
}

// Safety: requires layout of *e to match ErrorImpl<E>.
unsafe fn object_downcast<E>(e: &ErrorImpl<()>, target: TypeId) -> Option<NonNull<()>>
where
    E: 'static,
{
    if TypeId::of::<E>() == target {
        // Caller is looking for an E pointer and e is ErrorImpl<E>, take a
        // pointer to its E field.
        let unerased = e as *const ErrorImpl<()> as *const ErrorImpl<E>;
        let addr = &(*unerased)._object as *const E as *mut ();
        Some(NonNull::new_unchecked(addr))
    } else {
        None
    }
}

// Safety: requires layout of *e to match ErrorImpl<ContextError<C, E>>.
#[cfg(feature = "std")]
unsafe fn context_downcast<C, E>(e: &ErrorImpl<()>, target: TypeId) -> Option<NonNull<()>>
where
    C: 'static,
    E: 'static,
{
    if TypeId::of::<C>() == target {
        let unerased = e as *const ErrorImpl<()> as *const ErrorImpl<ContextError<C, E>>;
        let addr = &(*unerased)._object.context as *const C as *mut ();
        Some(NonNull::new_unchecked(addr))
    } else if TypeId::of::<E>() == target {
        let unerased = e as *const ErrorImpl<()> as *const ErrorImpl<ContextError<C, E>>;
        let addr = &(*unerased)._object.error as *const E as *mut ();
        Some(NonNull::new_unchecked(addr))
    } else {
        None
    }
}

// Safety: requires layout of *e to match ErrorImpl<ContextError<C, E>>.
#[cfg(feature = "std")]
unsafe fn context_drop_rest<C, E>(e: Box<ErrorImpl<()>>, target: TypeId)
where
    C: 'static,
    E: 'static,
{
    // Called after downcasting by value to either the C or the E and doing a
    // ptr::read to take ownership of that value.
    if TypeId::of::<C>() == target {
        let unerased = mem::transmute::<
            Box<ErrorImpl<()>>,
            Box<ErrorImpl<ContextError<ManuallyDrop<C>, E>>>,
        >(e);
        drop(unerased);
    } else {
        let unerased = mem::transmute::<
            Box<ErrorImpl<()>>,
            Box<ErrorImpl<ContextError<C, ManuallyDrop<E>>>>,
        >(e);
        drop(unerased);
    }
}

// Safety: requires layout of *e to match ErrorImpl<ContextError<C, Error>>.
unsafe fn context_chain_downcast<C>(e: &ErrorImpl<()>, target: TypeId) -> Option<NonNull<()>>
where
    C: 'static,
{
    if TypeId::of::<C>() == target {
        let unerased = e as *const ErrorImpl<()> as *const ErrorImpl<ContextError<C, Error>>;
        let addr = &(*unerased)._object.context as *const C as *mut ();
        Some(NonNull::new_unchecked(addr))
    } else {
        // Recurse down the context chain per the inner error's vtable.
        let unerased = e as *const ErrorImpl<()> as *const ErrorImpl<ContextError<C, Error>>;
        let source = &(*unerased)._object.error;
        (source.inner.vtable.object_downcast)(&source.inner, target)
    }
}

// Safety: requires layout of *e to match ErrorImpl<ContextError<C, Error>>.
unsafe fn context_chain_drop_rest<C>(e: Box<ErrorImpl<()>>, target: TypeId)
where
    C: 'static,
{
    // Called after downcasting by value to either the C or one of the causes
    // and doing a ptr::read to take ownership of that value.
    if TypeId::of::<C>() == target {
        let unerased = mem::transmute::<
            Box<ErrorImpl<()>>,
            Box<ErrorImpl<ContextError<ManuallyDrop<C>, Error>>>,
        >(e);
        // Drop the entire rest of the data structure rooted in the next Error.
        drop(unerased);
    } else {
        let unerased = mem::transmute::<
            Box<ErrorImpl<()>>,
            Box<ErrorImpl<ContextError<C, ManuallyDrop<Error>>>>,
        >(e);
        // Read out a ManuallyDrop<Box<ErrorImpl<()>>> from the next error.
        let inner = ptr::read(&unerased._object.error.inner);
        drop(unerased);
        let erased = ManuallyDrop::into_inner(inner);
        // Recursively drop the next error using the same target typeid.
        (erased.vtable.object_drop_rest)(erased, target);
    }
}

// repr C to ensure that E remains in the final position.
#[repr(C)]
pub(crate) struct ErrorImpl<E> {
    vtable: &'static ErrorVTable,
    backtrace: Option<Backtrace>,
    // NOTE: Don't use directly. Use only through vtable. Erased type may have
    // different alignment.
    _object: E,
}

// repr C to ensure that ContextError<C, E> has the same layout as
// ContextError<ManuallyDrop<C>, E> and ContextError<C, ManuallyDrop<E>>.
#[repr(C)]
pub(crate) struct ContextError<C, E> {
    pub context: C,
    pub error: E,
}

impl<E> ErrorImpl<E> {
    fn erase(&self) -> &ErrorImpl<()> {
        // Erase the concrete type of E but preserve the vtable in self.vtable
        // for manipulating the resulting thin pointer. This is analogous to an
        // unsize coersion.
        unsafe { &*(self as *const ErrorImpl<E> as *const ErrorImpl<()>) }
    }
}

impl ErrorImpl<()> {
    pub(crate) fn error(&self) -> &(dyn StdError + Send + Sync + 'static) {
        // Use vtable to attach E's native StdError vtable for the right
        // original type E.
        unsafe { &*(self.vtable.object_ref)(self) }
    }

    #[cfg(feature = "std")]
    pub(crate) fn error_mut(&mut self) -> &mut (dyn StdError + Send + Sync + 'static) {
        // Use vtable to attach E's native StdError vtable for the right
        // original type E.
        unsafe { &mut *(self.vtable.object_mut)(self) }
    }

    #[cfg(backtrace)]
    pub(crate) fn backtrace(&self) -> &Backtrace {
        // This unwrap can only panic if the underlying error's backtrace method
        // is nondeterministic, which would only happen in maliciously
        // constructed code.
        self.backtrace
            .as_ref()
            .or_else(|| self.error().backtrace())
            .expect("backtrace capture failed")
    }

    pub(crate) fn chain(&self) -> Chain {
        Chain::new(self.error())
    }
}

impl<E> StdError for ErrorImpl<E>
where
    E: StdError,
{
    #[cfg(backtrace)]
    fn backtrace(&self) -> Option<&Backtrace> {
        Some(self.erase().backtrace())
    }

    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.erase().error().source()
    }
}

impl<E> Debug for ErrorImpl<E>
where
    E: Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        self.erase().debug(formatter)
    }
}

impl<E> Display for ErrorImpl<E>
where
    E: Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.erase().error(), formatter)
    }
}

impl From<Error> for Box<dyn StdError + Send + Sync + 'static> {
    fn from(error: Error) -> Self {
        let outer = ManuallyDrop::new(error);
        unsafe {
            // Read Box<ErrorImpl<()>> from error. Can't move it out because
            // Error has a Drop impl which we want to not run.
            let inner = ptr::read(&outer.inner);
            let erased = ManuallyDrop::into_inner(inner);

            // Use vtable to attach ErrorImpl<E>'s native StdError vtable for
            // the right original type E.
            (erased.vtable.object_boxed)(erased)
        }
    }
}

impl From<Error> for Box<dyn StdError + Send + 'static> {
    fn from(error: Error) -> Self {
        Box::<dyn StdError + Send + Sync>::from(error)
    }
}

impl From<Error> for Box<dyn StdError + 'static> {
    fn from(error: Error) -> Self {
        Box::<dyn StdError + Send + Sync>::from(error)
    }
}

#[cfg(feature = "std")]
impl AsRef<dyn StdError + Send + Sync> for Error {
    fn as_ref(&self) -> &(dyn StdError + Send + Sync + 'static) {
        &**self
    }
}

#[cfg(feature = "std")]
impl AsRef<dyn StdError> for Error {
    fn as_ref(&self) -> &(dyn StdError + 'static) {
        &**self
    }
}
