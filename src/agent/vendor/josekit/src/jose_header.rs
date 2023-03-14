use crate::Value;

use std::fmt::Debug;

pub trait JoseHeader: Send + Sync + Debug {
    // Return claim count.
    fn len(&self) -> usize;

    /// Return the value for header claim of a specified key.
    ///
    /// # Arguments
    ///
    /// * `key` - a key name of header claim
    fn claim(&self, key: &str) -> Option<&Value>;

    fn box_clone(&self) -> Box<dyn JoseHeader>;
}

impl Clone for Box<dyn JoseHeader> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}
