use crate::fmt::{format::Writer, time::FormatTime, writer::WriteAdaptor};
use std::fmt;
use time::{format_description::well_known, formatting::Formattable, OffsetDateTime};

/// Formats the current [local time] using a [formatter] from the [`time` crate].
///
/// To format the current [UTC time] instead, use the [`UtcTime`] type.
///
/// [local time]: https://docs.rs/time/0.3/time/struct.OffsetDateTime.html#method.now_local
/// [UTC time]: https://docs.rs/time/0.3/time/struct.OffsetDateTime.html#method.now_utc
/// [formatter]: https://docs.rs/time/0.3/time/formatting/trait.Formattable.html
/// [`time` crate]: https://docs.rs/time/0.3/time/
#[derive(Clone, Debug)]
#[cfg_attr(docsrs, doc(cfg(all(feature = "time", feature = "local-time"))))]
#[cfg(feature = "local-time")]
pub struct LocalTime<F> {
    format: F,
}

/// Formats the current [UTC time] using a [formatter] from the [`time` crate].
///
/// To format the current [local time] instead, use the [`LocalTime`] type.
///
/// [local time]: https://docs.rs/time/0.3/time/struct.OffsetDateTime.html#method.now_local
/// [UTC time]: https://docs.rs/time/0.3/time/struct.OffsetDateTime.html#method.now_utc
/// [formatter]: https://docs.rs/time/0.3/time/formatting/trait.Formattable.html
/// [`time` crate]: https://docs.rs/time/0.3/time/
#[cfg_attr(docsrs, doc(cfg(feature = "time")))]
#[derive(Clone, Debug)]
pub struct UtcTime<F> {
    format: F,
}

// === impl LocalTime ===

#[cfg(feature = "local-time")]
impl LocalTime<well_known::Rfc3339> {
    /// Returns a formatter that formats the current [local time] in the
    /// [RFC 3339] format (a subset of the [ISO 8601] timestamp format).
    ///
    /// # Examples
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time};
    ///
    /// let collector = tracing_subscriber::fmt()
    ///     .with_timer(time::LocalTime::rfc_3339());
    /// # drop(collector);
    /// ```
    ///
    /// [local time]: https://docs.rs/time/0.3/time/struct.OffsetDateTime.html#method.now_local
    /// [RFC 3339]: https://datatracker.ietf.org/doc/html/rfc3339
    /// [ISO 8601]: https://en.wikipedia.org/wiki/ISO_8601
    pub fn rfc_3339() -> Self {
        Self::new(well_known::Rfc3339)
    }
}

#[cfg(feature = "local-time")]
impl<F: Formattable> LocalTime<F> {
    /// Returns a formatter that formats the current [local time] using the
    /// [`time` crate] with the provided provided format. The format may be any
    /// type that implements the [`Formattable`] trait.
    ///
    /// Typically, the format will be a format description string, or one of the
    /// `time` crate's [well-known formats].
    ///
    /// If the format description is statically known, then the
    /// [`format_description!`] macro should be used. This is identical to the
    /// [`time::format_description::parse`] method, but runs at compile-time,
    /// throwing an error if the format description is invalid. If the desired format
    /// is not known statically (e.g., a user is providing a format string), then the
    /// [`time::format_description::parse`] method should be used. Note that this
    /// method is fallible.
    ///
    /// See the [`time` book] for details on the format description syntax.
    ///
    /// # Examples
    ///
    /// Using the [`format_description!`] macro:
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time::LocalTime};
    /// use time::macros::format_description;
    ///
    /// let timer = LocalTime::new(format_description!("[hour]:[minute]:[second]"));
    /// let collector = tracing_subscriber::fmt()
    ///     .with_timer(timer);
    /// # drop(collector);
    /// ```
    ///
    /// Using [`time::format_description::parse`]:
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time::LocalTime};
    ///
    /// let time_format = time::format_description::parse("[hour]:[minute]:[second]")
    ///     .expect("format string should be valid!");
    /// let timer = LocalTime::new(time_format);
    /// let collector = tracing_subscriber::fmt()
    ///     .with_timer(timer);
    /// # drop(collector);
    /// ```
    ///
    /// Using the [`format_description!`] macro requires enabling the `time`
    /// crate's "macros" feature flag.
    ///
    /// Using a [well-known format][well-known formats] (this is equivalent to
    /// [`LocalTime::rfc_3339`]):
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time::LocalTime};
    ///
    /// let timer = LocalTime::new(time::format_description::well_known::Rfc3339);
    /// let collector = tracing_subscriber::fmt()
    ///     .with_timer(timer);
    /// # drop(collector);
    /// ```
    ///
    /// [local time]: https://docs.rs/time/latest/time/struct.OffsetDateTime.html#method.now_local
    /// [`time` crate]: https://docs.rs/time/0.3/time/
    /// [`Formattable`]: https://docs.rs/time/0.3/time/formatting/trait.Formattable.html
    /// [well-known formats]: https://docs.rs/time/0.3/time/format_description/well_known/index.html
    /// [`format_description!`]: https://docs.rs/time/0.3/time/macros/macro.format_description.html
    /// [`time::format_description::parse`]: https://docs.rs/time/0.3/time/format_description/fn.parse.html
    /// [`time` book]: https://time-rs.github.io/book/api/format-description.html
    pub fn new(format: F) -> Self {
        Self { format }
    }
}

