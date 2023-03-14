//! # Resource
//!
//! A `Resource` is an immutable representation of the entity producing telemetry. For example, a
//! process producing telemetry that is running in a container on Kubernetes has a Pod name, it is
//! in a namespace, and possibly is part of a Deployment which also has a name. All three of these
//! attributes can be included in the `Resource`.
//!
//! The primary purpose of resources as a first-class concept in the SDK is decoupling of discovery
//! of resource information from exporters. This allows for independent development and easy
//! customization for users that need to integrate with closed source environments. When used with
//! distributed tracing, a resource can be associated with the [`TracerProvider`] when it is created.
//! That association cannot be changed later. When associated with a `TracerProvider`, all `Span`s
//! produced by any `Tracer` from the provider are associated with this `Resource`.
//!
//! [`TracerProvider`]: crate::trace::TracerProvider
#[cfg(feature = "metrics")]
use crate::labels;
use crate::sdk::EnvResourceDetector;
use crate::{Key, KeyValue, Value};
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};
use std::collections::{btree_map, BTreeMap};
use std::time::Duration;

/// Describes an entity about which identifying information and metadata is exposed.
///
/// Items are sorted by their key, and are only overwritten if the value is an empty string.
#[cfg_attr(feature = "serialize", derive(Deserialize, Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Resource {
    attrs: BTreeMap<Key, Value>,
}

impl Default for Resource {
    fn default() -> Self {
        Self::from_detectors(
            Duration::from_secs(0),
            vec![Box::new(EnvResourceDetector::new())],
        )
    }
}

impl Resource {
    /// Creates an empty resource.
    pub fn empty() -> Self {
        Self {
            attrs: Default::default(),
        }
    }

    /// Create a new `Resource` from key value pairs.
    ///
    /// Values are de-duplicated by key, and the first key-value pair with a non-empty string value
    /// will be retained
    pub fn new<T: IntoIterator<Item = KeyValue>>(kvs: T) -> Self {
        let mut resource = Resource::empty();

        for kv in kvs.into_iter() {
            resource.attrs.insert(kv.key, kv.value);
        }

        resource
    }

    /// Create a new `Resource` from resource detectors.
    ///
    /// timeout will be applied to each detector.
    pub fn from_detectors(timeout: Duration, detectors: Vec<Box<dyn ResourceDetector>>) -> Self {
        let mut resource = Resource::empty();
        for detector in detectors {
            let detected_res = detector.detect(timeout);
            for (key, value) in detected_res.into_iter() {
                // using insert instead of merge to avoid clone.
                resource.attrs.insert(key, value);
            }
        }

        resource
    }

    /// Create a new `Resource` by combining two resources.
    ///
    /// Keys from the `other` resource have priority over keys from this resource, even if the
    /// updated value is empty.
    pub fn merge(&self, other: &Self) -> Self {
        if self.attrs.is_empty() {
            return other.clone();
        }
        if other.attrs.is_empty() {
            return self.clone();
        }

        let mut resource = Resource::empty();

        // attrs from self must be added first so they have priority
        for (k, v) in self.attrs.iter() {
            resource.attrs.insert(k.clone(), v.clone());
        }
        for (k, v) in other.attrs.iter() {
            resource.attrs.insert(k.clone(), v.clone());
        }

        resource
    }

    /// Returns the number of attributes for this resource
    pub fn len(&self) -> usize {
        self.attrs.len()
    }

    /// Returns `true` if the resource contains no attributes.
    pub fn is_empty(&self) -> bool {
        self.attrs.is_empty()
    }

    /// Gets an iterator over the attributes of this resource, sorted by key.
    pub fn iter(&self) -> Iter<'_> {
        self.into_iter()
    }

    /// Encoded labels
    #[cfg(feature = "metrics")]
    #[cfg_attr(docsrs, doc(cfg(feature = "metrics")))]
    pub fn encoded(&self, encoder: &dyn labels::Encoder) -> String {
        encoder.encode(&mut self.into_iter())
    }
}

/// An owned iterator over the entries of a `Resource`.
#[derive(Debug)]
pub struct IntoIter(btree_map::IntoIter<Key, Value>);

impl Iterator for IntoIter {
    type Item = (Key, Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl IntoIterator for Resource {
    type Item = (Key, Value);
    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.attrs.into_iter())
    }
}

/// An iterator over the entries of a `Resource`.
#[derive(Debug)]
pub struct Iter<'a>(btree_map::Iter<'a, Key, Value>);

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a Key, &'a Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a> IntoIterator for &'a Resource {
    type Item = (&'a Key, &'a Value);
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter(self.attrs.iter())
    }
}

/// ResourceDetector detects OpenTelemetry resource information
///
/// Implementations of this trait can be passed to
/// the `Resource::from_detectors` function to generate a Resource from the merged information.
pub trait ResourceDetector {
    /// detect returns an initialized Resource based on gathered information.
    ///
    /// timeout is used in case the detection operation takes too much time.
    ///
    /// If source information to construct a Resource is inaccessible, an empty Resource should be returned
    ///
    /// If source information to construct a Resource is invalid, for example,
    /// missing required values. an empty Resource should be returned.
    fn detect(&self, timeout: Duration) -> Resource;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk::EnvResourceDetector;
    use std::collections::BTreeMap;
    use std::{env, time};

    #[test]
    fn new_resource() {
        let args_with_dupe_keys = vec![KeyValue::new("a", ""), KeyValue::new("a", "final")];

        let mut expected_attrs = BTreeMap::new();
        expected_attrs.insert(Key::new("a"), Value::from("final"));

        assert_eq!(
            Resource::new(args_with_dupe_keys),
            Resource {
                attrs: expected_attrs
            }
        );
    }

    #[test]
    fn merge_resource() {
        let resource_a = Resource::new(vec![
            KeyValue::new("a", ""),
            KeyValue::new("b", "b-value"),
            KeyValue::new("d", "d-value"),
        ]);

        let resource_b = Resource::new(vec![
            KeyValue::new("a", "a-value"),
            KeyValue::new("c", "c-value"),
            KeyValue::new("d", ""),
        ]);

        let mut expected_attrs = BTreeMap::new();
        expected_attrs.insert(Key::new("a"), Value::from("a-value"));
        expected_attrs.insert(Key::new("b"), Value::from("b-value"));
        expected_attrs.insert(Key::new("c"), Value::from("c-value"));
        expected_attrs.insert(Key::new("d"), Value::from(""));

        assert_eq!(
            resource_a.merge(&resource_b),
            Resource {
                attrs: expected_attrs
            }
        );
    }

    #[test]
    fn detect_resource() {
        env::set_var("OTEL_RESOURCE_ATTRIBUTES", "key=value, k = v , a= x, a=z");
        env::set_var("irrelevant".to_uppercase(), "20200810");

        let detector = EnvResourceDetector::new();
        let resource =
            Resource::from_detectors(time::Duration::from_secs(5), vec![Box::new(detector)]);
        assert_eq!(
            resource,
            Resource::new(vec![
                KeyValue::new("key", "value"),
                KeyValue::new("k", "v"),
                KeyValue::new("a", "x"),
                KeyValue::new("a", "z")
            ])
        )
    }
}
