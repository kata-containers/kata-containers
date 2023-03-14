//! Formatters for event timestamps.
use std::fmt;
use std::time::Instant;

#[cfg(not(feature = "chrono"))]
mod datetime;

/// A type that can measure and format the current time.
///
/// This trait is used by `Format` to include a timestamp with each `Event` when it is logged.
///
/// Notable default implementations of this trait are `SystemTime` and `()`. The former prints the
/// current time as reported by `std::time::SystemTime`, and the latter does not print the current
/// time at all. `FormatTime` is also automatically implemented for any function pointer with the
/// appropriate signature.
///
/// The full list of provided implementations can be found in [`time`].
///
/// [`time`]: ./index.html
pub trait FormatTime {
    /// Measure and write out the current time.
    ///
    /// When `format_time` is called, implementors should get the current time using their desired
    /// mechanism, and write it out to the given `fmt::Write`. Implementors must insert a trailing
    /// space themselves if they wish to separate the time from subsequent log message text.
    fn format_time(&self, w: &mut dyn fmt::Write) -> fmt::Result;
}

/// Returns a new `SystemTime` timestamp provider.
///
/// This can then be configured further to determine how timestamps should be
/// configured.
///
/// This is equivalent to calling
/// ```rust
/// # fn timer() -> tracing_subscriber::fmt::time::SystemTime {
/// tracing_subscriber::fmt::time::SystemTime::default()
/// # }
/// ```
pub fn time() -> SystemTime {
    SystemTime::default()
}

/// Returns a new `Uptime` timestamp provider.
///
/// With this timer, timestamps will be formatted with the amount of time
/// elapsed since the timestamp provider was constructed.
///
/// This can then be configured further to determine how timestamps should be
/// configured.
///
/// This is equivalent to calling
/// ```rust
/// # fn timer() -> tracing_subscriber::fmt::time::Uptime {
/// tracing_subscriber::fmt::time::Uptime::default()
/// # }
/// ```
pub fn uptime() -> Uptime {
    Uptime::default()
}

impl<'a, F> FormatTime for &'a F
where
    F: FormatTime,
{
    fn format_time(&self, w: &mut dyn fmt::Write) -> fmt::Result {
        (*self).format_time(w)
    }
}

impl FormatTime for () {
    fn format_time(&self, _: &mut dyn fmt::Write) -> fmt::Result {
        Ok(())
    }
}

impl FormatTime for fn(&mut dyn fmt::Write) -> fmt::Result {
    fn format_time(&self, w: &mut dyn fmt::Write) -> fmt::Result {
        (*self)(w)
    }
}

/// Retrieve and print the current wall-clock time.
///
/// If the `chrono` feature is enabled, the current time is printed in a human-readable format like
/// "Jun 25 14:27:12.955". Otherwise the `Debug` implementation of `std::time::SystemTime` is used.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub struct SystemTime;

/// Retrieve and print the relative elapsed wall-clock time since an epoch.
///
/// The `Default` implementation for `Uptime` makes the epoch the current time.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Uptime {
    epoch: Instant,
}

impl Default for Uptime {
    fn default() -> Self {
        Uptime {
            epoch: Instant::now(),
        }
    }
}

impl From<Instant> for Uptime {
    fn from(epoch: Instant) -> Self {
        Uptime { epoch }
    }
}

#[cfg(feature = "chrono")]
impl FormatTime for SystemTime {
    fn format_time(&self, w: &mut dyn fmt::Write) -> fmt::Result {
        write!(w, "{}", chrono::Local::now().format("%b %d %H:%M:%S%.3f"))
    }
}

#[cfg(not(feature = "chrono"))]
impl FormatTime for SystemTime {
    fn format_time(&self, w: &mut dyn fmt::Write) -> fmt::Result {
        write!(
            w,
            "{}",
            datetime::DateTime::from(std::time::SystemTime::now())
        )
    }
}

impl FormatTime for Uptime {
    fn format_time(&self, w: &mut dyn fmt::Write) -> fmt::Result {
        let e = self.epoch.elapsed();
        write!(w, "{:4}.{:09}s", e.as_secs(), e.subsec_nanos())
    }
}

