use std::heap::{Heap, Alloc, Layout};

use core::mem;
use core::ptr;

use Fail;
use backtrace::Backtrace;

pub(crate) struct ErrorImpl {
    inner: &'static mut Inner,
}

// Dynamically sized inner value
struct Inner {
    backtrace: Backtrace,
    vtable: *const VTable,
    failure: FailData,
}

unsafe impl Send for Inner { }
unsafe impl Sync for Inner { }

extern {
    type VTable;
    type FailData;
}

#[allow(dead_code)]
struct InnerRaw<F> {
    header: InnerHeader,
    failure: F,
}

#[allow(dead_code)]
struct InnerHeader {
    backtrace: Backtrace,
    vtable: *const VTable,
}

struct TraitObject {
    #[allow(dead_code)]
    data: *const FailData,
    vtable: *const VTable,
}

impl<F: Fail> From<F> for ErrorImpl {
    fn from(failure: F) -> ErrorImpl {
        let backtrace = if failure.backtrace().is_none() {
            Backtrace::new()
        } else {
            Backtrace::none()
        };

        unsafe {
            let vtable = mem::transmute::<_, TraitObject>(&failure as &Fail).vtable;

            let ptr: *mut InnerRaw<F> = match Heap.alloc(Layout::new::<InnerRaw<F>>()) {
                Ok(p)   => p as *mut InnerRaw<F>,
                Err(e)  => Heap.oom(e),
            };

            // N.B. must use `ptr::write`, not `=`, to avoid dropping the contents of `*ptr`
            ptr::write(ptr, InnerRaw {
                header: InnerHeader {
                    backtrace,
                    vtable,
                },
                failure,
            });

            let inner: &'static mut Inner = mem::transmute(ptr);

            ErrorImpl { inner }
        }
    }
}

impl ErrorImpl {
    pub(crate) fn failure(&self) -> &Fail {
        unsafe {
            mem::transmute::<TraitObject, &Fail>(TraitObject {
                data: &self.inner.failure as *const FailData,
                vtable: self.inner.vtable,
            })
        }
    }

    pub(crate) fn failure_mut(&mut self) -> &mut Fail {
        unsafe {
            mem::transmute::<TraitObject, &mut Fail>(TraitObject {
                data: &mut self.inner.failure as *const FailData,
                vtable: self.inner.vtable,
            })
        }
    }

    pub(crate) fn backtrace(&self) -> &Backtrace {
        &self.inner.backtrace
    }

    pub(crate) fn downcast<T: Fail>(self) -> Result<T, ErrorImpl> {
        let ret: Option<T> = self.failure().downcast_ref().map(|fail| {
            unsafe {
                // drop the backtrace
                let _ = ptr::read(&self.inner.backtrace as *const Backtrace);
                // read out the fail type
                ptr::read(fail as *const T)
            }
        });
        match ret {
            Some(ret) => {
                // forget self (backtrace is dropped, failure is moved
                mem::forget(self);
                Ok(ret)
            }
            _ => Err(self)
        }
    }
}


#[cfg(test)]
mod test {
    use std::mem::size_of;

    use super::ErrorImpl;

    #[test]
    fn assert_is_one_word() {
        assert_eq!(size_of::<ErrorImpl>(), size_of::<usize>());
    }
}
