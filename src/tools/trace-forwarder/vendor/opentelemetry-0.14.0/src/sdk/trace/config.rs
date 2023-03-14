//! SDK Configuration
//!
//! Configuration represents the global tracing configuration, overrides
//! can be set for the default OpenTelemetry limits and Sampler.
use crate::sdk::trace::span_limit::SpanLimits;
use crate::{sdk, sdk::trace::Sampler, trace::IdGenerator};
use std::env;
use std::str::FromStr;
use std::sync::Arc;

/// Default trace configuration
pub fn config() -> Config {
    Config::default()
}

/// Tracer configuration
#[derive(Debug)]
pub struct Config {
    /// The sampler that the sdk should use
    pub sampler: Box<dyn sdk::trace::ShouldSample>,
    /// The id generator that the sdk should use
    pub id_generator: Box<dyn IdGenerator>,
    /// span limits
    pub span_limits: SpanLimits,
    /// Contains attributes representing an entity that produces telemetry.
    pub resource: Option<Arc<sdk::Resource>>,
}

impl Config {
    /// Specify the sampler to be used.
    pub fn with_sampler<T: sdk::trace::ShouldSample + 'static>(mut self, sampler: T) -> Self {
        self.sampler = Box::new(sampler);
        self
    }

    /// Specify the id generator to be used.
    pub fn with_id_generator<T: IdGenerator + 'static>(mut self, id_generator: T) -> Self {
        self.id_generator = Box::new(id_generator);
        self
    }

    /// Specify the number of events to be recorded per span.
    pub fn with_max_events_per_span(mut self, max_events: u32) -> Self {
        self.span_limits.max_events_per_span = max_events;
        self
    }

    /// Specify the number of attributes to be recorded per span.
    pub fn with_max_attributes_per_span(mut self, max_attributes: u32) -> Self {
        self.span_limits.max_attributes_per_span = max_attributes;
        self
    }

    /// Specify the number of events to be recorded per span.
    pub fn with_max_links_per_span(mut self, max_links: u32) -> Self {
        self.span_limits.max_links_per_span = max_links;
        self
    }

    /// Specify the number of attributes one event can have.
    pub fn with_max_attributes_per_event(mut self, max_attributes: u32) -> Self {
        self.span_limits.max_attributes_per_event = max_attributes;
        self
    }

    /// Specify the number of attributes one link can have.
    pub fn with_max_attributes_per_link(mut self, max_attributes: u32) -> Self {
        self.span_limits.max_attributes_per_link = max_attributes;
        self
    }

    /// Specify all limit via the span_limits
    pub fn with_span_limits(mut self, span_limits: SpanLimits) -> Self {
        self.span_limits = span_limits;
        self
    }

    /// Specify the attributes representing the entity that produces telemetry
    pub fn with_resource(mut self, resource: sdk::Resource) -> Self {
        self.resource = Some(Arc::new(resource));
        self
    }
}

impl Default for Config {
    /// Create default global sdk configuration.
    fn default() -> Self {
        let mut config = Config {
            sampler: Box::new(Sampler::ParentBased(Box::new(Sampler::AlwaysOn))),
            id_generator: Box::new(sdk::trace::IdGenerator::default()),
            span_limits: SpanLimits::default(),
            resource: None,
        };

        if let Some(max_attributes_per_span) = env::var("OTEL_SPAN_ATTRIBUTE_COUNT_LIMIT")
            .ok()
            .and_then(|count_limit| u32::from_str(&count_limit).ok())
        {
            config.span_limits.max_attributes_per_span = max_attributes_per_span;
        }

        if let Some(max_events_per_span) = env::var("OTEL_SPAN_EVENT_COUNT_LIMIT")
            .ok()
            .and_then(|max_events| u32::from_str(&max_events).ok())
        {
            config.span_limits.max_events_per_span = max_events_per_span;
        }

        if let Some(max_links_per_span) = env::var("OTEL_SPAN_LINK_COUNT_LIMIT")
            .ok()
            .and_then(|max_links| u32::from_str(&max_links).ok())
        {
            config.span_limits.max_links_per_span = max_links_per_span;
        }

        config
    }
}
