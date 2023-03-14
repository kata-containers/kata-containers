use crate::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{
    cell::{Cell, UnsafeCell},
    fmt,
    marker::PhantomData,
};
pub(crate) struct Local<T> {
    // TODO(eliza): this once used a `crossbeam_util::ShardedRwLock`. We may
    // eventually wish to replace it with a sharded lock implementation on top
    // of our internal `RwLock` wrapper type. If possible, we should profile
    // this first to determine if it's necessary.
    inner: RwLock<Inner<T>>,
}

type Inner<T> = Vec<Option<UnsafeCell<T>>>;

/// Uniquely identifies a thread.
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) struct Id {
    id: usize,
    _not_send: PhantomData<UnsafeCell<()>>,
}

// === impl Local ===

impl<T> Local<T> {
    pub(crate) fn new() -> Self {
        let len = Id::current().as_usize();
        // Preallocate up to the current thread ID, so we don't have to inside
        // the lock.
        let mut data = Vec::with_capacity(len);
        data.resize_with(len, || None);
        Local {
            inner: RwLock::new(data),
        }
    }

    pub(crate) fn with_or_else<O>(
        &self,
        new: impl FnOnce() -> T,
        f: impl FnOnce(&mut T) -> O,
    ) -> Option<O> {
        let i = Id::current().as_usize();
        let mut f = Some(f);
        self.try_with_index(i, |item| f.take().expect("called twice")(item))
            .or_else(move || {
                self.new_thread(i, new);
                self.try_with_index(i, |item| f.take().expect("called twice")(item))
            })
    }

    fn try_with_index<O>(&self, i: usize, f: impl FnOnce(&mut T) -> O) -> Option<O> {
        let lock = try_lock!(self.inner.read(), else return None);
        let slot = lock.get(i)?.as_ref()?;
        let item = unsafe { &mut *slot.get() };
        Some(f(item))
    }

    #[cold]
    fn new_thread(&self, i: usize, new: impl FnOnce() -> T) {
        let mut lock = try_lock!(self.inner.write());
        let this = &mut *lock;
        this.resize_with(i + 1, || None);
        this[i] = Some(UnsafeCell::new(new()));
    }
}

impl<T: Default> Local<T> {
    #[inline]
    pub(crate) fn with<O>(&self, f: impl FnOnce(&mut T) -> O) -> Option<O> {
        self.with_or_else(T::default, f)
    }
}

unsafe impl<T> Sync for Local<T> {}

impl<T: fmt::Debug> fmt::Debug for Local<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let id = Id::current();
        self.try_with_index(id.as_usize(), |local| {
            f.debug_struct("Local")
                .field("thread", &id)
                .field("local", &*local)
                .finish()
        })
        .unwrap_or_else(|| {
            f.debug_struct("Local")
                .field("thread", &id)
                .field("local", &format_args!("<uninitialized>"))
                .finish()
        })
    }
}

// === impl Id ===

impl Id {
    pub(crate) fn current() -> Self {
        thread_local! {
            static MY_ID: Cell<Option<Id>> = Cell::new(None);
        }

        MY_ID
            .try_with(|my_id| my_id.get().unwrap_or_else(|| Self::new_thread(my_id)))
            .unwrap_or_else(|_| Self::poisoned())
    }

    pub(crate) fn as_usize(self) -> usize {
        self.id
    }

    #[cold]
    fn new_thread(local: &Cell<Option<Id>>) -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::AcqRel);
        let tid = Self {
            id,
            _not_send: PhantomData,
        };
        local.set(Some(tid));
        tid
    }

    #[cold]
    fn poisoned() -> Self {
        Self {
            id: std::usize::MAX,
            _not_send: PhantomData,
        }
    }

    /// Returns true if the local thread ID was accessed while unwinding.
    pub(crate) fn is_poisoned(self) -> bool {
        self.id == std::usize::MAX
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_poisoned() {
            f.debug_tuple("Id")
                .field(&format_args!("<poisoned>"))
                .finish()
        } else {
            f.debug_tuple("Id").field(&self.id).finish()
        }
    }
}
