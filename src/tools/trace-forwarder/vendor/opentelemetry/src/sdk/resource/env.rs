//! Environment variables resource detector
//!
//! Implementation of `ResourceDetector` to extract a `Resource` from environment
//! variables.
use crate::sdk::{resource::ResourceDetector, Resource};
use crate::KeyValue;
use std::env;
use std::time::Duration;

const OTEL_RESOURCE_ATTRIBUTES: &str = "OTEL_RESOURCE_ATTRIBUTES";
const OTEL_SERVICE_NAME: &str = "OTEL_SERVICE_NAME";

/// Resource detector implements ResourceDetector and is used to extract
/// general SDK configuration from environment.
///
/// See
/// [semantic conventions](https://github.com/open-telemetry/opentelemetry-specification/tree/master/specification/resource/semantic_conventions#telemetry-sdk)
/// for details.
#[derive(Debug)]
pub struct EnvResourceDetector {
    _private: (),
}

impl ResourceDetector for EnvResourceDetector {
    fn detect(&self, _timeout: Duration) -> Resource {
        match env::var(OTEL_RESOURCE_ATTRIBUTES) {
            Ok(s) if !s.is_empty() => construct_otel_resources(s),
            Ok(_) | Err(_) => Resource::new(vec![]), // return empty resource
        }
    }
}

impl EnvResourceDetector {
    /// Create `EnvResourceDetector` instance.
    pub fn new() -> Self {
        EnvResourceDetector { _private: () }
    }
}

impl Default for EnvResourceDetector {
    fn default() -> Self {
        EnvResourceDetector::new()
    }
}

/// Extract key value pairs and construct a resource from resources string like
/// key1=value1,key2=value2,...
fn construct_otel_resources(s: String) -> Resource {
    Resource::new(s.split_terminator(',').filter_map(|entry| {
        let mut parts = entry.splitn(2, '=');
        let key = parts.next()?.trim();
        let value = parts.next()?.trim();
        if value.find('=').is_some() {
            return None;
        }

        Some(KeyValue::new(key.to_owned(), value.to_owned()))
    }))
}

/// There are the attributes which MUST be provided by the SDK as specified in
/// [the Resource SDK specification]. This detector detect those attributes and
/// if the attribute cannot be detected, use the default value.
///
/// [the Resource SDK specification]:https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/resource/sdk.md#sdk-provided-resource-attributes
#[derive(Debug)]
pub struct SdkProvidedResourceDetector;

impl ResourceDetector for SdkProvidedResourceDetector {
    fn detect(&self, _timeout: Duration) -> Resource {
        Resource::new(vec![KeyValue::new(
            "service.name",
            env::var(OTEL_SERVICE_NAME)
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "unknown_service".to_string()),
        )])
    }
}

#[cfg(test)]
mod tests {
    use crate::sdk::resource::env::{
        SdkProvidedResourceDetector, OTEL_RESOURCE_ATTRIBUTES, OTEL_SERVICE_NAME,
    };
    use crate::sdk::resource::{EnvResourceDetector, Resource, ResourceDetector};
    use crate::{Key, KeyValue, Value};
    use std::time::Duration;
    use std::{env, time};

    #[test]
    fn test_read_from_env() {
        env::set_var(OTEL_RESOURCE_ATTRIBUTES, "key=value, k = v , a= x, a=z");
        env::set_var("irrelevant".to_uppercase(), "20200810");

        let detector = EnvResourceDetector::new();
        let resource = detector.detect(time::Duration::from_secs(5));
        assert_eq!(
            resource,
            Resource::new(vec![
                KeyValue::new("key", "value"),
                KeyValue::new("k", "v"),
                KeyValue::new("a", "x"),
                KeyValue::new("a", "z"),
            ])
        );

        // Test this case in same test to avoid race condition when running tests in parallel.
        env::set_var(OTEL_RESOURCE_ATTRIBUTES, "");

        let detector = EnvResourceDetector::new();
        let resource = detector.detect(time::Duration::from_secs(5));
        assert!(resource.is_empty());
    }

    #[test]
    fn test_sdk_provided_resource_detector() {
        const SERVICE_NAME: &str = "service.name";
        let no_env = SdkProvidedResourceDetector.detect(Duration::from_secs(1));
        assert_eq!(
            no_env.get(Key::from_static_str(SERVICE_NAME)),
            Some(Value::from("unknown_service")),
        );

        env::set_var(OTEL_SERVICE_NAME, "test service");
        let with_service = SdkProvidedResourceDetector.detect(Duration::from_secs(1));
        assert_eq!(
            with_service.get(Key::from_static_str(SERVICE_NAME)),
            Some(Value::from("test service")),
        );
        env::set_var(OTEL_SERVICE_NAME, ""); // clear the env var
    }
}
