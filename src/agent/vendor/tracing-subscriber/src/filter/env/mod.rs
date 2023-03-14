//! A `Layer` that enables or disables spans and events based on a set of
//! filtering directives.

// these are publicly re-exported, but the compiler doesn't realize
// that for some reason.
#[allow(unreachable_pub)]
pub use self::{directive::Directive, field::BadName as BadFieldName};
mod directive;
mod field;

use crate::{
    filter::LevelFilter,
    layer::{Context, Layer},
    sync::RwLock,
};
use directive::ParseError;
use std::{cell::RefCell, collections::HashMap, env, error::Error, fmt, str::FromStr};
use tracing_core::{
    callsite,
    field::Field,
    span,
    subscriber::{Interest, Subscriber},
    Metadata,
};

/// A [`Layer`] which filters spans and events based on a set of filter
/// directives.
///
/// # Directives
///
/// A filter consists of one or more directives which match on [`Span`]s and [`Event`]s.
/// Each directive may have a corresponding maximum verbosity [`level`] which
/// enables (e.g., _selects for_) spans and events that match. Like `log`,
/// `tracing` considers less exclusive levels (like `trace` or `info`) to be more
/// verbose than more exclusive levels (like `error` or `warn`).
///
/// The directive syntax is similar to that of [`env_logger`]'s. At a high level, the syntax for directives
/// consists of several parts:
///
/// ```text
/// target[span{field=value}]=level
/// ```
///
/// Each component (`target`, `span`, `field`, `value`, and `level`) will be covered in turn.
///
/// - `target` matches the event or span's target. In general, this is the module path and/or crate name.
///    Examples of targets `h2`, `tokio::net`, or `tide::server`. For more information on targets,
///    please refer to [`Metadata`]'s documentation.
/// - `span` matches on the span's name. If a `span` directive is provided alongside a `target`,
///    the `span` directive will match on spans _within_ the `target`.
/// - `field` matches on [fields] within spans. Field names can also be supplied without a `value`
///    and will match on any [`Span`] or [`Event`] that has a field with that name.
///    For example: `[span{field=\"value\"}]=debug`, `[{field}]=trace`.
/// - `value` matches on the value of a span's field. If a value is a numeric literal or a bool,
///    it will match _only_ on that value. Otherwise, this filter acts as a regex on
///    the `std::fmt::Debug` output from the value.
/// - `level` sets a maximum verbosity level accepted by this directive.
///
/// ## Usage Notes
///
/// - The portion of the directive which is included within the square brackets is `tracing`-specific.
/// - Any portion of the directive can be omitted.
///     - The sole exception are the `field` and `value` directives. If a `value` is provided,
///       a `field` must _also_ be provided. However, the converse does not hold, as fields can
///       be matched without a value.
/// - If only a level is provided, it will set the maximum level for all `Span`s and `Event`s
///   that are not enabled by other filters.
/// - A directive without a level will enable anything that it matches. This is equivalent to `=trace`.
/// - When a crate has a dash in its name, the default target for events will be the
///   crate's module path as it appears in Rust. This means every dash will be replaced
///   with an underscore.
/// - A dash in a target will only appear when being specified explicitly:
///   `tracing::info!(target: "target-name", ...);`
///
/// ## Examples
///
/// - `tokio::net=info` will enable all spans or events that:
///    - have the `tokio::net` target,
///    - at the level `info` or above.
/// - `my_crate[span_a]=trace` will enable all spans and events that:
///    - are within the `span_a` span or named `span_a` _if_ `span_a` has the target `my_crate`,
///    - at the level `trace` or above.
/// - `[span_b{name=\"bob\"}]` will enable all spans or event that:
///    - have _any_ target,
///    - are inside a span named `span_b`,
///    - which has a field named `name` with value `bob`,
///    - at _any_ level.
///
/// The [`Targets`] type implements a similar form of filtering, but without the
/// ability to dynamically enable events based on the current span context, and
/// without filtering on field values. When these features are not required,
/// [`Targets`] provides a lighter-weight alternative to [`EnvFilter`].
///
/// [`Layer`]: ../layer/trait.Layer.html
/// [`env_logger`]: https://docs.rs/env_logger/0.7.1/env_logger/#enabling-logging
/// [`Span`]: https://docs.rs/tracing-core/latest/tracing_core/span/index.html
/// [fields]: https://docs.rs/tracing-core/latest/tracing_core/struct.Field.html
/// [`Event`]: https://docs.rs/tracing-core/latest/tracing_core/struct.Event.html
/// [`level`]: https://docs.rs/tracing-core/latest/tracing_core/struct.Level.html
/// [`Metadata`]: https://docs.rs/tracing-core/latest/tracing_core/struct.Metadata.html
/// [`Targets`]: crate::filter::Targets
#[cfg(feature = "env-filter")]
#[cfg_attr(docsrs, doc(cfg(feature = "env-filter")))]
#[derive(Debug)]
pub struct EnvFilter {
    statics: directive::Statics,
    dynamics: directive::Dynamics,
    has_dynamics: bool,
    by_id: RwLock<HashMap<span::Id, directive::SpanMatcher>>,
    by_cs: RwLock<HashMap<callsite::Identifier, directive::CallsiteMatcher>>,
}

