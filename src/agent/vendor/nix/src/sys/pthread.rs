use libc::{self, pthread_t};

pub type Pthread = pthread_t;

/// Obtain ID of the calling thread (see
/// [`pthread_self(3)`](http://pubs.opengroup.org/onlinepubs/9699919799/functions/pthread_self.html)
///
/// The thread ID returned by `pthread_self()` is not the same thing as
/// the kernel thread ID returned by a call to `gettid(2)`.
#[inline]
pub fn pthread_self() -> Pthread {
    unsafe { libc::pthread_self() }
}
