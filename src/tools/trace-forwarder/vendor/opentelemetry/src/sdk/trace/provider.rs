//! # Trace Provider SDK
//!
//! ## Tracer Creation
//!
//! New `Tracer` instances are always created through a `TracerProvider`.
//!
//! All configuration objects and extension points (span processors,
//! propagators) are provided by the `TracerProvider`. `Tracer` instances do
//! not duplicate this data to avoid that different `Tracer` instances
//! of the `TracerProvider` have different versions of these data.
use crate::sdk::resource::SdkProvidedResourceDetector;
use crate::sdk::trace::runtime::TraceRuntime;
use crate::sdk::Resource;
use crate::trace::TraceResult;
use crate::{
    global,
    sdk::{self, export::trace::SpanExporter, trace::SpanProcessor},
};
use std::sync::Arc;
use std::time::Duration;

/// Default tracer name if empty string is provided.
const DEFAULT_COMPONENT_NAME: &str = "rust.opentelemetry.io/sdk/tracer";

/// TracerProvider inner type
#[derive(Debug)]
pub(crate) struct TracerProviderInner {
    processors: Vec<Box<dyn SpanProcessor>>,
    config: sdk::trace::Config,
}

impl Drop for TracerProviderInner {
    fn drop(&mut self) {
        for processor in &mut self.processors {
            if let Err(err) = processor.shutdown() {
                global::handle_error(err);
            }
        }
    }
}

/// Creator and registry of named `Tracer` instances.
#[derive(Clone, Debug)]
pub struct TracerProvider {
    inner: Arc<TracerProviderInner>,
}

impl Default for TracerProvider {
    fn default() -> Self {
        TracerProvider::builder().build()
    }
}

impl TracerProvider {
    /// Build a new tracer provider
    pub(crate) fn new(inner: Arc<TracerProviderInner>) -> Self {
        TracerProvider { inner }
    }

    /// Create a new `TracerProvider` builder.
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Span processors associated with this provider
    pub fn span_processors(&self) -> &Vec<Box<dyn SpanProcessor>> {
        &self.inner.processors
    }

    /// Config associated with this tracer
    pub fn config(&self) -> &sdk::trace::Config {
        &self.inner.config
    }
}

impl crate::trace::TracerProvider for TracerProvider {
    /// This implementation of `TracerProvider` produces `Tracer` instances.
    type Tracer = sdk::trace::Tracer;

    /// Find or create `Tracer` instance by name.
    fn tracer(&self, name: &'static str, version: Option<&'static str>) -> Self::Tracer {
        // Use default value if name is invalid empty string
        let component_name = if name.is_empty() {
            DEFAULT_COMPONENT_NAME
        } else {
            name
        };
        let instrumentation_lib = sdk::InstrumentationLibrary::new(component_name, version);

        sdk::trace::Tracer::new(instrumentation_lib, Arc::downgrade(&self.inner))
    }

    /// Force flush all remaining spans in span processors and return results.
    fn force_flush(&self) -> Vec<TraceResult<()>> {
        self.span_processors()
            .iter()
            .map(|processor| processor.force_flush())
            .collect()
    }
}

/// Builder for provider attributes.
#[derive(Default, Debug)]
pub struct Builder {
    processors: Vec<Box<dyn SpanProcessor>>,
    config: sdk::trace::Config,
}

impl Builder {
    /// The `SpanExporter` that this provider should use.
    pub fn with_simple_exporter<T: SpanExporter + 'static>(self, exporter: T) -> Self {
        let mut processors = self.processors;
        processors.push(Box::new(sdk::trace::SimpleSpanProcessor::new(Box::new(
            exporter,
        ))));

        Builder { processors, ..self }
    }

    /// The `SpanExporter` setup using a default `BatchSpanProcessor` that this provider should use.
    pub fn with_batch_exporter<T: SpanExporter + 'static, R: TraceRuntime>(
        self,
        exporter: T,
        runtime: R,
    ) -> Self {
        let batch = sdk::trace::BatchSpanProcessor::builder(exporter, runtime).build();
        self.with_span_processor(batch)
    }

    /// The `SpanProcessor` that this provider should use.
    pub fn with_span_processor<T: SpanProcessor + 'static>(self, processor: T) -> Self {
        let mut processors = self.processors;
        processors.push(Box::new(processor));

        Builder { processors, ..self }
    }

    /// The sdk `Config` that this provider will use.
    pub fn with_config(self, config: sdk::trace::Config) -> Self {
        Builder { config, ..self }
    }

    /// Create a new provider from this configuration.
    pub fn build(self) -> TracerProvider {
        let mut config = self.config;
        let sdk_provided_resource = Resource::from_detectors(
            Duration::from_secs(0),
            vec![Box::new(SdkProvidedResourceDetector)],
        );
        config.resource = match config.resource {
            None => Some(Arc::new(sdk_provided_resource)),
            Some(resource) => {
                if resource.is_empty() {
                    None
                } else {
                    Some(resource)
                }
            }
        };
        TracerProvider {
            inner: Arc::new(TracerProviderInner {
                processors: self.processors,
                config,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::sdk::export::trace::SpanData;
    use crate::sdk::trace::provider::TracerProviderInner;
    use crate::sdk::trace::{Config, Span, SpanProcessor};
    use crate::sdk::Resource;
    use crate::trace::{TraceError, TraceResult, TracerProvider};
    use crate::{Context, Key, KeyValue};
    use std::sync::Arc;

    #[derive(Debug)]
    struct TestSpanProcessor {
        success: bool,
    }

    impl SpanProcessor for TestSpanProcessor {
        fn on_start(&self, _span: &mut Span, _cx: &Context) {
            unimplemented!()
        }

        fn on_end(&self, _span: SpanData) {
            unimplemented!()
        }

        fn force_flush(&self) -> TraceResult<()> {
            if self.success {
                Ok(())
            } else {
                Err(TraceError::from("cannot export"))
            }
        }

        fn shutdown(&mut self) -> TraceResult<()> {
            self.force_flush()
        }
    }

    #[test]
    fn test_force_flush() {
        let tracer_provider = super::TracerProvider::new(Arc::from(TracerProviderInner {
            processors: vec![
                Box::from(TestSpanProcessor { success: true }),
                Box::from(TestSpanProcessor { success: false }),
            ],
            config: Default::default(),
        }));

        let results = tracer_provider.force_flush();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_tracer_provider_default_resource() {
        // If users didn't provided a resource and there isn't a env var set. Use default one
        let assert_service_name = |provider: super::TracerProvider,
                                   expect: Option<&'static str>| {
            assert_eq!(
                provider.config().resource.as_ref().and_then(|r| r
                    .get(Key::from_static_str("service.name"))
                    .map(|v| v.to_string())),
                expect.map(|s| s.to_string())
            );
        };
        let default_config_provider = super::TracerProvider::builder().build();
        assert_service_name(default_config_provider, Some("unknown_service"));

        // If user didn't provided a resource, try to get a default from env var
        let custom_config_provider = super::TracerProvider::builder()
            .with_config(Config {
                resource: Some(Arc::new(Resource::new(vec![KeyValue::new(
                    "service.name",
                    "test_service",
                )]))),
                ..Default::default()
            })
            .build();
        assert_service_name(custom_config_provider, Some("test_service"));

        // If user provided a resource, it can override everything
        let no_service_name = super::TracerProvider::builder()
            .with_config(Config {
                resource: Some(Arc::new(Resource::empty())),
                ..Default::default()
            })
            .build();

        assert_service_name(no_service_name, None);
    }
}