thread_local! {
    static SCOPE: RefCell<Vec<LevelFilter>> = RefCell::new(Vec::new());
}

type FieldMap<T> = HashMap<Field, T>;

/// Indicates that an error occurred while parsing a `EnvFilter` from an
/// environment variable.
#[cfg_attr(docsrs, doc(cfg(feature = "env-filter")))]
#[derive(Debug)]
pub struct FromEnvError {
    kind: ErrorKind,
}

#[derive(Debug)]
enum ErrorKind {
    Parse(ParseError),
    Env(env::VarError),
}

impl EnvFilter {
    /// `RUST_LOG` is the default environment variable used by
    /// [`EnvFilter::from_default_env`] and [`EnvFilter::try_from_default_env`].
    ///
    /// [`EnvFilter::from_default_env`]: #method.from_default_env
    /// [`EnvFilter::try_from_default_env`]: #method.try_from_default_env
    pub const DEFAULT_ENV: &'static str = "RUST_LOG";

    /// Returns a new `EnvFilter` from the value of the `RUST_LOG` environment
    /// variable, ignoring any invalid filter directives.
    pub fn from_default_env() -> Self {
        Self::from_env(Self::DEFAULT_ENV)
    }

    /// Returns a new `EnvFilter` from the value of the given environment
    /// variable, ignoring any invalid filter directives.
    pub fn from_env<A: AsRef<str>>(env: A) -> Self {
        env::var(env.as_ref()).map(Self::new).unwrap_or_default()
    }

    /// Returns a new `EnvFilter` from the directives in the given string,
    /// ignoring any that are invalid.
    pub fn new<S: AsRef<str>>(dirs: S) -> Self {
        let directives = dirs.as_ref().split(',').filter_map(|s| match s.parse() {
            Ok(d) => Some(d),
            Err(err) => {
                eprintln!("ignoring `{}`: {}", s, err);
                None
            }
        });
        Self::from_directives(directives)
    }

