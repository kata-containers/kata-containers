//! Thread synchronization primitives.
//!
//! * [`Parker`], a thread parking primitive.
//! * [`ShardedLock`], a sharded reader-writer lock with fast concurrent reads.
//! * [`WaitGroup`], for synchronizing the beginning or end of some computation.
//!
//! [`Parker`]: struct.Parker.html
//! [`ShardedLock`]: struct.ShardedLock.html
//! [`WaitGroup`]: struct.WaitGroup.html

mod parker;
mod sharded_lock;
mod wait_group;

pub use self::parker::{Parker, Unparker};
pub use self::sharded_lock::{ShardedLock, ShardedLockReadGuard, ShardedLockWriteGuard};
pub use self::wait_group::WaitGroup;
