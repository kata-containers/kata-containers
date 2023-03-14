#![allow(missing_docs)]
use super::{field, metadata, span, Parent};

use std::fmt;

/// A mock event.
///
/// This is intended for use with the mock subscriber API in the
/// `subscriber` module.
#[derive(Default, Eq, PartialEq)]
pub struct MockEvent {
    pub fields: Option<field::Expect>,
    pub(in crate::support) parent: Option<Parent>,
    in_spans: Vec<span::MockSpan>,
    metadata: metadata::Expect,
}

pub fn mock() -> MockEvent {
    MockEvent {
        ..Default::default()
    }
}

pub fn msg(message: impl fmt::Display) -> MockEvent {
    mock().with_fields(field::msg(message))
}

impl MockEvent {
    pub fn named<I>(self, name: I) -> Self
    where
        I: Into<String>,
    {
        Self {
            metadata: metadata::Expect {
                name: Some(name.into()),
                ..self.metadata
            },
            ..self
        }
    }

    pub fn with_fields<I>(self, fields: I) -> Self
    where
        I: Into<field::Expect>,
    {
        Self {
            fields: Some(fields.into()),
            ..self
        }
    }

    pub fn at_level(self, level: tracing::Level) -> Self {
        Self {
            metadata: metadata::Expect {
                level: Some(level),
                ..self.metadata
            },
            ..self
        }
    }

    pub fn with_target<I>(self, target: I) -> Self
    where
        I: Into<String>,
    {
        Self {
            metadata: metadata::Expect {
                target: Some(target.into()),
                ..self.metadata
            },
            ..self
        }
    }

    pub fn with_explicit_parent(self, parent: Option<&str>) -> MockEvent {
        let parent = match parent {
            Some(name) => Parent::Explicit(name.into()),
            None => Parent::ExplicitRoot,
        };
        Self {
            parent: Some(parent),
            ..self
        }
    }

    pub fn check(
        &mut self,
        event: &tracing::Event<'_>,
        get_parent_name: impl FnOnce() -> Option<String>,
        subscriber_name: &str,
    ) {
        let meta = event.metadata();
        let name = meta.name();
        self.metadata
            .check(meta, format_args!("event \"{}\"", name), subscriber_name);
        assert!(
            meta.is_event(),
            "[{}] expected {}, but got {:?}",
            subscriber_name,
            self,
            event
        );
        if let Some(ref mut expected_fields) = self.fields {
            let mut checker = expected_fields.checker(name, subscriber_name);
            event.record(&mut checker);
            checker.finish();
        }

        if let Some(ref expected_parent) = self.parent {
            let actual_parent = get_parent_name();
            expected_parent.check_parent_name(
                actual_parent.as_deref(),
                event.parent().cloned(),
                event.metadata().name(),
                subscriber_name,
            )
        }
    }

    pub fn in_scope(self, spans: impl IntoIterator<Item = span::MockSpan>) -> Self {
        Self {
            in_spans: spans.into_iter().collect(),
            ..self
        }
    }

    pub fn scope_mut(&mut self) -> &mut [span::MockSpan] {
        &mut self.in_spans[..]
    }
}

impl fmt::Display for MockEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "an event{}", self.metadata)
    }
}

impl fmt::Debug for MockEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("MockEvent");

        if let Some(ref name) = self.metadata.name {
            s.field("name", name);
        }

        if let Some(ref target) = self.metadata.target {
            s.field("target", target);
        }

        if let Some(ref level) = self.metadata.level {
            s.field("level", &format_args!("{:?}", level));
        }

        if let Some(ref fields) = self.fields {
            s.field("fields", fields);
        }

        if let Some(ref parent) = self.parent {
            s.field("parent", &format_args!("{:?}", parent));
        }

        if !self.in_spans.is_empty() {
            s.field("in_spans", &self.in_spans);
        }

        s.finish()
    }
}
