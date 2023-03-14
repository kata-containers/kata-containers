use core::cell::UnsafeCell;
use core::default::Default;
use core::fmt;
use core::marker::PhantomData;
use core::mem;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::sync::atomic::{spin_loop_hint as cpu_relax, AtomicUsize, Ordering};

pub struct RwLock<T: ?Sized> {
    lock: AtomicUsize,
    data: UnsafeCell<T>,
}

const READER: usize = 1 << 2;

const UPGRADED: usize = 1 << 1;

const WRITER: usize = 1;

#[derive(Debug)]
pub struct RwLockReadGuard<'a, T: 'a + ?Sized> {
    lock: &'a AtomicUsize,
    data: NonNull<T>,
}

unsafe impl<'a, T: Send> Send for RwLockReadGuard<'a, T> {}

unsafe impl<'a, T: Sync> Sync for RwLockReadGuard<'a, T> {}

#[derive(Debug)]
pub struct RwLockWriteGuard<'a, T: 'a + ?Sized> {
    lock: &'a AtomicUsize,
    data: NonNull<T>,
    #[doc(hidden)]
    _invariant: PhantomData<&'a mut T>,
}

unsafe impl<'a, T: Send> Send for RwLockWriteGuard<'a, T> {}

unsafe impl<'a, T: Sync> Sync for RwLockWriteGuard<'a, T> {}

#[derive(Debug)]
pub struct RwLockUpgradeableGuard<'a, T: 'a + ?Sized> {
    lock: &'a AtomicUsize,
    data: NonNull<T>,
    #[doc(hidden)]
    _invariant: PhantomData<&'a mut T>,
}

unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}

unsafe impl<T: ?Sized + Send + Sync> Sync for RwLock<T> {}

impl<T> RwLock<T> {
    pub const fn new(user_data: T) -> RwLock<T> {
        RwLock {
            lock: AtomicUsize::new(0),
            data: UnsafeCell::new(user_data),
        }
    }

    pub fn into_inner(self) -> T {
        let RwLock { data, .. } = self;

        data.into_inner()
    }
}

impl<T: ?Sized> RwLock<T> {
    pub fn read(&self) -> RwLockReadGuard<T> {
        loop {
            match self.try_read() {
                Some(guard) => return guard,
                None => cpu_relax(),
            }
        }
    }

    pub fn try_read(&self) -> Option<RwLockReadGuard<T>> {
        let value = self.lock.fetch_add(READER, Ordering::Acquire);

        // We check the UPGRADED bit here so that new readers are prevented when an UPGRADED lock is held.
        // This helps reduce writer starvation.
        if value & (WRITER | UPGRADED) != 0 {
            // Lock is taken, undo.
            self.lock.fetch_sub(READER, Ordering::Release);

            None
        } else {
            Some(RwLockReadGuard {
                lock: &self.lock,
                data: unsafe { NonNull::new_unchecked(self.data.get()) },
            })
        }
    }

    /// # Safety
    ///
    /// This is only safe if the lock is currently locked in read mode and the number of readers is not 0.
    pub unsafe fn force_read_decrement(&self) {
        debug_assert!(self.lock.load(Ordering::Relaxed) & !WRITER > 0);

        self.lock.fetch_sub(READER, Ordering::Release);
    }

    /// # Safety
    ///
    /// The lock must be locked in write mode.
    pub unsafe fn force_write_unlock(&self) {
        debug_assert_eq!(self.lock.load(Ordering::Relaxed) & !(WRITER | UPGRADED), 0);

        self.lock.fetch_and(!(WRITER | UPGRADED), Ordering::Release);
    }

    fn try_write_internal(&self, strong: bool) -> Option<RwLockWriteGuard<T>> {
        if compare_exchange(
            &self.lock,
            0,
            WRITER,
            Ordering::Acquire,
            Ordering::Relaxed,
            strong,
        )
        .is_ok()
        {
            Some(RwLockWriteGuard {
                lock: &self.lock,
                data: unsafe { NonNull::new_unchecked(self.data.get()) },
                _invariant: PhantomData,
            })
        } else {
            None
        }
    }

    pub fn write(&self) -> RwLockWriteGuard<T> {
        loop {
            match self.try_write_internal(false) {
                Some(guard) => return guard,
                None => cpu_relax(),
            }
        }
    }

    pub fn try_write(&self) -> Option<RwLockWriteGuard<T>> {
        self.try_write_internal(true)
    }

    pub fn upgradeable_read(&self) -> RwLockUpgradeableGuard<T> {
        loop {
            match self.try_upgradeable_read() {
                Some(guard) => return guard,
                None => cpu_relax(),
            }
        }
    }