    /// Returns a new `EnvFilter` from the directives in the given string,
    /// or an error if any are invalid.
    pub fn try_new<S: AsRef<str>>(dirs: S) -> Result<Self, directive::ParseError> {
        let directives = dirs
            .as_ref()
            .split(',')
            .map(|s| s.parse())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::from_directives(directives))
    }

    /// Returns a new `EnvFilter` from the value of the `RUST_LOG` environment
    /// variable, or an error if the environment variable contains any invalid
    /// filter directives.
    pub fn try_from_default_env() -> Result<Self, FromEnvError> {
        Self::try_from_env(Self::DEFAULT_ENV)
    }

    /// Returns a new `EnvFilter` from the value of the given environment
    /// variable, or an error if the environment variable is unset or contains
    /// any invalid filter directives.
    pub fn try_from_env<A: AsRef<str>>(env: A) -> Result<Self, FromEnvError> {
        env::var(env.as_ref())?.parse().map_err(Into::into)
    }

    /// Add a filtering directive to this `EnvFilter`.
    ///
    /// The added directive will be used in addition to any previously set
    /// directives, either added using this method or provided when the filter
    /// is constructed.
    ///
    /// Filters may be created from [`LevelFilter`] or [`Level`], which will
    /// enable all traces at or below a certain verbosity level, or
    /// parsed from a string specifying a directive.
    ///
    /// If a filter directive is inserted that matches exactly the same spans
    /// and events as a previous filter, but sets a different level for those
    /// spans and events, the previous directive is overwritten.
    ///
    /// [`LevelFilter`]: ../filter/struct.LevelFilter.html
    /// [`Level`]: https://docs.rs/tracing-core/latest/tracing_core/struct.Level.html
    ///
    /// # Examples
    ///
    /// From [`LevelFilter`]:
    ////
    /// ```rust
    /// use tracing_subscriber::filter::{EnvFilter, LevelFilter};
    /// let mut filter = EnvFilter::from_default_env()
    ///     .add_directive(LevelFilter::INFO.into());
    /// ```
    ///
    /// Or from [`Level`]:
    ///
    /// ```rust
    /// # use tracing_subscriber::filter::{EnvFilter, LevelFilter};
    /// # use tracing::Level;
    /// let mut filter = EnvFilter::from_default_env()
    ///     .add_directive(Level::INFO.into());
    /// ```
    ////
    /// Parsed from a string:
    ////
    /// ```rust
    /// use tracing_subscriber::filter::{EnvFilter, Directive};
    ///
    /// # fn try_mk_filter() -> Result<(), Box<dyn ::std::error::Error>> {
    /// let mut filter = EnvFilter::try_from_default_env()?
    ///     .add_directive("my_crate::module=trace".parse()?)
    ///     .add_directive("my_crate::my_other_module::something=info".parse()?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_directive(mut self, directive: Directive) -> Self {
        if let Some(stat) = directive.to_static() {
            self.statics.add(stat)
        } else {
            self.has_dynamics = true;
            self.dynamics.add(directive);
        }
        self
    }

    fn from_directives(directives: impl IntoIterator<Item = Directive>) -> Self {
        use tracing::level_filters::STATIC_MAX_LEVEL;
        use tracing::Level;

        let directives: Vec<_> = directives.into_iter().collect();

        let disabled: Vec<_> = directives
            .iter()
            .filter(|directive| directive.level > STATIC_MAX_LEVEL)
            .collect();

        if !disabled.is_empty() {
            #[cfg(feature = "ansi_term")]
            use ansi_term::{Color, Style};
            // NOTE: We can't use a configured `MakeWriter` because the EnvFilter
            // has no knowledge of any underlying subscriber or subscriber, which
            // may or may not use a `MakeWriter`.
            let warn = |msg: &str| {
                #[cfg(not(feature = "ansi_term"))]
                let msg = format!("warning: {}", msg);
                #[cfg(feature = "ansi_term")]
                let msg = {
                    let bold = Style::new().bold();
                    let mut warning = Color::Yellow.paint("warning");
                    warning.style_ref_mut().is_bold = true;
                    format!("{}{} {}", warning, bold.paint(":"), bold.paint(msg))
                };
                eprintln!("{}", msg);
            };
            let ctx_prefixed = |prefix: &str, msg: &str| {
                #[cfg(not(feature = "ansi_term"))]
                let msg = format!("note: {}", msg);
                #[cfg(feature = "ansi_term")]
                let msg = {
                    let mut equal = Color::Fixed(21).paint("="); // dark blue
                    equal.style_ref_mut().is_bold = true;
                    format!(" {} {} {}", equal, Style::new().bold().paint(prefix), msg)
                };
                eprintln!("{}", msg);
            };
            let ctx_help = |msg| ctx_prefixed("help:", msg);
            let ctx_note = |msg| ctx_prefixed("note:", msg);
            let ctx = |msg: &str| {
                #[cfg(not(feature = "ansi_term"))]
                let msg = format!("note: {}", msg);
                #[cfg(feature = "ansi_term")]
                let msg = {
                    let mut pipe = Color::Fixed(21).paint("|");
                    pipe.style_ref_mut().is_bold = true;
                    format!(" {} {}", pipe, msg)
                };
                eprintln!("{}", msg);
            };
            warn("some trace filter directives would enable traces that are disabled statically");
            for directive in disabled {
                let target = if let Some(target) = &directive.target {
                    format!("the `{}` target", target)
                } else {
                    "all targets".into()
                };
                let level = directive
                    .level
                    .into_level()
                    .expect("=off would not have enabled any filters");
                ctx(&format!(
                    "`{}` would enable the {} level for {}",
                    directive, level, target
                ));
            }
            ctx_note(&format!("the static max level is `{}`", STATIC_MAX_LEVEL));
            let help_msg = || {
                let (feature, filter) = match STATIC_MAX_LEVEL.into_level() {
                    Some(Level::TRACE) => unreachable!(
                        "if the max level is trace, no static filtering features are enabled"
                    ),
                    Some(Level::DEBUG) => ("max_level_debug", Level::TRACE),
                    Some(Level::INFO) => ("max_level_info", Level::DEBUG),
                    Some(Level::WARN) => ("max_level_warn", Level::INFO),
                    Some(Level::ERROR) => ("max_level_error", Level::WARN),
                    None => return ("max_level_off", String::new()),
                };
                (feature, format!("{} ", filter))
            };
            let (feature, earlier_level) = help_msg();
            ctx_help(&format!(
                "to enable {}logging, remove the `{}` feature",
                earlier_level, feature
            ));
        }

        let (dynamics, mut statics) = Directive::make_tables(directives);
        let has_dynamics = !dynamics.is_empty();

        if statics.is_empty() && !has_dynamics {
            statics.add(directive::StaticDirective::default());
        }

        Self {
            statics,
            dynamics,
            has_dynamics,
            by_id: RwLock::new(HashMap::new()),
            by_cs: RwLock::new(HashMap::new()),
        }
    }

    fn cares_about_span(&self, span: &span::Id) -> bool {
        let spans = try_lock!(self.by_id.read(), else return false);
        spans.contains_key(span)
    }

    fn base_interest(&self) -> Interest {
        if self.has_dynamics {
            Interest::sometimes()
        } else {
            Interest::never()
        }
    }
}