/// The RFC 3339 format is used by default and using
/// this struct allows chrono to bypass the parsing
/// used when a custom format string is provided
#[cfg(feature = "chrono")]
#[derive(Debug, Clone, Eq, PartialEq)]
enum ChronoFmtType {
    Rfc3339,
    Custom(String),
}

#[cfg(feature = "chrono")]
impl Default for ChronoFmtType {
    fn default() -> Self {
        ChronoFmtType::Rfc3339
    }
}

/// Retrieve and print the current UTC time.
#[cfg(feature = "chrono")]
#[cfg_attr(docsrs, doc(cfg(feature = "chrono")))]
#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct ChronoUtc {
    format: ChronoFmtType,
}

#[cfg(feature = "chrono")]
#[cfg_attr(docsrs, doc(cfg(feature = "chrono")))]
impl ChronoUtc {
    /// Format the time using the [`RFC 3339`] format
    /// (a subset of [`ISO 8601`]).
    ///
    /// [`RFC 3339`]: https://tools.ietf.org/html/rfc3339
    /// [`ISO 8601`]: https://en.wikipedia.org/wiki/ISO_8601
    pub fn rfc3339() -> Self {
        ChronoUtc {
            format: ChronoFmtType::Rfc3339,
        }
    }

    /// Format the time using the given format string.
    ///
    /// See [`chrono::format::strftime`]
    /// for details on the supported syntax.
    ///
    /// [`chrono::format::strftime`]: https://docs.rs/chrono/0.4.9/chrono/format/strftime/index.html
    pub fn with_format(format_string: String) -> Self {
        ChronoUtc {
            format: ChronoFmtType::Custom(format_string),
        }
    }
}

#[cfg(feature = "chrono")]
#[cfg_attr(docsrs, doc(cfg(feature = "chrono")))]
impl FormatTime for ChronoUtc {
    fn format_time(&self, w: &mut dyn fmt::Write) -> fmt::Result {
        let time = chrono::Utc::now();
        match self.format {
            ChronoFmtType::Rfc3339 => write!(w, "{}", time.to_rfc3339()),
            ChronoFmtType::Custom(ref format_str) => write!(w, "{}", time.format(format_str)),
        }
    }
}

/// Retrieve and print the current local time.
#[cfg(feature = "chrono")]
#[cfg_attr(docsrs, doc(cfg(feature = "chrono")))]
#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct ChronoLocal {
    format: ChronoFmtType,
}

#[cfg(feature = "chrono")]
#[cfg_attr(docsrs, doc(cfg(feature = "chrono")))]
impl ChronoLocal {
    /// Format the time using the [`RFC 3339`] format
    /// (a subset of [`ISO 8601`]).
    ///
    /// [`RFC 3339`]: https://tools.ietf.org/html/rfc3339
    /// [`ISO 8601`]: https://en.wikipedia.org/wiki/ISO_8601
    pub fn rfc3339() -> Self {
        ChronoLocal {
            format: ChronoFmtType::Rfc3339,
        }
    }

    /// Format the time using the given format string.
    ///
    /// See [`chrono::format::strftime`]
    /// for details on the supported syntax.
    ///
    /// [`chrono::format::strftime`]: https://docs.rs/chrono/0.4.9/chrono/format/strftime/index.html
    pub fn with_format(format_string: String) -> Self {
        ChronoLocal {
            format: ChronoFmtType::Custom(format_string),
        }
    }
}

#[cfg(feature = "chrono")]
#[cfg_attr(docsrs, doc(cfg(feature = "chrono")))]
impl FormatTime for ChronoLocal {
    fn format_time(&self, w: &mut dyn fmt::Write) -> fmt::Result {
        let time = chrono::Local::now();
        match self.format {
            ChronoFmtType::Rfc3339 => write!(w, "{}", time.to_rfc3339()),
            ChronoFmtType::Custom(ref format_str) => write!(w, "{}", time.format(format_str)),
        }
    }
}
