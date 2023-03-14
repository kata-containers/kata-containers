//! # OpenTelemetry Trace Link Interface
use crate::{trace::SpanContext, KeyValue};
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};

/// During the `Span` creation user MUST have the ability to record links to other `Span`s. Linked
/// `Span`s can be from the same or a different trace.
#[cfg_attr(feature = "serialize", derive(Deserialize, Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    span_context: SpanContext,
    pub(crate) attributes: Vec<KeyValue>,
    pub(crate) dropped_attributes_count: u32,
}

impl Link {
    /// Create a new link
    pub fn new(span_context: SpanContext, attributes: Vec<KeyValue>) -> Self {
        Link {
            span_context,
            attributes,
            dropped_attributes_count: 0,
        }
    }

    /// The span context of the linked span
    pub fn span_context(&self) -> &SpanContext {
        &self.span_context
    }

    /// Attributes of the span link
    pub fn attributes(&self) -> &Vec<KeyValue> {
        &self.attributes
    }

    /// Dropped attributes count
    pub fn dropped_attributes_count(&self) -> u32 {
        self.dropped_attributes_count
    }
}