impl<S: Subscriber> Layer<S> for EnvFilter {
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        if self.has_dynamics && metadata.is_span() {
            // If this metadata describes a span, first, check if there is a
            // dynamic filter that should be constructed for it. If so, it
            // should always be enabled, since it influences filtering.
            if let Some(matcher) = self.dynamics.matcher(metadata) {
                let mut by_cs = try_lock!(self.by_cs.write(), else return self.base_interest());
                by_cs.insert(metadata.callsite(), matcher);
                return Interest::always();
            }
        }

        // Otherwise, check if any of our static filters enable this metadata.
        if self.statics.enabled(metadata) {
            Interest::always()
        } else {
            self.base_interest()
        }
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        if self.dynamics.has_value_filters() {
            // If we perform any filtering on span field *values*, we will
            // enable *all* spans, because their field values are not known
            // until recording.
            return Some(LevelFilter::TRACE);
        }
        std::cmp::max(
            self.statics.max_level.into(),
            self.dynamics.max_level.into(),
        )
    }

    fn enabled(&self, metadata: &Metadata<'_>, _: Context<'_, S>) -> bool {
        let level = metadata.level();

        // is it possible for a dynamic filter directive to enable this event?
        // if not, we can avoid the thread local access + iterating over the
        // spans in the current scope.
        if self.has_dynamics && self.dynamics.max_level >= *level {
            if metadata.is_span() {
                // If the metadata is a span, see if we care about its callsite.
                let enabled_by_cs = self
                    .by_cs
                    .read()
                    .ok()
                    .map(|by_cs| by_cs.contains_key(&metadata.callsite()))
                    .unwrap_or(false);
                if enabled_by_cs {
                    return true;
                }
            }

            let enabled_by_scope = SCOPE.with(|scope| {
                for filter in scope.borrow().iter() {
                    if filter >= level {
                        return true;
                    }
                }
                false
            });
            if enabled_by_scope {
                return true;
            }
        }

        // is it possible for a static filter directive to enable this event?
        if self.statics.max_level >= *level {
            // Otherwise, fall back to checking if the callsite is
            // statically enabled.
            return self.statics.enabled(metadata);
        }

        false
    }

    fn new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, _: Context<'_, S>) {
        let by_cs = try_lock!(self.by_cs.read());
        if let Some(cs) = by_cs.get(&attrs.metadata().callsite()) {
            let span = cs.to_span_match(attrs);
            try_lock!(self.by_id.write()).insert(id.clone(), span);
        }
    }

    fn on_record(&self, id: &span::Id, values: &span::Record<'_>, _: Context<'_, S>) {
        if let Some(span) = try_lock!(self.by_id.read()).get(id) {
            span.record_update(values);
        }
    }

    fn on_enter(&self, id: &span::Id, _: Context<'_, S>) {
        // XXX: This is where _we_ could push IDs to the stack instead, and use
        // that to allow changing the filter while a span is already entered.
        // But that might be much less efficient...
        if let Some(span) = try_lock!(self.by_id.read()).get(id) {
            SCOPE.with(|scope| scope.borrow_mut().push(span.level()));
        }
    }

    fn on_exit(&self, id: &span::Id, _: Context<'_, S>) {
        if self.cares_about_span(id) {
            SCOPE.with(|scope| scope.borrow_mut().pop());
        }
    }

    fn on_close(&self, id: span::Id, _: Context<'_, S>) {
        // If we don't need to acquire a write lock, avoid doing so.
        if !self.cares_about_span(&id) {
            return;
        }

        let mut spans = try_lock!(self.by_id.write());
        spans.remove(&id);
    }
}

