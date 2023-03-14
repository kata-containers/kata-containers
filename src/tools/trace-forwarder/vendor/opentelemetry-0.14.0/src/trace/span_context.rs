//! # OpenTelemetry SpanContext interface
//!
//! A `SpanContext` represents the portion of a `Span` which must be serialized and propagated along
//! side of a distributed context. `SpanContext`s are immutable.
//!
//! The OpenTelemetry `SpanContext` representation conforms to the [w3c TraceContext specification].
//! It contains two identifiers - a `TraceId` and a `SpanId` - along with a set of common
//! `TraceFlags` and system-specific `TraceState` values.
//!
//! The spec can be viewed here: https://github.com/open-telemetry/opentelemetry-specification/blob/master/specification/api-tracing.md#SpanContext
//!
//! [w3c TraceContext specification]: https://www.w3.org/TR/trace-context/
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::str::FromStr;

/// A SpanContext with TRACE_FLAG_NOT_SAMPLED means the span is not sampled.
pub const TRACE_FLAG_NOT_SAMPLED: u8 = 0x00;
/// TRACE_FLAG_SAMPLED is a bitmask with the sampled bit set. A SpanContext
/// with the sampling bit set means the span is sampled.
pub const TRACE_FLAG_SAMPLED: u8 = 0x01;
/// TRACE_FLAGS_DEFERRED is a bitmask with the deferred bit set. A SpanContext
/// with the deferred bit set means the sampling decision has been
/// defered to the receiver.
pub const TRACE_FLAG_DEFERRED: u8 = 0x02;
/// TRACE_FLAGS_DEBUG is a bitmask with the debug bit set.
pub const TRACE_FLAG_DEBUG: u8 = 0x04;

/// TraceId is an 16-byte value which uniquely identifies a given trace
/// The actual `u128` value is wrapped in a tuple struct in order to leverage the newtype pattern
#[cfg_attr(feature = "serialize", derive(Deserialize, Serialize))]
#[derive(Clone, Debug, PartialEq, Eq, Copy, Hash)]
pub struct TraceId(u128);

impl TraceId {
    /// Construct a new invalid (zero-valued) TraceId
    pub fn invalid() -> Self {
        TraceId(0)
    }

    /// Convert from u128 to TraceId
    pub fn from_u128(item: u128) -> Self {
        TraceId(item)
    }

    /// Convert from TraceId to u128
    pub fn to_u128(self) -> u128 {
        self.0
    }

    /// Convert from TraceId to Hexadecimal String
    pub fn to_hex(self) -> String {
        format!("{:032x}", self.0)
    }

    /// Convert from TraceId to Big-Endian byte array
    pub fn to_byte_array(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }

    /// Construct a new TraceId from Hexadecimal String
    pub fn from_hex(hex: &str) -> Self {
        TraceId(u128::from_str_radix(hex, 16).unwrap_or(0))
    }

    /// Construct a new TraceId from Big-Endian byte array
    pub fn from_byte_array(byte_array: [u8; 16]) -> Self {
        TraceId(u128::from_be_bytes(byte_array))
    }
}

/// SpanId is an 8-byte value which uniquely identifies a given span within a trace
/// The actual `u64` value is wrapped in a tuple struct in order to leverage the newtype pattern
#[cfg_attr(feature = "serialize", derive(Deserialize, Serialize))]
#[derive(Clone, Debug, PartialEq, Eq, Copy, Hash)]
pub struct SpanId(u64);

impl SpanId {
    /// Construct a new invalid (zero-valued) SpanId
    pub fn invalid() -> Self {
        SpanId(0)
    }

    /// Convert from u64 to SpanId
    pub fn from_u64(item: u64) -> Self {
        SpanId(item)
    }

    /// Convert from SpanId to u64
    pub fn to_u64(self) -> u64 {
        self.0
    }

    /// Convert from SpanId to Hexadecimal String
    pub fn to_hex(self) -> String {
        format!("{:016x}", self.0)
    }

    /// Convert from SpanId to Big-Endian byte array
    pub fn to_byte_array(self) -> [u8; 8] {
        self.0.to_be_bytes()
    }

