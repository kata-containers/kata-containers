use super::*;
use crate::{
    field::{VisitFmt, VisitOutput},
    fmt::fmt_layer::{FmtContext, FormattedFields},
    registry::LookupSpan,
};

use std::fmt::{self, Write};
use tracing_core::{
    field::{self, Field},
    Event, Level, Subscriber,
};

#[cfg(feature = "tracing-log")]
use tracing_log::NormalizeEvent;

use ansi_term::{Colour, Style};

/// An excessively pretty, human-readable event formatter.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Pretty {
    display_location: bool,
}

/// The [visitor] produced by [`Pretty`]'s [`MakeVisitor`] implementation.
///
/// [visitor]: ../../field/trait.Visit.html
/// [`DefaultFields`]: struct.DefaultFields.html
/// [`MakeVisitor`]: ../../field/trait.MakeVisitor.html
pub struct PrettyVisitor<'a> {
    writer: &'a mut dyn Write,
    is_empty: bool,
    ansi: bool,
    style: Style,
    result: fmt::Result,
}

/// An excessively pretty, human-readable [`MakeVisitor`] implementation.
///
/// [`MakeVisitor`]: crate::field::MakeVisitor
#[derive(Debug)]
pub struct PrettyFields {
    ansi: bool,
}

// === impl Pretty ===

impl Default for Pretty {
    fn default() -> Self {
        Self {
            display_location: true,
        }
    }
}

impl Pretty {
    fn style_for(level: &Level) -> Style {
        match *level {
            Level::TRACE => Style::new().fg(Colour::Purple),
            Level::DEBUG => Style::new().fg(Colour::Blue),
            Level::INFO => Style::new().fg(Colour::Green),
            Level::WARN => Style::new().fg(Colour::Yellow),
            Level::ERROR => Style::new().fg(Colour::Red),
        }
    }

    /// Sets whether the event's source code location is displayed.
    ///
    /// This defaults to `true`.
    pub fn with_source_location(self, display_location: bool) -> Self {
        Self {
            display_location,
            ..self
        }
    }
}

impl<T> Format<Pretty, T> {
    /// Sets whether or not the source code location from which an event
    /// originated is displayed.
    ///
    /// This defaults to `true`.
    pub fn with_source_location(mut self, display_location: bool) -> Self {
        self.format = self.format.with_source_location(display_location);
        self
    }
}

impl<C, N, T> FormatEvent<C, N> for Format<Pretty, T>
where
    C: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
    T: FormatTime,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, C, N>,
        writer: &mut dyn fmt::Write,
        event: &Event<'_>,
    ) -> fmt::Result {
        #[cfg(feature = "tracing-log")]
        let normalized_meta = event.normalized_metadata();
        #[cfg(feature = "tracing-log")]
        let meta = normalized_meta.as_ref().unwrap_or_else(|| event.metadata());
        #[cfg(not(feature = "tracing-log"))]
        let meta = event.metadata();
        write!(writer, "  ")?;

        self.format_timestamp(writer)?;

        let style = if self.display_level && self.ansi {
            Pretty::style_for(meta.level())
        } else {
            Style::new()
        };

        if self.display_level {
            write!(writer, "{} ", super::FmtLevel::new(meta.level(), self.ansi))?;
        }

        if self.display_target {
            let target_style = if self.ansi { style.bold() } else { style };
            write!(
                writer,
                "{}{}{}: ",
                target_style.prefix(),
                meta.target(),
                target_style.infix(style)
            )?;
        }
        let mut v = PrettyVisitor::new(writer, true)
            .with_style(style)
            .with_ansi(self.ansi);
        event.record(&mut v);
        v.finish()?;
        writer.write_char('\n')?;

        let dimmed = if self.ansi {
            Style::new().dimmed().italic()
        } else {
            Style::new()
        };
        let thread = self.display_thread_name || self.display_thread_id;
        if let (true, Some(file), Some(line)) =
            (self.format.display_location, meta.file(), meta.line())
        {
            write!(
                writer,
                "    {} {}:{}{}",
                dimmed.paint("at"),
                file,
                line,
                dimmed.paint(if thread { " " } else { "\n" })
            )?;
        } else if thread {
            write!(writer, "    ")?;
        }

        if thread {
            write!(writer, "{} ", dimmed.paint("on"))?;
            let thread = std::thread::current();
            if self.display_thread_name {
                if let Some(name) = thread.name() {
                    write!(writer, "{}", name)?;
                    if self.display_thread_id {
                        write!(writer, " ({:?})", thread.id())?;
                    }
                } else if !self.display_thread_id {
                    write!(writer, " {:?}", thread.id())?;
                }
            } else if self.display_thread_id {
                write!(writer, " {:?}", thread.id())?;
            }
            writer.write_char('\n')?;
        }

        let bold = if self.ansi {
            Style::new().bold()
        } else {
            Style::new()
        };
        let span = event
            .parent()
            .and_then(|id| ctx.span(id))
            .or_else(|| ctx.lookup_current());

        let scope = span.into_iter().flat_map(|span| span.scope());

        for span in scope {
            let meta = span.metadata();
            if self.display_target {
                write!(
                    writer,
                    "    {} {}::{}",
                    dimmed.paint("in"),
                    meta.target(),
                    bold.paint(meta.name()),
                )?;
            } else {
                write!(
                    writer,
                    "    {} {}",
                    dimmed.paint("in"),
                    bold.paint(meta.name()),
                )?;
            }

            let ext = span.extensions();
            let fields = &ext
                .get::<FormattedFields<N>>()
                .expect("Unable to find FormattedFields in extensions; this is a bug");
            if !fields.is_empty() {
                write!(writer, " {} {}", dimmed.paint("with"), fields)?;
            }
            writer.write_char('\n')?;
        }

        writer.write_char('\n')
    }
}

