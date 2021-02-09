use core::any::TypeId;

use Fail;
use backtrace::Backtrace;

pub(crate) struct ErrorImpl {
    inner: Box<Inner<dyn Fail>>,
}

struct Inner<F: ?Sized + Fail> {
    backtrace: Backtrace,
    pub(crate) failure: F,
}

impl<F: Fail> From<F> for ErrorImpl {
    fn from(failure: F) -> ErrorImpl {
        let inner: Inner<F> = {
            let backtrace = if failure.backtrace().is_none() {
                Backtrace::new()
            } else { Backtrace::none() };
            Inner { failure, backtrace }
        };
        ErrorImpl { inner: Box::new(inner) }
    }
}

impl ErrorImpl {
    pub(crate) fn failure(&self) -> &dyn Fail {
        &self.inner.failure
    }

    pub(crate) fn failure_mut(&mut self) -> &mut dyn Fail {
        &mut self.inner.failure
    }

    pub(crate) fn backtrace(&self) -> &Backtrace {
        &self.inner.backtrace
    }

    pub(crate) fn downcast<T: Fail>(self) -> Result<T, ErrorImpl> {
        if self.failure().__private_get_type_id__() == TypeId::of::<T>() {
            let ErrorImpl { inner } = self;
            let casted = unsafe { Box::from_raw(Box::into_raw(inner) as *mut Inner<T>) };
            let Inner { backtrace:_, failure } = *casted;
            Ok(failure)
        } else {
            Err(self)
        }
    }
}