impl FromStr for EnvFilter {
    type Err = directive::ParseError;

    fn from_str(spec: &str) -> Result<Self, Self::Err> {
        Self::try_new(spec)
    }
}

impl<S> From<S> for EnvFilter
where
    S: AsRef<str>,
{
    fn from(s: S) -> Self {
        Self::new(s)
    }
}

impl Default for EnvFilter {
    fn default() -> Self {
        Self::from_directives(std::iter::empty())
    }
}

impl fmt::Display for EnvFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut statics = self.statics.iter();
        let wrote_statics = if let Some(next) = statics.next() {
            fmt::Display::fmt(next, f)?;
            for directive in statics {
                write!(f, ",{}", directive)?;
            }
            true
        } else {
            false
        };

        let mut dynamics = self.dynamics.iter();
        if let Some(next) = dynamics.next() {
            if wrote_statics {
                f.write_str(",")?;
            }
            fmt::Display::fmt(next, f)?;
            for directive in dynamics {
                write!(f, ",{}", directive)?;
            }
        }
        Ok(())
    }
}

// ===== impl FromEnvError =====

impl From<directive::ParseError> for FromEnvError {
    fn from(p: directive::ParseError) -> Self {
        Self {
            kind: ErrorKind::Parse(p),
        }
    }
}

impl From<env::VarError> for FromEnvError {
    fn from(v: env::VarError) -> Self {
        Self {
            kind: ErrorKind::Env(v),
        }
    }
}

impl fmt::Display for FromEnvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ErrorKind::Parse(ref p) => p.fmt(f),
            ErrorKind::Env(ref e) => e.fmt(f),
        }
    }
}

impl Error for FromEnvError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self.kind {
            ErrorKind::Parse(ref p) => Some(p),
            ErrorKind::Env(ref e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_core::field::FieldSet;
    use tracing_core::*;

    struct NoSubscriber;
    impl Subscriber for NoSubscriber {
        #[inline]
        fn register_callsite(&self, _: &'static Metadata<'static>) -> subscriber::Interest {
            subscriber::Interest::always()
        }
        fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
            span::Id::from_u64(0xDEAD)
        }
        fn event(&self, _event: &Event<'_>) {}
        fn record(&self, _span: &span::Id, _values: &span::Record<'_>) {}
        fn record_follows_from(&self, _span: &span::Id, _follows: &span::Id) {}

        #[inline]
        fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
            true
        }
        fn enter(&self, _span: &span::Id) {}
        fn exit(&self, _span: &span::Id) {}
    }

    struct Cs;
    impl Callsite for Cs {
        fn set_interest(&self, _interest: Interest) {}
        fn metadata(&self) -> &Metadata<'_> {
            unimplemented!()
        }
    }

