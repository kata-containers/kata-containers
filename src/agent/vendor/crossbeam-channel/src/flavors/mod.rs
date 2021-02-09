//! Channel flavors.
//!
//! There are six flavors:
//!
//! 1. `after` - Channel that delivers a message after a certain amount of time.
//! 2. `array` - Bounded channel based on a preallocated array.
//! 3. `list` - Unbounded channel implemented as a linked list.
//! 4. `never` - Channel that never delivers messages.
//! 5. `tick` - Channel that delivers messages periodically.
//! 6. `zero` - Zero-capacity channel.

pub mod after;
pub mod array;
pub mod list;
pub mod never;
pub mod tick;
pub mod zero;