#[cfg(feature = "local-time")]
impl<F> FormatTime for LocalTime<F>
where
    F: Formattable,
{
    fn format_time(&self, w: &mut Writer<'_>) -> fmt::Result {
        let now = OffsetDateTime::now_local().map_err(|_| fmt::Error)?;
        format_datetime(now, w, &self.format)
    }
}

#[cfg(feature = "local-time")]
impl<F> Default for LocalTime<F>
where
    F: Formattable + Default,
{
    fn default() -> Self {
        Self::new(F::default())
    }
}

// === impl UtcTime ===

impl UtcTime<well_known::Rfc3339> {
    /// Returns a formatter that formats the current [UTC time] in the
    /// [RFC 3339] format, which is a subset of the [ISO 8601] timestamp format.
    ///
    /// # Examples
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time};
    ///
    /// let collector = tracing_subscriber::fmt()
    ///     .with_timer(time::UtcTime::rfc_3339());
    /// # drop(collector);
    /// ```
    ///
    /// [local time]: https://docs.rs/time/0.3/time/struct.OffsetDateTime.html#method.now_utc
    /// [RFC 3339]: https://datatracker.ietf.org/doc/html/rfc3339
    /// [ISO 8601]: https://en.wikipedia.org/wiki/ISO_8601
    pub fn rfc_3339() -> Self {
        Self::new(well_known::Rfc3339)
    }
}

impl<F: Formattable> UtcTime<F> {
    /// Returns a formatter that formats the current [UTC time] using the
    /// [`time` crate], with the provided provided format. The format may be any
    /// type that implements the [`Formattable`] trait.
    ///
    /// Typically, the format will be a format description string, or one of the
    /// `time` crate's [well-known formats].
    ///
    /// If the format description is statically known, then the
    /// [`format_description!`] macro should be used. This is identical to the
    /// [`time::format_description::parse`] method, but runs at compile-time,
    /// failing  an error if the format description is invalid. If the desired format
    /// is not known statically (e.g., a user is providing a format string), then the
    /// [`time::format_description::parse`] method should be used. Note that this
    /// method is fallible.
    ///
    /// See the [`time` book] for details on the format description syntax.
    ///
    /// # Examples
    ///
    /// Using the [`format_description!`] macro:
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time::UtcTime};
    /// use time::macros::format_description;
    ///
    /// let timer = UtcTime::new(format_description!("[hour]:[minute]:[second]"));
    /// let collector = tracing_subscriber::fmt()
    ///     .with_timer(timer);
    /// # drop(collector);
    /// ```
    ///
    /// Using the [`format_description!`] macro requires enabling the `time`
    /// crate's "macros" feature flag.
    ///
    /// Using [`time::format_description::parse`]:
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time::UtcTime};
    ///
    /// let time_format = time::format_description::parse("[hour]:[minute]:[second]")
    ///     .expect("format string should be valid!");
    /// let timer = UtcTime::new(time_format);
    /// let collector = tracing_subscriber::fmt()
    ///     .with_timer(timer);
    /// # drop(collector);
    /// ```
    ///
    /// Using a [well-known format][well-known formats] (this is equivalent to
    /// [`UtcTime::rfc_3339`]):
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time::UtcTime};
    ///
    /// let timer = UtcTime::new(time::format_description::well_known::Rfc3339);
    /// let collector = tracing_subscriber::fmt()
    ///     .with_timer(timer);
    /// # drop(collector);
    /// ```
    ///
    /// [UTC time]: https://docs.rs/time/latest/time/struct.OffsetDateTime.html#method.now_utc
    /// [`time` crate]: https://docs.rs/time/0.3/time/
    /// [`Formattable`]: https://docs.rs/time/0.3/time/formatting/trait.Formattable.html
    /// [well-known formats]: https://docs.rs/time/0.3/time/format_description/well_known/index.html
    /// [`format_description!`]: https://docs.rs/time/0.3/time/macros/macro.format_description.html
    /// [`time::format_description::parse`]: https://docs.rs/time/0.3/time/format_description/fn.parse.html
    /// [`time` book]: https://time-rs.github.io/book/api/format-description.html
    pub fn new(format: F) -> Self {
        Self { format }
    }
}

impl<F> FormatTime for UtcTime<F>
where
    F: Formattable,
{
    fn format_time(&self, w: &mut Writer<'_>) -> fmt::Result {
        format_datetime(OffsetDateTime::now_utc(), w, &self.format)
    }
}

impl<F> Default for UtcTime<F>
where
    F: Formattable + Default,
{
    fn default() -> Self {
        Self::new(F::default())
    }
}

fn format_datetime(
    now: OffsetDateTime,
    into: &mut Writer<'_>,
    fmt: &impl Formattable,
) -> fmt::Result {
    let mut into = WriteAdaptor::new(into);
    now.format_into(&mut into, fmt)
        .map_err(|_| fmt::Error)
        .map(|_| ())
}
