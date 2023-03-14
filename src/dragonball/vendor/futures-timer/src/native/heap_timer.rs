use std::cmp::Ordering;
use std::sync::Arc;
use std::time::Instant;

use super::{Node, ScheduledTimer};

/// Entries in the timer heap, sorted by the instant they're firing at and then
/// also containing some payload data.
pub(crate) struct HeapTimer {
    pub(crate) at: Instant,
    pub(crate) gen: usize,
    pub(crate) node: Arc<Node<ScheduledTimer>>,
}

impl PartialEq for HeapTimer {
    fn eq(&self, other: &HeapTimer) -> bool {
        self.at == other.at
    }
}

impl Eq for HeapTimer {}

impl PartialOrd for HeapTimer {
    fn partial_cmp(&self, other: &HeapTimer) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HeapTimer {
    fn cmp(&self, other: &HeapTimer) -> Ordering {
        self.at.cmp(&other.at)
    }
}
