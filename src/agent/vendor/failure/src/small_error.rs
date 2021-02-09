use core::fmt::{self, Display, Debug};
use std::heap::{Heap, Alloc, Layout};

use core::mem;
use core::ptr;

use {Causes, Fail};
use backtrace::Backtrace;
use context::Context;
use compat::Compat;

/// The `Error` type, which can contain any failure.
///
/// Functions which accumulate many kinds of errors should return this type.
/// All failures can be converted into it, so functions which catch those
/// errors can be tried with `?` inside of a function that returns this kind
/// of error.
///
/// In addition to implementing `Debug` and `Display`, this type carries `Backtrace`
/// information, and can be downcast into the failure that underlies it for
/// more detailed inspection.
pub struct Error {
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

struct InnerRaw<F> {
    header: InnerHeader,
    failure: F,
}

struct InnerHeader {
    backtrace: Backtrace,
    vtable: *const VTable,
}

struct TraitObject {
    #[allow(dead_code)]
    data: *const FailData,
    vtable: *const VTable,
}

impl<F: Fail> From<F> for Error {
    fn from(failure: F) -> Error {
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

            Error { inner }
        }
    }
}

impl Inner {
    fn failure(&self) -> &Fail {
        unsafe {
            mem::transmute::<TraitObject, &Fail>(TraitObject {
                data: &self.failure as *const FailData,
                vtable: self.vtable,
            })
        }
    }

    fn failure_mut(&mut self) -> &mut Fail {
        unsafe {
            mem::transmute::<TraitObject, &mut Fail>(TraitObject {
                data: &mut self.failure as *const FailData,
                vtable: self.vtable,
            })
        }
    }
}

impl Error {
    /// Returns a reference to the underlying cause of this `Error`. Unlike the
    /// method on `Fail`, this does not return an `Option`. The `Error` type
    /// always has an underlying failure.
    pub fn cause(&self) -> &Fail {
        self.inner.failure()
    }

    /// Gets a reference to the `Backtrace` for this `Error`.
    ///
    /// If the failure this wrapped carried a backtrace, that backtrace will
    /// be returned. Otherwise, the backtrace will have been constructed at
    /// the point that failure was cast into the `Error` type.
    pub fn backtrace(&self) -> &Backtrace {
        self.inner.failure().backtrace().unwrap_or(&self.inner.backtrace)
    }

    /// Provides context for this `Error`.
    ///
    /// This can provide additional information about this error, appropriate
    /// to the semantics of the current layer. That is, if you have a
    /// lower-level error, such as an IO error, you can provide additional context
    /// about what that error means in the context of your function. This
    /// gives users of this function more information about what has gone
    /// wrong.
    ///
    /// This takes any type that implements `Display`, as well as
    /// `Send`/`Sync`/`'static`. In practice, this means it can take a `String`
    /// or a string literal, or a failure, or some other custom context-carrying
    /// type.
    pub fn context<D: Display + Send + Sync + 'static>(self, context: D) -> Context<D> {
        Context::with_err(context, self)
    }

    /// Wraps `Error` in a compatibility type.
    ///
    /// This type implements the `Error` trait from `std::error`. If you need
    /// to pass failure's `Error` to an interface that takes any `Error`, you
    /// can use this method to get a compatible type.
    pub fn compat(self) -> Compat<Error> {
        Compat { error: self }
    }

    /// Attempts to downcast this `Error` to a particular `Fail` type.
    ///
    /// This downcasts by value, returning an owned `T` if the underlying
    /// failure is of the type `T`. For this reason it returns a `Result` - in
    /// the case that the underlying error is of a different type, the
    /// original `Error` is returned.
    pub fn downcast<T: Fail>(self) -> Result<T, Error> {
        let ret: Option<T> = self.downcast_ref().map(|fail| {
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

    /// Returns the "root cause" of this error - the last value in the
    /// cause chain which does not return an underlying `cause`.
    pub fn root_cause(&self) -> &Fail {
        ::find_root_cause(self.cause())
    }

    /// Attempts to downcast this `Error` to a particular `Fail` type by
    /// reference.
    ///
    /// If the underlying error is not of type `T`, this will return `None`.
    pub fn downcast_ref<T: Fail>(&self) -> Option<&T> {
        self.inner.failure().downcast_ref()
    }

    /// Attempts to downcast this `Error` to a particular `Fail` type by
    /// mutable reference.
    ///
    /// If the underlying error is not of type `T`, this will return `None`.
    pub fn downcast_mut<T: Fail>(&mut self) -> Option<&mut T> {
        self.inner.failure_mut().downcast_mut()
    }

    /// Returns a iterator over the causes of the `Error`, beginning with
    /// the failure returned by the `cause` method and ending with the failure
    /// returned by `root_cause`.
    pub fn causes(&self) -> Causes {
        Causes { fail: Some(self.cause()) }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(self.inner.failure(), f)
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.inner.backtrace.is_none() {
            Debug::fmt(self.inner.failure(), f)
        } else {
            write!(f, "{:?}\n\n{:?}", self.inner.failure(), self.inner.backtrace)
        }
    }
}

impl Drop for Error {
    fn drop(&mut self) {
        unsafe {
            let layout = {
                let header = Layout::new::<InnerHeader>();
                header.extend(Layout::for_value(self.inner.failure())).unwrap().0
            };
            Heap.dealloc(self.inner as *const _ as *const u8 as *mut u8, layout);
        }
    }
}

#[cfg(test)]
mod test {
    use std::mem::size_of;
    use std::io;

    use super::Error;

    #[test]
    fn assert_error_is_just_data() {
        fn assert_just_data<T: Send + Sync + 'static>() { }
        assert_just_data::<Error>();
    }

    #[test]
    fn assert_is_one_word() {
        assert_eq!(size_of::<Error>(), size_of::<usize>());
    }

    #[test]
    fn methods_seem_to_work() {
        let io_error: io::Error = io::Error::new(io::ErrorKind::NotFound, "test");
        let error: Error = io::Error::new(io::ErrorKind::NotFound, "test").into();
        assert!(error.downcast_ref::<io::Error>().is_some());
        let _: ::Backtrace = *error.backtrace();
        assert_eq!(format!("{:?}", io_error), format!("{:?}", error));
        assert_eq!(format!("{}", io_error), format!("{}", error));
        drop(error);
        assert!(true);
    }
}
