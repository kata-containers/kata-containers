use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

/// Cached size field used in generated code.
/// It is always equal to itself to simplify generated code.
/// (Generated code can use `#[derive(Eq)]`).
#[derive(Debug, Default)]
pub struct CachedSize {
    size: AtomicUsize,
}

impl CachedSize {
    /// Get cached size
    pub fn get(&self) -> u32 {
        self.size.load(Ordering::Relaxed) as u32
    }

    /// Set cached size
    pub fn set(&self, size: u32) {
        self.size.store(size as usize, Ordering::Relaxed)
    }
}

impl Clone for CachedSize {
    fn clone(&self) -> CachedSize {
        CachedSize {
            size: AtomicUsize::new(self.size.load(Ordering::Relaxed)),
        }
    }
}

impl PartialEq<CachedSize> for CachedSize {
    fn eq(&self, _other: &CachedSize) -> bool {
        true
    }
}

impl Eq for CachedSize {}
