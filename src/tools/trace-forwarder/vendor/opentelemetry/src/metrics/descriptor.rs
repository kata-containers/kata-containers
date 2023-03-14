use crate::metrics::{InstrumentConfig, InstrumentKind, NumberKind};
use crate::sdk::InstrumentationLibrary;
use fnv::FnvHasher;
use std::hash::{Hash, Hasher};

/// Descriptor contains all the settings that describe an instrument, including
/// its name, metric kind, number kind, and the configurable options.
#[derive(Clone, Debug, PartialEq)]
pub struct Descriptor {
    name: String,
    instrument_kind: InstrumentKind,
    number_kind: NumberKind,
    pub(crate) config: InstrumentConfig,
    attribute_hash: u64,
}

impl Descriptor {
    /// Create a new descriptor
    pub fn new(
        name: String,
        instrumentation_name: &'static str,
        instrumentation_version: Option<&'static str>,
        instrument_kind: InstrumentKind,
        number_kind: NumberKind,
    ) -> Self {
        let mut hasher = FnvHasher::default();
        name.hash(&mut hasher);
        instrumentation_name.hash(&mut hasher);
        instrumentation_version.hash(&mut hasher);
        instrument_kind.hash(&mut hasher);
        number_kind.hash(&mut hasher);

        Descriptor {
            name,
            instrument_kind,
            number_kind,
            config: InstrumentConfig::with_instrumentation(
                instrumentation_name,
                instrumentation_version,
            ),
            attribute_hash: hasher.finish(),
        }
    }

    /// The metric instrument's name.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// The specific kind of instrument.
    pub fn instrument_kind(&self) -> &InstrumentKind {
        &self.instrument_kind
    }

    /// NumberKind returns whether this instrument is declared over int64, float64, or uint64
    /// values.
    pub fn number_kind(&self) -> &NumberKind {
        &self.number_kind
    }

    /// A human-readable description of the metric instrument.
    pub fn description(&self) -> Option<&String> {
        self.config.description.as_ref()
    }

    /// Assign a new description
    pub fn set_description(&mut self, description: String) {
        self.config.description = Some(description);
    }

    /// Unit describes the units of the metric instrument.
    pub fn unit(&self) -> Option<&str> {
        self.config.unit.as_ref().map(|unit| unit.as_ref())
    }

    /// The name of the library that provided instrumentation for this instrument.
    pub fn instrumentation_name(&self) -> &'static str {
        self.config.instrumentation_name()
    }

    /// The version of library that provided instrumentation for this instrument. Optional
    pub fn instrumentation_version(&self) -> Option<&'static str> {
        self.config.instrumentation_library.version
    }

    /// Instrumentation library reference
    pub fn instrumentation_library(&self) -> &InstrumentationLibrary {
        &self.config.instrumentation_library
    }

    /// The pre-computed hash of the descriptor data
    pub fn attribute_hash(&self) -> u64 {
        self.attribute_hash
    }
}