    /// Construct a new SpanId from Hexadecimal String
    pub fn from_hex(hex: &str) -> Self {
        SpanId(u64::from_str_radix(hex, 16).unwrap_or(0))
    }

    /// Construct a new SpanId from Big-Endian byte array
    pub fn from_byte_array(byte_array: [u8; 8]) -> Self {
        SpanId(u64::from_be_bytes(byte_array))
    }
}

/// TraceState carries system-specific configuration data, represented as a list
/// of key-value pairs. TraceState allows multiple tracing systems to
/// participate in the same trace.
///
/// Please review the [W3C specification] for details on this field.
///
/// [W3C specification]: https://www.w3.org/TR/trace-context/#tracestate-header
#[cfg_attr(feature = "serialize", derive(Deserialize, Serialize))]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TraceState(Option<VecDeque<(String, String)>>);

impl TraceState {
    /// Validates that the given `TraceState` list-member key is valid per the [W3 Spec]['spec'].
    ///
    /// ['spec']: https://www.w3.org/TR/trace-context/#key
    fn valid_key(key: &str) -> bool {
        if key.len() > 256 {
            return false;
        }

        let allowed_special = |b: u8| (b == b'_' || b == b'-' || b == b'*' || b == b'/');
        let mut vendor_start = None;
        for (i, &b) in key.as_bytes().iter().enumerate() {
            if !(b.is_ascii_lowercase() || b.is_ascii_digit() || allowed_special(b) || b == b'@') {
                return false;
            }

            if i == 0 && (!b.is_ascii_lowercase() && !b.is_ascii_digit()) {
                return false;
            } else if b == b'@' {
                if vendor_start.is_some() || i < key.len() - 14 {
                    return false;
                }
                vendor_start = Some(i);
            } else if let Some(start) = vendor_start {
                if i == start + 1 && !(b.is_ascii_lowercase() || b.is_ascii_digit()) {
                    return false;
                }
            }
        }

        true
    }

    /// Validates that the given `TraceState` list-member value is valid per the [W3 Spec]['spec'].
    ///
    /// ['spec']: https://www.w3.org/TR/trace-context/#value
    fn valid_value(value: &str) -> bool {
        if value.len() > 256 {
            return false;
        }

        !(value.contains(',') || value.contains('='))
    }

    /// Creates a new `TraceState` from the given key-value collection.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::trace::TraceState;
    ///
    /// let kvs = vec![("foo", "bar"), ("apple", "banana")];
    /// let trace_state: Result<TraceState, ()> = TraceState::from_key_value(kvs);
    ///
    /// assert!(trace_state.is_ok());
    /// assert_eq!(trace_state.unwrap().header(), String::from("foo=bar,apple=banana"))
    /// ```
    #[allow(clippy::all)]
    pub fn from_key_value<T, K, V>(trace_state: T) -> Result<Self, ()>
    where
        T: IntoIterator<Item = (K, V)>,
        K: ToString,
        V: ToString,
    {
        let ordered_data = trace_state
            .into_iter()
            .map(|(key, value)| {
                let (key, value) = (key.to_string(), value.to_string());
                if !TraceState::valid_key(key.as_str()) || !TraceState::valid_value(value.as_str())
                {
                    return Err(());
                }

                Ok((key, value))
            })
            .collect::<Result<VecDeque<_>, ()>>()?;

        if ordered_data.is_empty() {
            Ok(TraceState(None))
        } else {
            Ok(TraceState(Some(ordered_data)))
        }
    }

