use core::fmt::{self, Debug, Display};

use Fail;

without_std! {
    /// An error with context around it.
    ///
    /// The context is intended to be a human-readable, user-facing explanation for the
    /// error that has occurred. The underlying error is not assumed to be end-user-relevant
    /// information.
    ///
    /// The `Display` impl for `Context` only prints the human-readable context, while the
    /// `Debug` impl also prints the underlying error.
    pub struct Context<D: Display + Send + Sync + 'static> {
        context: D,
    }

    impl<D: Display + Send + Sync + 'static> Context<D> {
        /// Creates a new context without an underlying error message.
        pub fn new(context: D) -> Context<D> {
            Context { context }
        }

        /// Returns a reference to the context provided with this error.
        pub fn get_context(&self) -> &D {
            &self.context
        }

        /// Maps `Context<D>` to `Context<T>` by applying a function to the contained context.
        pub fn map<F, T>(self, op: F) -> Context<T>
            where F: FnOnce(D) -> T,
                  T: Display + Send + Sync + 'static
        {
            Context {
                context: op(self.context),
            }
        }

        pub(crate) fn with_err<E: Fail>(context: D, _: E) -> Context<D> {
            Context { context }
        }
    }

    impl<D: Display + Send + Sync + 'static> Fail for Context<D> { }

    impl<D: Display + Send + Sync + 'static> Debug for Context<D> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{}", self.context)
        }
    }

    impl<D: Display + Send + Sync + 'static> Display for Context<D> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{}", self.context)
        }
    }

    #[test]
    fn test_map() {
        let ctx = Context::new("a string").map(|s| format!("{} with some more stuff", s));
        assert_eq!(ctx.context, String::from("a string with some more stuff"));
    }
}

with_std! {
    use {Error, Backtrace};

    /// An error with context around it.
    ///
    /// The context is intended to be a human-readable, user-facing explanation for the
    /// error that has occurred. The underlying error is not assumed to be end-user-relevant
    /// information.
    ///
    /// The `Display` impl for `Context` only prints the human-readable context, while the
    /// `Debug` impl also prints the underlying error.
    pub struct Context<D: Display + Send + Sync + 'static> {
        context: D,
        failure: Either<Backtrace, Error>,
    }

    impl<D: Display + Send + Sync + 'static> Context<D> {
        /// Creates a new context without an underlying error message.
        pub fn new(context: D) -> Context<D> {
            let failure = Either::This(Backtrace::new());
            Context { context, failure }
        }

        /// Returns a reference to the context provided with this error.
        pub fn get_context(&self) -> &D {
            &self.context
        }

        /// Maps `Context<D>` to `Context<T>` by applying a function to the contained context.
        pub fn map<F, T>(self, op: F) -> Context<T>
            where F: FnOnce(D) -> T,
                  T: Display + Send + Sync + 'static
        {
            Context {
                context: op(self.context),
                failure: self.failure,
            }
        }

        pub(crate) fn with_err<E: Into<Error>>(context: D, error: E) -> Context<D> {
            let failure = Either::That(error.into());
            Context { context, failure }
        }
    }

    impl<D: Display + Send + Sync + 'static> Fail for Context<D> {
        fn name(&self) -> Option<&str> {
            self.failure.as_cause().and_then(|x| x.name())
        }

        fn cause(&self) -> Option<&dyn Fail> {
            self.failure.as_cause()
        }

        fn backtrace(&self) -> Option<&Backtrace> {
            Some(self.failure.backtrace())
        }
    }

    impl<D: Display + Send + Sync + 'static> Debug for Context<D> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{:?}\n\n{}", self.failure, self.context)
        }
    }

    impl<D: Display + Send + Sync + 'static> Display for Context<D> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{}", self.context)
        }
    }

    enum Either<A, B> {
        This(A),
        That(B),
    }

    impl Either<Backtrace, Error> {
        fn backtrace(&self) -> &Backtrace {
            match *self {
                Either::This(ref backtrace) => backtrace,
                Either::That(ref error)     => error.backtrace(),
            }
        }

        fn as_cause(&self) -> Option<&dyn Fail> {
            match *self {
                Either::This(_)         => None,
                Either::That(ref error) => Some(error.as_fail())
            }
        }
    }

    impl Debug for Either<Backtrace, Error> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match *self {
                Either::This(ref backtrace) => write!(f, "{:?}", backtrace),
                Either::That(ref error)     => write!(f, "{:?}", error),
            }
        }
    }

    #[test]
    fn test_map() {
        let ctx = Context::new("a string").map(|s| format!("{} with some more stuff", s));
        assert_eq!(ctx.context, String::from("a string with some more stuff"));
    }
}

impl<D> From<D> for Context<D>
where
    D: Display + Send + Sync + 'static,
{
    fn from(display: D) -> Context<D> {
        Context::new(display)
    }
}
