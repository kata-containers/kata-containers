//! Process resource detector
//!
//! Detect process related information like pid, executable name.

use crate::sdk::resource::ResourceDetector;
use crate::sdk::Resource;
use crate::{Array, KeyValue, Value};
use std::borrow::Cow;
use std::env::args_os;
use std::process::id;
use std::time::Duration;

/// Detect process information.
///
/// This resource detector returns the following information:
///
/// - process command line arguments(`process.command_args`), the full command arguments of this
/// application.
/// - OS assigned process id(`process.pid`).
#[derive(Debug)]
pub struct ProcessResourceDetector;

impl ResourceDetector for ProcessResourceDetector {
    fn detect(&self, _timeout: Duration) -> Resource {
        let arguments = args_os();
        let cmd_arg_val = arguments
            .into_iter()
            .map(|arg| Cow::from(arg.to_string_lossy().into_owned()))
            .collect::<Vec<Cow<'_, str>>>();
        Resource::new(vec![
            KeyValue::new(
                "process.command_args",
                Value::Array(Array::String(cmd_arg_val)),
            ),
            KeyValue::new("process.pid", id() as i64),
        ])
    }
}

#[cfg(target_os = "linux")]
#[cfg(test)]
mod tests {
    use crate::sdk::resource::{ProcessResourceDetector, ResourceDetector};
    use std::time::Duration;

    #[test]
    fn test_processor_resource_detector() {
        let resource = ProcessResourceDetector.detect(Duration::from_secs(0));
        assert_eq!(resource.len(), 2); // we cannot assert on the values because it changes along with runtime.
    }
}
