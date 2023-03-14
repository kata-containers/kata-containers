//! # OpenTelemetry Trace SDK
//!
//! The tracing SDK consist of a few main structs:
//!
//! * The `Tracer` struct which performs all tracing operations.
//! * The `Span` struct with is a mutable object storing information about the
//! current operation execution.
//! * The `TracerProvider` struct which configures and produces `Tracer`s.
mod config;
mod evicted_hash_map;
mod evicted_queue;
mod id_generator;
mod provider;
mod runtime;
mod sampler;
mod span;
mod span_limit;
mod span_processor;
mod tracer;

pub use config::{config, Config};
pub use evicted_hash_map::EvictedHashMap;
pub use evicted_queue::EvictedQueue;
pub use id_generator::{aws::XrayIdGenerator, IdGenerator};
pub use provider::{Builder, TracerProvider};
pub use runtime::{TraceRuntime, TrySend};
pub use sampler::{Sampler, SamplingDecision, SamplingResult, ShouldSample};
pub use span::Span;
pub use span_limit::SpanLimits;
pub use span_processor::{
    BatchConfig, BatchMessage, BatchSpanProcessor, BatchSpanProcessorBuilder, SimpleSpanProcessor,
    SpanProcessor,
};
pub use tracer::Tracer;
