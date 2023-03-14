//! OS resource detector
//!
//! Detect the runtime operating system type.
use crate::sdk::resource::ResourceDetector;
use crate::sdk::Resource;
use crate::KeyValue;
use std::env::consts::OS;
use std::time::Duration;

/// Detect runtime operating system information.
///
/// This detector uses Rust's [`OS constant`] to detect the operating system type and
/// maps the result to the supported value defined in [`OpenTelemetry spec`].
///
/// [`OS constant`]: https://doc.rust-lang.org/std/env/consts/constant.OS.html
/// [`OpenTelemetry spec`]: https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/resource/semantic_conventions/os.md
#[derive(Debug)]
pub struct OsResourceDetector;

impl ResourceDetector for OsResourceDetector {
    fn detect(&self, _timeout: Duration) -> Resource {
        Resource::new(vec![KeyValue::new("os.type", OS)])
    }
}

#[cfg(target_os = "linux")]
#[cfg(test)]
mod tests {
    use crate::sdk::resource::os::OsResourceDetector;
    use crate::sdk::resource::ResourceDetector;
    use crate::Key;
    use std::time::Duration;

    #[test]
    fn test_os_resource_detector() {
        let resource = OsResourceDetector.detect(Duration::from_secs(0));
        assert_eq!(
            resource
                .iter()
                .0
                .find(|(k, _v)| **k == Key::from_static_str("os.type"))
                .map(|(_k, v)| v.to_string()),
            Some("linux".to_string())
        );
    }
}