    #[test]
    fn callsite_enabled_no_span_directive() {
        let filter = EnvFilter::new("app=debug").with_subscriber(NoSubscriber);
        static META: &Metadata<'static> = &Metadata::new(
            "mySpan",
            "app",
            Level::TRACE,
            None,
            None,
            None,
            FieldSet::new(&[], identify_callsite!(&Cs)),
            Kind::SPAN,
        );

        let interest = filter.register_callsite(META);
        assert!(interest.is_never());
    }

    #[test]
    fn callsite_off() {
        let filter = EnvFilter::new("app=off").with_subscriber(NoSubscriber);
        static META: &Metadata<'static> = &Metadata::new(
            "mySpan",
            "app",
            Level::ERROR,
            None,
            None,
            None,
            FieldSet::new(&[], identify_callsite!(&Cs)),
            Kind::SPAN,
        );

        let interest = filter.register_callsite(META);
        assert!(interest.is_never());
    }

    #[test]
    fn callsite_enabled_includes_span_directive() {
        let filter = EnvFilter::new("app[mySpan]=debug").with_subscriber(NoSubscriber);
        static META: &Metadata<'static> = &Metadata::new(
            "mySpan",
            "app",
            Level::TRACE,
            None,
            None,
            None,
            FieldSet::new(&[], identify_callsite!(&Cs)),
            Kind::SPAN,
        );

        let interest = filter.register_callsite(META);
        assert!(interest.is_always());
    }

    #[test]
    fn callsite_enabled_includes_span_directive_field() {
        let filter =
            EnvFilter::new("app[mySpan{field=\"value\"}]=debug").with_subscriber(NoSubscriber);
        static META: &Metadata<'static> = &Metadata::new(
            "mySpan",
            "app",
            Level::TRACE,
            None,
            None,
            None,
            FieldSet::new(&["field"], identify_callsite!(&Cs)),
            Kind::SPAN,
        );

        let interest = filter.register_callsite(META);
        assert!(interest.is_always());
    }

    #[test]
    fn callsite_enabled_includes_span_directive_multiple_fields() {
        let filter = EnvFilter::new("app[mySpan{field=\"value\",field2=2}]=debug")
            .with_subscriber(NoSubscriber);
        static META: &Metadata<'static> = &Metadata::new(
            "mySpan",
            "app",
            Level::TRACE,
            None,
            None,
            None,
            FieldSet::new(&["field"], identify_callsite!(&Cs)),
            Kind::SPAN,
        );

        let interest = filter.register_callsite(META);
        assert!(interest.is_never());
    }

    #[test]
    fn roundtrip() {
        let f1: EnvFilter =
            "[span1{foo=1}]=error,[span2{bar=2 baz=false}],crate2[{quux=\"quuux\"}]=debug"
                .parse()
                .unwrap();
        let f2: EnvFilter = format!("{}", f1).parse().unwrap();
        assert_eq!(f1.statics, f2.statics);
        assert_eq!(f1.dynamics, f2.dynamics);
    }

    #[test]
    fn size_of_filters() {
        fn print_sz(s: &str) {
            let filter = s.parse::<EnvFilter>().expect("filter should parse");
            println!(
                "size_of_val({:?})\n -> {}B",
                s,
                std::mem::size_of_val(&filter)
            );
        }

        print_sz("info");

        print_sz("foo=debug");

        print_sz(
            "crate1::mod1=error,crate1::mod2=warn,crate1::mod2::mod3=info,\
            crate2=debug,crate3=trace,crate3::mod2::mod1=off",
        );

        print_sz("[span1{foo=1}]=error,[span2{bar=2 baz=false}],crate2[{quux=\"quuux\"}]=debug");

        print_sz(
            "crate1::mod1=error,crate1::mod2=warn,crate1::mod2::mod3=info,\
            crate2=debug,crate3=trace,crate3::mod2::mod1=off,[span1{foo=1}]=error,\
            [span2{bar=2 baz=false}],crate2[{quux=\"quuux\"}]=debug",
        );
    }
}
