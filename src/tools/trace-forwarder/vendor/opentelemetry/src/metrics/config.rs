use crate::metrics::Unit;
use crate::sdk::InstrumentationLibrary;

/// Config contains some options for metrics of any kind.
#[derive(Clone, Debug, PartialEq, Hash)]
pub struct InstrumentConfig {
    pub(crate) description: Option<String>,
    pub(crate) unit: Option<Unit>,
    pub(crate) instrumentation_library: InstrumentationLibrary,
}

impl InstrumentConfig {
    /// Create a new config from instrumentation name
    pub fn with_instrumentation_name(instrumentation_name: &'static str) -> Self {
        InstrumentConfig {
            description: None,
            unit: None,
            instrumentation_library: InstrumentationLibrary {
                name: instrumentation_name,
                version: None,
            },
        }
    }

    /// Create a new config with instrumentation name and optional version
    pub fn with_instrumentation(
        instrumentation_name: &'static str,
        instrumentation_version: Option<&'static str>,
    ) -> Self {
        InstrumentConfig {
            description: None,
            unit: None,
            instrumentation_library: InstrumentationLibrary {
                name: instrumentation_name,
                version: instrumentation_version,
            },
        }
    }

    /// Description is an optional field describing the metric instrument.
    pub fn description(&self) -> Option<&String> {
        self.description.as_ref()
    }

    /// Unit is an optional field describing the metric instrument data.
    pub fn unit(&self) -> Option<&Unit> {
        self.unit.as_ref()
    }

    /// Instrumentation name is the name given to the Meter that created this instrument.
    pub fn instrumentation_name(&self) -> &'static str {
        self.instrumentation_library.name
    }

    /// Instrumentation version returns the version of instrumentation
    pub fn instrumentation_version(&self) -> Option<&'static str> {
        self.instrumentation_library.version
    }
}
