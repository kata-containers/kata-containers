//! Utilities for configuring the `env_logger` crate to emit `tracing` events.

/// Extension trait to configure an `env_logger::Builder` to emit traces.
pub trait BuilderExt: crate::sealed::Sealed {
    /// Configure the built `env_logger::Logger` to emit `tracing` events for
    /// all consumed `log` records, rather than printing them to standard out.
    ///
    /// Note that this replaces any previously configured formatting.
    fn emit_traces(&mut self) -> &mut Self;
}

impl crate::sealed::Sealed for env_logger::Builder {}

impl BuilderExt for env_logger::Builder {
    fn emit_traces(&mut self) -> &mut Self {
        self.format(|_, record| super::format_trace(record))
    }
}

/// Attempts to initialize the global logger with an env logger configured to
/// emit `tracing` events.
///
/// This should be called early in the execution of a Rust program. Any log
/// events that occur before initialization will be ignored.
///
/// # Errors
///
/// This function will fail if it is called more than once, or if another
/// library has already initialized a global logger.
pub fn try_init() -> Result<(), log::SetLoggerError> {
    env_logger::Builder::from_default_env()
        .emit_traces()
        .try_init()
}

/// Initializes the global logger with an env logger configured to
/// emit `tracing` events.
///
/// This should be called early in the execution of a Rust program. Any log
/// events that occur before initialization will be ignored.
///
/// # Panics
///
/// This function will panic if it is called more than once, or if another
/// library has already initialized a global logger.
pub fn init() {
    try_init()
        .expect("tracing_log::env_logger::init should not be called after logger initialized");
}
