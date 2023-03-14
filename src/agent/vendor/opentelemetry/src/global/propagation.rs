use crate::propagation::TextMapPropagator;
use crate::sdk::propagation::TextMapCompositePropagator;
use std::sync::RwLock;

lazy_static::lazy_static! {
    /// The current global `TextMapPropagator` propagator.
    static ref GLOBAL_TEXT_MAP_PROPAGATOR: RwLock<Box<dyn TextMapPropagator + Send + Sync>> = RwLock::new(Box::new(TextMapCompositePropagator::new(vec![])));
    /// The global default `TextMapPropagator` propagator.
    static ref DEFAULT_TEXT_MAP_PROPAGATOR: TextMapCompositePropagator = TextMapCompositePropagator::new(vec![]);
}

/// Sets the given [`TextMapPropagator`] propagator as the current global propagator.
///
/// # Examples
///
/// ```
/// use opentelemetry::{global, sdk::propagation::TraceContextPropagator};
///
/// // create your text map propagator
/// let propagator = TraceContextPropagator::new();
///
/// // assign it as the global propagator
/// global::set_text_map_propagator(propagator);
/// ```
pub fn set_text_map_propagator<P: TextMapPropagator + Send + Sync + 'static>(propagator: P) {
    let _lock = GLOBAL_TEXT_MAP_PROPAGATOR
        .write()
        .map(|mut global_propagator| *global_propagator = Box::new(propagator));
}

/// Executes a closure with a reference to the current global [`TextMapPropagator`] propagator.
///
/// # Examples
///
/// ```
/// use opentelemetry::{propagation::TextMapPropagator, global};
/// use opentelemetry::sdk::propagation::TraceContextPropagator;
/// use std::collections::HashMap;
///
/// let example_carrier = HashMap::new();
///
/// // create your text map propagator
/// let tc_propagator = TraceContextPropagator::new();
/// global::set_text_map_propagator(tc_propagator);
///
/// // use the global text map propagator to extract contexts
/// let _cx = global::get_text_map_propagator(|propagator| propagator.extract(&example_carrier));
/// ```
pub fn get_text_map_propagator<T, F>(mut f: F) -> T
where
    F: FnMut(&dyn TextMapPropagator) -> T,
{
    GLOBAL_TEXT_MAP_PROPAGATOR
        .read()
        .map(|propagator| f(&**propagator))
        .unwrap_or_else(|_| f(&*DEFAULT_TEXT_MAP_PROPAGATOR as &dyn TextMapPropagator))
}