impl<'writer> FormatFields<'writer> for Pretty {
    fn format_fields<R: RecordFields>(
        &self,
        writer: &'writer mut dyn fmt::Write,
        fields: R,
    ) -> fmt::Result {
        let mut v = PrettyVisitor::new(writer, true);
        fields.record(&mut v);
        v.finish()
    }

    fn add_fields(&self, current: &'writer mut String, fields: &span::Record<'_>) -> fmt::Result {
        let empty = current.is_empty();
        let mut v = PrettyVisitor::new(current, empty);
        fields.record(&mut v);
        v.finish()
    }
}

// === impl PrettyFields ===

impl Default for PrettyFields {
    fn default() -> Self {
        Self::new()
    }
}

impl PrettyFields {
    /// Returns a new default [`PrettyFields`] implementation.
    pub fn new() -> Self {
        Self { ansi: true }
    }

    /// Enable ANSI encoding for formatted fields.
    pub fn with_ansi(self, ansi: bool) -> Self {
        Self { ansi, ..self }
    }
}

impl<'a> MakeVisitor<&'a mut dyn Write> for PrettyFields {
    type Visitor = PrettyVisitor<'a>;

    #[inline]
    fn make_visitor(&self, target: &'a mut dyn Write) -> Self::Visitor {
        PrettyVisitor::new(target, true).with_ansi(self.ansi)
    }
}

// === impl PrettyVisitor ===

impl<'a> PrettyVisitor<'a> {
    /// Returns a new default visitor that formats to the provided `writer`.
    ///
    /// # Arguments
    /// - `writer`: the writer to format to.
    /// - `is_empty`: whether or not any fields have been previously written to
    ///   that writer.
    pub fn new(writer: &'a mut dyn Write, is_empty: bool) -> Self {
        Self {
            writer,
            is_empty,
            ansi: true,
            style: Style::default(),
            result: Ok(()),
        }
    }

    pub(crate) fn with_style(self, style: Style) -> Self {
        Self { style, ..self }
    }

    pub(crate) fn with_ansi(self, ansi: bool) -> Self {
        Self { ansi, ..self }
    }

    fn write_padded(&mut self, value: &impl fmt::Debug) {
        let padding = if self.is_empty {
            self.is_empty = false;
            ""
        } else {
            ", "
        };
        self.result = write!(self.writer, "{}{:?}", padding, value);
    }

    fn bold(&self) -> Style {
        if self.ansi {
            self.style.bold()
        } else {
            Style::new()
        }
    }
}

impl<'a> field::Visit for PrettyVisitor<'a> {
    fn record_str(&mut self, field: &Field, value: &str) {
        if self.result.is_err() {
            return;
        }

        if field.name() == "message" {
            self.record_debug(field, &format_args!("{}", value))
        } else {
            self.record_debug(field, &value)
        }
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        if let Some(source) = value.source() {
            let bold = self.bold();
            self.record_debug(
                field,
                &format_args!(
                    "{}, {}{}.sources{}: {}",
                    value,
                    bold.prefix(),
                    field,
                    bold.infix(self.style),
                    ErrorSourceList(source),
                ),
            )
        } else {
            self.record_debug(field, &format_args!("{}", value))
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if self.result.is_err() {
            return;
        }
        let bold = self.bold();
        match field.name() {
            "message" => self.write_padded(&format_args!("{}{:?}", self.style.prefix(), value,)),
            // Skip fields that are actually log metadata that have already been handled
            #[cfg(feature = "tracing-log")]
            name if name.starts_with("log.") => self.result = Ok(()),
            name if name.starts_with("r#") => self.write_padded(&format_args!(
                "{}{}{}: {:?}",
                bold.prefix(),
                &name[2..],
                bold.infix(self.style),
                value
            )),
            name => self.write_padded(&format_args!(
                "{}{}{}: {:?}",
                bold.prefix(),
                name,
                bold.infix(self.style),
                value
            )),
        };
    }
}

impl<'a> VisitOutput<fmt::Result> for PrettyVisitor<'a> {
    fn finish(self) -> fmt::Result {
        write!(self.writer, "{}", self.style.suffix())?;
        self.result
    }
}

impl<'a> VisitFmt for PrettyVisitor<'a> {
    fn writer(&mut self) -> &mut dyn fmt::Write {
        self.writer
    }
}

impl<'a> fmt::Debug for PrettyVisitor<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrettyVisitor")
            .field("writer", &format_args!("<dyn fmt::Write>"))
            .field("is_empty", &self.is_empty)
            .field("result", &self.result)
            .field("style", &self.style)
            .field("ansi", &self.ansi)
            .finish()
    }
}
