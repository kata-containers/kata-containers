use super::{IntoIter, IterMut, ThreadLocal};
use std::cell::UnsafeCell;
use std::fmt;
use std::panic::UnwindSafe;
use std::sync::atomic::{AtomicUsize, Ordering};
use thread_id;
use unreachable::{UncheckedOptionExt, UncheckedResultExt};

/// Wrapper around `ThreadLocal` which adds a fast path for a single thread.
///
/// This has the same API as `ThreadLocal`, but will register the first thread
/// that sets a value as its owner. All accesses by the owner will go through
/// a special fast path which is much faster than the normal `ThreadLocal` path.
pub struct CachedThreadLocal<T: Send> {
    owner: AtomicUsize,
    local: UnsafeCell<Option<Box<T>>>,
    global: ThreadLocal<T>,
}

// CachedThreadLocal is always Sync, even if T isn't
unsafe impl<T: Send> Sync for CachedThreadLocal<T> {}

impl<T: Send> Default for CachedThreadLocal<T> {
    fn default() -> CachedThreadLocal<T> {
        CachedThreadLocal::new()
    }
}

impl<T: Send> CachedThreadLocal<T> {
    /// Creates a new empty `CachedThreadLocal`.
    pub fn new() -> CachedThreadLocal<T> {
        CachedThreadLocal {
            owner: AtomicUsize::new(0),
            local: UnsafeCell::new(None),
            global: ThreadLocal::new(),
        }
    }

    /// Returns the element for the current thread, if it exists.
    pub fn get(&self) -> Option<&T> {
        let id = thread_id::get();
        let owner = self.owner.load(Ordering::Relaxed);
        if owner == id {
            return unsafe { Some((*self.local.get()).as_ref().unchecked_unwrap()) };
        }
        if owner == 0 {
            return None;
        }
        self.global.get_fast(id)
    }

    /// Returns the element for the current thread, or creates it if it doesn't
    /// exist.
    #[inline(always)]
    pub fn get_or<F>(&self, create: F) -> &T
    where
        F: FnOnce() -> T,
    {
        unsafe {
            self.get_or_try(|| Ok::<T, ()>(create()))
                .unchecked_unwrap_ok()
        }
    }

    /// Returns the element for the current thread, or creates it if it doesn't
    /// exist. If `create` fails, that error is returned and no element is
    /// added.
    pub fn get_or_try<F, E>(&self, create: F) -> Result<&T, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        let id = thread_id::get();
        let owner = self.owner.load(Ordering::Relaxed);
        if owner == id {
            return Ok(unsafe { (*self.local.get()).as_ref().unchecked_unwrap() });
        }
        self.get_or_try_slow(id, owner, create)
    }

    #[cold]
    #[inline(never)]
    fn get_or_try_slow<F, E>(&self, id: usize, owner: usize, create: F) -> Result<&T, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        if owner == 0 && self.owner.compare_and_swap(0, id, Ordering::Relaxed) == 0 {
            unsafe {
                (*self.local.get()) = Some(Box::new(create()?));
                return Ok((*self.local.get()).as_ref().unchecked_unwrap());
            }
        }
        match self.global.get_fast(id) {
            Some(x) => Ok(x),
            None => Ok(self.global.insert(id, Box::new(create()?), true)),
        }
    }

    /// Returns a mutable iterator over the local values of all threads.
    ///
    /// Since this call borrows the `ThreadLocal` mutably, this operation can
    /// be done safely---the mutable borrow statically guarantees no other
    /// threads are currently accessing their associated values.
    pub fn iter_mut(&mut self) -> CachedIterMut<T> {
        CachedIterMut {
            local: unsafe { (*self.local.get()).as_mut().map(|x| &mut **x) },
            global: self.global.iter_mut(),
        }
    }

    /// Removes all thread-specific values from the `ThreadLocal`, effectively
    /// reseting it to its original state.
    ///
    /// Since this call borrows the `ThreadLocal` mutably, this operation can
    /// be done safely---the mutable borrow statically guarantees no other
    /// threads are currently accessing their associated values.
    pub fn clear(&mut self) {
        *self = CachedThreadLocal::new();
    }
}

impl<T: Send> IntoIterator for CachedThreadLocal<T> {
    type Item = T;
    type IntoIter = CachedIntoIter<T>;

    fn into_iter(self) -> CachedIntoIter<T> {
        CachedIntoIter {
            local: unsafe { (*self.local.get()).take().map(|x| *x) },
            global: self.global.into_iter(),
        }
    }
}

impl<'a, T: Send + 'a> IntoIterator for &'a mut CachedThreadLocal<T> {
    type Item = &'a mut T;
    type IntoIter = CachedIterMut<'a, T>;

    fn into_iter(self) -> CachedIterMut<'a, T> {
        self.iter_mut()
    }
}

impl<T: Send + Default> CachedThreadLocal<T> {
    /// Returns the element for the current thread, or creates a default one if
    /// it doesn't exist.
    pub fn get_or_default(&self) -> &T {
        self.get_or(T::default)
    }
}

impl<T: Send + fmt::Debug> fmt::Debug for CachedThreadLocal<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ThreadLocal {{ local_data: {:?} }}", self.get())
    }
}

impl<T: Send + UnwindSafe> UnwindSafe for CachedThreadLocal<T> {}

/// Mutable iterator over the contents of a `CachedThreadLocal`.
pub struct CachedIterMut<'a, T: Send + 'a> {
    local: Option<&'a mut T>,
    global: IterMut<'a, T>,
}

impl<'a, T: Send + 'a> Iterator for CachedIterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<&'a mut T> {
        self.local.take().or_else(|| self.global.next())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.global.size_hint().0 + self.local.is_some() as usize;
        (len, Some(len))
    }
}

impl<'a, T: Send + 'a> ExactSizeIterator for CachedIterMut<'a, T> {}

/// An iterator that moves out of a `CachedThreadLocal`.
pub struct CachedIntoIter<T: Send> {
    local: Option<T>,
    global: IntoIter<T>,
}

impl<T: Send> Iterator for CachedIntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        self.local.take().or_else(|| self.global.next())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.global.size_hint().0 + self.local.is_some() as usize;
        (len, Some(len))
    }
}

impl<T: Send> ExactSizeIterator for CachedIntoIter<T> {}
