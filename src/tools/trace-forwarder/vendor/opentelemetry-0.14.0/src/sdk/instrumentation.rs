//! Provides instrumentation information for both tracing and metric.
//! See `OTEPS-0083` for details.
//!
//! [OTEPS-0083](https://github.com/open-telemetry/oteps/blob/master/text/0083-component.md)

/// InstrumentationLibrary contains information about instrumentation library.
///
/// See `Instrumentation Libraries` for more information.
///
/// [`Instrumentation Libraries`](https://github.com/open-telemetry/opentelemetry-specification/blob/master/specification/overview.md#instrumentation-libraries)
#[derive(Debug, Default, Hash, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct InstrumentationLibrary {
    /// instrumentation library name, cannot be empty
    pub name: &'static str,
    /// instrumentation library version, can be empty
    pub version: Option<&'static str>,
}

impl InstrumentationLibrary {
    /// Create an InstrumentationLibrary from name and version.
    pub fn new(name: &'static str, version: Option<&'static str>) -> InstrumentationLibrary {
        InstrumentationLibrary { name, version }
    }
}
