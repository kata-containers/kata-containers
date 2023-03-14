extern crate libc;

use std::num::NonZeroUsize;

extern "C" {
    fn kinfo_getproc(pid: libc::pid_t) -> *mut libc::kinfo_proc;
}

pub(crate) fn num_threads() -> Option<NonZeroUsize> {
    // Safety: `kinfo_getproc` and `getpid` are both thread-safe. All invariants of `as_ref` are
    // upheld.
    unsafe {
        let kip = kinfo_getproc(libc::getpid());
        let num_threads = NonZeroUsize::new(kip.as_ref()?.ki_numthreads as usize);
        libc::free(kip as *mut libc::c_void);
        num_threads
    }
}