    pub fn try_upgradeable_read(&self) -> Option<RwLockUpgradeableGuard<T>> {
        if self.lock.fetch_or(UPGRADED, Ordering::Acquire) & (WRITER | UPGRADED) == 0 {
            Some(RwLockUpgradeableGuard {
                lock: &self.lock,
                data: unsafe { NonNull::new_unchecked(self.data.get()) },
                _invariant: PhantomData,
            })
        } else {
            None
        }
    }

    /// # Safety
    /// Write locks may not be used in combination with this method.
    pub unsafe fn get(&self) -> &T {
        &*self.data.get()
    }

    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data.get() }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.try_read() {
            Some(guard) => write!(f, "RwLock {{ data: ")
                .and_then(|()| (&*guard).fmt(f))
                .and_then(|()| write!(f, "}}")),
            None => write!(f, "RwLock {{ <locked> }}"),
        }
    }
}

impl<T: ?Sized + Default> Default for RwLock<T> {
    fn default() -> RwLock<T> {
        RwLock::new(Default::default())
    }
}

impl<'rwlock, T: ?Sized> RwLockUpgradeableGuard<'rwlock, T> {
    fn try_upgrade_internal(self, strong: bool) -> Result<RwLockWriteGuard<'rwlock, T>, Self> {
        if compare_exchange(
            &self.lock,
            UPGRADED,
            WRITER,
            Ordering::Acquire,
            Ordering::Relaxed,
            strong,
        )
        .is_ok()
        {
            let out = Ok(RwLockWriteGuard {
                lock: &self.lock,
                data: self.data,
                _invariant: PhantomData,
            });

            mem::forget(self);

            out
        } else {
            Err(self)
        }
    }

    pub fn upgrade(mut self) -> RwLockWriteGuard<'rwlock, T> {
        loop {
            self = match self.try_upgrade_internal(false) {
                Ok(guard) => return guard,
                Err(e) => e,
            };

            cpu_relax();
        }
    }

    pub fn try_upgrade(self) -> Result<RwLockWriteGuard<'rwlock, T>, Self> {
        self.try_upgrade_internal(true)
    }

    pub fn downgrade(self) -> RwLockReadGuard<'rwlock, T> {
        self.lock.fetch_add(READER, Ordering::Acquire);

        RwLockReadGuard {
            lock: &self.lock,
            data: self.data,
        }
    }
}

impl<'rwlock, T: ?Sized> RwLockWriteGuard<'rwlock, T> {
    pub fn downgrade(self) -> RwLockReadGuard<'rwlock, T> {
        self.lock.fetch_add(READER, Ordering::Acquire);

        RwLockReadGuard {
            lock: &self.lock,
            data: self.data,
        }
    }
}

impl<'rwlock, T: ?Sized> Deref for RwLockReadGuard<'rwlock, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { self.data.as_ref() }
    }
}

impl<'rwlock, T: ?Sized> Deref for RwLockUpgradeableGuard<'rwlock, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { self.data.as_ref() }
    }
}

impl<'rwlock, T: ?Sized> Deref for RwLockWriteGuard<'rwlock, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { self.data.as_ref() }
    }
}

impl<'rwlock, T: ?Sized> DerefMut for RwLockWriteGuard<'rwlock, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.data.as_mut() }
    }
}

impl<'rwlock, T: ?Sized> Drop for RwLockReadGuard<'rwlock, T> {
    fn drop(&mut self) {
        debug_assert!(self.lock.load(Ordering::Relaxed) & !(WRITER | UPGRADED) > 0);

        self.lock.fetch_sub(READER, Ordering::Release);
    }
}

impl<'rwlock, T: ?Sized> Drop for RwLockUpgradeableGuard<'rwlock, T> {
    fn drop(&mut self) {
        debug_assert_eq!(
            self.lock.load(Ordering::Relaxed) & (WRITER | UPGRADED),
            UPGRADED
        );

        self.lock.fetch_sub(UPGRADED, Ordering::AcqRel);
    }
}

impl<'rwlock, T: ?Sized> Drop for RwLockWriteGuard<'rwlock, T> {
    fn drop(&mut self) {
        debug_assert_eq!(self.lock.load(Ordering::Relaxed) & WRITER, WRITER);

        self.lock.fetch_and(!(WRITER | UPGRADED), Ordering::Release);
    }
}

