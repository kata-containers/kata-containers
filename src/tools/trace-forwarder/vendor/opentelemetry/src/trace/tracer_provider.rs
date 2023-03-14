use crate::trace::{TraceResult, Tracer};
use std::fmt;

/// Types that can create instances of [`Tracer`].
pub trait TracerProvider: fmt::Debug + 'static {
    /// The [`Tracer`] type that this provider will return.
    type Tracer: Tracer;

    /// Returns a new tracer with the given name and version.
    ///
    /// The `name` should be the application name or the name of the library
    /// providing instrumentation. If the name is empty, then an
    /// implementation-defined default name may be used instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::{global, trace::TracerProvider};
    ///
    /// let provider = global::tracer_provider();
    ///
    /// // App tracer
    /// let tracer = provider.tracer("my_app", None);
    ///
    /// // Library tracer
    /// let tracer = provider.tracer("my_library", Some(env!("CARGO_PKG_VERSION")));
    /// ```
    fn tracer(&self, name: &'static str, version: Option<&'static str>) -> Self::Tracer;

    /// Force flush all remaining spans in span processors and return results.
    fn force_flush(&self) -> Vec<TraceResult<()>>;
}