    /// Retrieves a value for a given key from the `TraceState` if it exists.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.as_ref().and_then(|kvs| {
            kvs.iter().find_map(|item| {
                if item.0.as_str() == key {
                    Some(item.1.as_str())
                } else {
                    None
                }
            })
        })
    }

    /// Inserts the given key-value pair into the `TraceState`. If a value already exists for the
    /// given key, this updates the value and updates the value's position. If the key or value are
    /// invalid per the [W3 Spec]['spec'] an `Err` is returned, else a new `TraceState` with the
    /// updated key/value is returned.
    ///
    /// ['spec']: https://www.w3.org/TR/trace-context/#list
    #[allow(clippy::all)]
    pub fn insert(&self, key: String, value: String) -> Result<TraceState, ()> {
        if !TraceState::valid_key(key.as_str()) || !TraceState::valid_value(value.as_str()) {
            return Err(());
        }

        let mut trace_state = self.delete(key.clone())?;
        let kvs = trace_state.0.get_or_insert(VecDeque::with_capacity(1));

        kvs.push_front((key, value));

        Ok(trace_state)
    }

    /// Removes the given key-value pair from the `TraceState`. If the key is invalid per the
    /// [W3 Spec]['spec'] or the key does not exist an `Err` is returned. Else, a new `TraceState`
    /// with the removed entry is returned.
    ///
    /// ['spec']: https://www.w3.org/TR/trace-context/#list
    #[allow(clippy::all)]
    pub fn delete(&self, key: String) -> Result<TraceState, ()> {
        if !TraceState::valid_key(key.as_str()) {
            return Err(());
        }

        let mut owned = self.clone();
        let kvs = owned.0.as_mut().ok_or(())?;

        if let Some(index) = kvs.iter().position(|x| *x.0 == *key) {
            kvs.remove(index);
        } else {
            return Err(());
        }

        Ok(owned)
    }

    /// Creates a new `TraceState` header string, delimiting each key and value with a `=` and each
    /// entry with a `,`.
    pub fn header(&self) -> String {
        self.header_delimited("=", ",")
    }

    /// Creates a new `TraceState` header string, with the given key/value delimiter and entry delimiter.
    pub fn header_delimited(&self, entry_delimiter: &str, list_delimiter: &str) -> String {
        self.0
            .as_ref()
            .map(|kvs| {
                kvs.iter()
                    .map(|(key, value)| format!("{}{}{}", key, entry_delimiter, value))
                    .collect::<Vec<String>>()
                    .join(list_delimiter)
            })
            .unwrap_or_default()
    }
}

impl FromStr for TraceState {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let list_members: Vec<&str> = s.split_terminator(',').collect();
        let mut key_value_pairs: Vec<(String, String)> = Vec::with_capacity(list_members.len());

        for list_member in list_members {
            match list_member.find('=') {
                None => return Err(()),
                Some(separator_index) => {
                    let (key, value) = list_member.split_at(separator_index);
                    key_value_pairs
                        .push((key.to_string(), value.trim_start_matches('=').to_string()));
                }
            }
        }

        TraceState::from_key_value(key_value_pairs)
    }
}

/// Immutable portion of a `Span` which can be serialized and propagated.
#[cfg_attr(feature = "serialize", derive(Deserialize, Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct SpanContext {
    trace_id: TraceId,
    span_id: SpanId,
    trace_flags: u8,
    is_remote: bool,
    trace_state: TraceState,
}

impl SpanContext {
    /// Create an invalid empty span context
    pub fn empty_context() -> Self {
        SpanContext::new(
            TraceId::invalid(),
            SpanId::invalid(),
            0,
            false,
            TraceState::default(),
        )
    }

    /// Construct a new `SpanContext`
    pub fn new(
        trace_id: TraceId,
        span_id: SpanId,
        trace_flags: u8,
        is_remote: bool,
        trace_state: TraceState,
    ) -> Self {
        SpanContext {
            trace_id,
            span_id,
            trace_flags,
            is_remote,
            trace_state,
        }
    }

    /// A valid trace identifier is a non-zero `u128`.
    pub fn trace_id(&self) -> TraceId {
        self.trace_id
    }

    /// A valid span identifier is a non-zero `u64`.
    pub fn span_id(&self) -> SpanId {
        self.span_id
    }

    /// Returns details about the trace. Unlike `TraceState` values, these are
    /// present in all traces. Currently, the only option is a boolean sampled flag.
    pub fn trace_flags(&self) -> u8 {
        self.trace_flags
    }

    /// Returns a bool flag which is true if the `SpanContext` has a valid (non-zero) `trace_id`
    /// and a valid (non-zero) `span_id`.
    pub fn is_valid(&self) -> bool {
        self.trace_id.0 != 0 && self.span_id.0 != 0
    }

    /// Returns true if the `SpanContext` was propagated from a remote parent.
    pub fn is_remote(&self) -> bool {
        self.is_remote
    }