fn compare_exchange(
    atomic: &AtomicUsize,
    current: usize,
    new: usize,
    success: Ordering,
    failure: Ordering,
    strong: bool,
) -> Result<usize, usize> {
    if strong {
        atomic.compare_exchange(current, new, success, failure)
    } else {
        atomic.compare_exchange_weak(current, new, success, failure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::channel;
    use std::sync::Arc;
    use std::thread;

    #[derive(Eq, PartialEq, Debug)]
    struct NonCopy(i32);

    #[test]
    fn smoke() {
        let l = RwLock::new(());
        drop(l.read());
        drop(l.write());
        drop((l.read(), l.read()));
        drop(l.write());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_rw_arc() {
        let arc = Arc::new(RwLock::new(0));
        let arc2 = arc.clone();
        let (tx, rx) = channel();

        thread::spawn(move || {
            let mut lock = arc2.write();

            for _ in 0..10 {
                let tmp = *lock;
                *lock = -1;
                thread::yield_now();
                *lock = tmp + 1;
            }

            tx.send(()).unwrap();
        });

        let mut children = Vec::new();

        for _ in 0..5 {
            let arc3 = arc.clone();

            children.push(thread::spawn(move || {
                let lock = arc3.read();
                assert!(*lock >= 0);
            }));
        }

        for r in children {
            assert!(r.join().is_ok());
        }

        rx.recv().unwrap();
        let lock = arc.read();
        assert_eq!(*lock, 10);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_rw_access_in_unwind() {
        let arc = Arc::new(RwLock::new(1));
        let arc2 = arc.clone();

        let _ = thread::spawn(move || {
            struct Unwinder {
                i: Arc<RwLock<isize>>,
            }

            impl Drop for Unwinder {
                fn drop(&mut self) {
                    let mut lock = self.i.write();

                    *lock += 1;
                }
            }

            let _u = Unwinder { i: arc2 };

            panic!();
        })
        .join();

        let lock = arc.read();
        assert_eq!(*lock, 2);
    }

    #[test]
    fn test_rwlock_unsized() {
        let rw: &RwLock<[i32]> = &RwLock::new([1, 2, 3]);

        {
            let b = &mut *rw.write();
            b[0] = 4;
            b[2] = 5;
        }

        let comp: &[i32] = &[4, 2, 5];

        assert_eq!(&*rw.read(), comp);
    }

    #[test]
    fn test_rwlock_try_write() {
        use std::mem::drop;

        let lock = RwLock::new(0isize);
        let read_guard = lock.read();
        let write_result = lock.try_write();

        match write_result {
            None => (),
            Some(_) => panic!("try_write should not succeed while read_guard is in scope"),
        }

        drop(read_guard);
    }

    #[test]
    fn test_rw_try_read() {
        let m = RwLock::new(0);

        mem::forget(m.write());

        assert!(m.try_read().is_none());
    }

    #[test]
    fn test_into_inner() {
        let m = RwLock::new(NonCopy(10));

        assert_eq!(m.into_inner(), NonCopy(10));
    }

    #[test]
    fn test_into_inner_drop() {
        struct Foo(Arc<AtomicUsize>);

        impl Drop for Foo {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let num_drops = Arc::new(AtomicUsize::new(0));
        let m = RwLock::new(Foo(num_drops.clone()));
        assert_eq!(num_drops.load(Ordering::SeqCst), 0);

        {
            let _inner = m.into_inner();
            assert_eq!(num_drops.load(Ordering::SeqCst), 0);
        }

        assert_eq!(num_drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_force_read_decrement() {
        let m = RwLock::new(());
        ::std::mem::forget(m.read());
        ::std::mem::forget(m.read());
        ::std::mem::forget(m.read());
        assert!(m.try_write().is_none());

        unsafe {
            m.force_read_decrement();
            m.force_read_decrement();
        }

        assert!(m.try_write().is_none());

        unsafe {
            m.force_read_decrement();
        }

        assert!(m.try_write().is_some());
    }

    #[test]
    fn test_force_write_unlock() {
        let m = RwLock::new(());

        ::std::mem::forget(m.write());

        assert!(m.try_read().is_none());

        unsafe {
            m.force_write_unlock();
        }

        assert!(m.try_read().is_some());
    }

    #[test]
    fn test_upgrade_downgrade() {
        let m = RwLock::new(());

        {
            let _r = m.read();
            let upg = m.try_upgradeable_read().unwrap();
            assert!(m.try_read().is_none());
            assert!(m.try_write().is_none());
            assert!(upg.try_upgrade().is_err());
        }

        {
            let w = m.write();
            assert!(m.try_upgradeable_read().is_none());
            let _r = w.downgrade();
            assert!(m.try_upgradeable_read().is_some());
            assert!(m.try_read().is_some());
            assert!(m.try_write().is_none());
        }

        {
            let _u = m.upgradeable_read();
            assert!(m.try_upgradeable_read().is_none());
        }

        assert!(m.try_upgradeable_read().unwrap().try_upgrade().is_ok());
    }
}
