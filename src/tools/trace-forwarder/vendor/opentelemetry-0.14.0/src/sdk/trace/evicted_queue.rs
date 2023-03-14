//! # Evicted Queue

#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// This queue maintains an ordered list of elements, and a count of
/// dropped elements. Elements are removed from the queue in a first
/// in first out fashion.
#[cfg_attr(feature = "serialize", derive(Deserialize, Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct EvictedQueue<T> {
    queue: Option<VecDeque<T>>,
    max_len: u32,
    dropped_count: u32,
}

impl<T> EvictedQueue<T> {
    /// Create a new `EvictedQueue` with a given max length.
    pub fn new(max_len: u32) -> Self {
        EvictedQueue {
            queue: None,
            max_len,
            dropped_count: 0,
        }
    }

    /// Push a new element to the back of the queue, dropping and
    /// recording dropped count if over capacity.
    pub(crate) fn push_back(&mut self, value: T) {
        let queue = self.queue.get_or_insert_with(Default::default);
        if queue.len() as u32 == self.max_len {
            queue.pop_front();
            self.dropped_count += 1;
        }
        queue.push_back(value);
    }

    /// Moves all the elements of other into self, leaving other empty.
    pub fn append_vec(&mut self, other: &mut Vec<T>) {
        self.extend(other.drain(..));
    }

    /// Returns `true` if the `EvictedQueue` is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.as_ref().map_or(true, |queue| queue.is_empty())
    }

    /// Returns a front-to-back iterator.
    pub fn iter(&self) -> Iter<'_, T> {
        Iter(self.queue.as_ref().map(|queue| queue.iter()))
    }

    /// Returns the number of elements in the `EvictedQueue`.
    pub fn len(&self) -> usize {
        self.queue.as_ref().map_or(0, |queue| queue.len())
    }

    /// Count of dropped attributes
    pub fn dropped_count(&self) -> u32 {
        self.dropped_count
    }
}

/// An owned iterator over the entries of a `EvictedQueue`.
#[derive(Debug)]
pub struct IntoIter<T>(Option<std::collections::vec_deque::IntoIter<T>>);

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.as_mut().and_then(|iter| iter.next())
    }
}

impl<T> IntoIterator for EvictedQueue<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.queue.map(|queue| queue.into_iter()))
    }
}

/// An iterator over the entries of an `EvictedQueue`.
#[derive(Debug)]
pub struct Iter<'a, T>(Option<std::collections::vec_deque::Iter<'a, T>>);

impl<'a, T: 'static> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.as_mut().and_then(|iter| iter.next())
    }
}

impl<T> Extend<T> for EvictedQueue<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        iter.into_iter().for_each(move |elt| self.push_back(elt));
    }
}

#[cfg(test)]
mod tests {
    use super::EvictedQueue;
    use std::collections::VecDeque;

    #[test]
    fn insert_over_capacity_test() {
        let capacity = 10;
        let mut queue = EvictedQueue::new(capacity);

        for i in 0..=capacity {
            queue.push_back(i)
        }

        assert_eq!(queue.dropped_count, 1);
        assert_eq!(queue.len(), capacity as usize);
        assert_eq!(
            queue.queue.unwrap(),
            (1..=capacity).collect::<VecDeque<_>>()
        );
    }
}