    /// Returns if the deferred bit is set in the trace flags
    pub fn is_deferred(&self) -> bool {
        (self.trace_flags & TRACE_FLAG_DEFERRED) == TRACE_FLAG_DEFERRED
    }

    /// Returns if the debug bit is set in the trace flags
    pub fn is_debug(&self) -> bool {
        (self.trace_flags & TRACE_FLAG_DEBUG) == TRACE_FLAG_DEBUG
    }

    /// Returns true if the `SpanContext` is sampled.
    pub fn is_sampled(&self) -> bool {
        (self.trace_flags & TRACE_FLAG_SAMPLED) == TRACE_FLAG_SAMPLED
    }

    /// Returns the context's `TraceState`.
    pub fn trace_state(&self) -> &TraceState {
        &self.trace_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rustfmt::skip]
    fn trace_id_test_data() -> Vec<(TraceId, &'static str, [u8; 16])> {
        vec![
            (TraceId(0), "00000000000000000000000000000000", [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            (TraceId(42), "0000000000000000000000000000002a", [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 42]),
            (TraceId(126642714606581564793456114182061442190), "5f467fe7bf42676c05e20ba4a90e448e", [95, 70, 127, 231, 191, 66, 103, 108, 5, 226, 11, 164, 169, 14, 68, 142])
        ]
    }

    #[rustfmt::skip]
    fn span_id_test_data() -> Vec<(SpanId, &'static str, [u8; 8])> {
        vec![
            (SpanId(0), "0000000000000000", [0, 0, 0, 0, 0, 0, 0, 0]),
            (SpanId(42), "000000000000002a", [0, 0, 0, 0, 0, 0, 0, 42]),
            (SpanId(5508496025762705295), "4c721bf33e3caf8f", [76, 114, 27, 243, 62, 60, 175, 143])
        ]
    }

    #[rustfmt::skip]
    fn trace_state_test_data() -> Vec<(TraceState, &'static str, &'static str)> {
        vec![
            (TraceState::from_key_value(vec![("foo", "bar")]).unwrap(), "foo=bar", "foo"),
            (TraceState::from_key_value(vec![("foo", ""), ("apple", "banana")]).unwrap(), "foo=,apple=banana", "apple"),
            (TraceState::from_key_value(vec![("foo", "bar"), ("apple", "banana")]).unwrap(), "foo=bar,apple=banana", "apple"),
        ]
    }

    #[test]
    fn test_trace_id() {
        for test_case in trace_id_test_data() {
            assert_eq!(test_case.0.to_hex(), test_case.1);
            assert_eq!(test_case.0.to_byte_array(), test_case.2);

            assert_eq!(test_case.0, TraceId::from_hex(test_case.1));
            assert_eq!(test_case.0, TraceId::from_byte_array(test_case.2));
        }
    }

    #[test]
    fn test_span_id() {
        for test_case in span_id_test_data() {
            assert_eq!(test_case.0.to_hex(), test_case.1);
            assert_eq!(test_case.0.to_byte_array(), test_case.2);

            assert_eq!(test_case.0, SpanId::from_hex(test_case.1));
            assert_eq!(test_case.0, SpanId::from_byte_array(test_case.2));
        }
    }

    #[test]
    fn test_trace_state() {
        for test_case in trace_state_test_data() {
            assert_eq!(test_case.0.clone().header(), test_case.1);

            let new_key = format!("{}-{}", test_case.0.get(test_case.2).unwrap(), "test");

            let updated_trace_state = test_case.0.insert(test_case.2.into(), new_key.clone());
            assert!(updated_trace_state.is_ok());
            let updated_trace_state = updated_trace_state.unwrap();

            let updated = format!("{}={}", test_case.2, new_key);

            let index = updated_trace_state.clone().header().find(&updated);

            assert!(index.is_some());
            assert_eq!(index.unwrap(), 0);

            let deleted_trace_state = updated_trace_state.delete(test_case.2.to_string());
            assert!(deleted_trace_state.is_ok());

            let deleted_trace_state = deleted_trace_state.unwrap();

            assert!(deleted_trace_state.get(test_case.2).is_none());
        }
    }
}
