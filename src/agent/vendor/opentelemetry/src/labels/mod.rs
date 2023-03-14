//! OpenTelemetry Labels
use crate::{Array, Key, KeyValue, Value};
use std::cmp::Ordering;
use std::collections::{btree_map, BTreeMap};
use std::hash::{Hash, Hasher};
use std::iter::Peekable;

mod encoder;
pub use encoder::{default_encoder, new_encoder_id, DefaultLabelEncoder, Encoder, EncoderId};

/// An immutable set of distinct labels.
#[derive(Clone, Debug, Default)]
pub struct LabelSet {
    labels: BTreeMap<Key, Value>,
}

impl LabelSet {
    /// Construct a new label set form a distinct set of labels
    pub fn from_labels<T: IntoIterator<Item = KeyValue>>(labels: T) -> Self {
        LabelSet {
            labels: labels.into_iter().map(|kv| (kv.key, kv.value)).collect(),
        }
    }

    /// The label set length.
    pub fn len(&self) -> usize {
        self.labels.len()
    }

    /// Check if the set of labels is empty.
    pub fn is_empty(&self) -> bool {
        self.labels.is_empty()
    }

    /// Iterate over the label key value pairs.
    pub fn iter(&self) -> Iter<'_> {
        self.into_iter()
    }

    /// Encode the label set with the given encoder and cache the result.
    pub fn encoded(&self, encoder: Option<&dyn Encoder>) -> String {
        encoder.map_or_else(String::new, |encoder| encoder.encode(&mut self.iter()))
    }
}

impl<'a> IntoIterator for &'a LabelSet {
    type Item = (&'a Key, &'a Value);
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter(self.labels.iter())
    }
}
/// An iterator over the entries of a `Set`.
#[derive(Debug)]
pub struct Iter<'a>(btree_map::Iter<'a, Key, Value>);
impl<'a> Iterator for Iter<'a> {
    type Item = (&'a Key, &'a Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// Impl of Hash for `KeyValue`
pub fn hash_labels<'a, H: Hasher, I: IntoIterator<Item = (&'a Key, &'a Value)>>(
    state: &mut H,
    labels: I,
) {
    for (key, value) in labels.into_iter() {
        key.hash(state);
        hash_value(state, value);
    }
}

fn hash_value<H: Hasher>(state: &mut H, value: &Value) {
    match value {
        Value::Bool(b) => b.hash(state),
        Value::I64(i) => i.hash(state),
        Value::F64(f) => {
            // FIXME: f64 does not impl hash, this impl may have incorrect outcomes.
            f.to_bits().hash(state)
        }
        Value::String(s) => s.hash(state),
        Value::Array(arr) => match arr {
            // recursively hash array values
            Array::Bool(values) => values.iter().for_each(|v| v.hash(state)),
            Array::I64(values) => values.iter().for_each(|v| v.hash(state)),
            Array::F64(values) => values.iter().for_each(|v| v.to_bits().hash(state)),
            Array::String(values) => values.iter().for_each(|v| v.hash(state)),
        },
    }
}

/// Merge two iterators, yielding sorted results
pub fn merge_iters<
    'a,
    'b,
    A: Iterator<Item = (&'a Key, &'a Value)>,
    B: Iterator<Item = (&'b Key, &'b Value)>,
>(
    a: A,
    b: B,
) -> MergeIter<'a, 'b, A, B> {
    MergeIter {
        a: a.peekable(),
        b: b.peekable(),
    }
}

/// Merge two iterators, sorting by key
#[derive(Debug)]
pub struct MergeIter<'a, 'b, A, B>
where
    A: Iterator<Item = (&'a Key, &'a Value)>,
    B: Iterator<Item = (&'b Key, &'b Value)>,
{
    a: Peekable<A>,
    b: Peekable<B>,
}

impl<'a, A: Iterator<Item = (&'a Key, &'a Value)>, B: Iterator<Item = (&'a Key, &'a Value)>>
    Iterator for MergeIter<'a, 'a, A, B>
{
    type Item = (&'a Key, &'a Value);
    fn next(&mut self) -> Option<Self::Item> {
        let which = match (self.a.peek(), self.b.peek()) {
            (Some(a), Some(b)) => Some(a.0.cmp(&b.0)),
            (Some(_), None) => Some(Ordering::Less),
            (None, Some(_)) => Some(Ordering::Greater),
            (None, None) => None,
        };

        match which {
            Some(Ordering::Less) => self.a.next(),
            Some(Ordering::Equal) => self.a.next(),
            Some(Ordering::Greater) => self.b.next(),
            None => None,
        }
    }
}
