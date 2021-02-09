use libc;
#[cfg(not(target_env = "musl"))]
use Result;
#[cfg(not(target_env = "musl"))]
use errno::Errno;
use std::mem;
use sys::signal::SigSet;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct UContext {
    context: libc::ucontext_t,
}

impl UContext {
    #[cfg(not(target_env = "musl"))]
    pub fn get() -> Result<UContext> {
        let mut context = mem::MaybeUninit::<libc::ucontext_t>::uninit();
        let res = unsafe { libc::getcontext(context.as_mut_ptr()) };
        Errno::result(res).map(|_| unsafe {
            UContext { context: context.assume_init()}
        })
    }

    #[cfg(not(target_env = "musl"))]
    pub fn set(&self) -> Result<()> {
        let res = unsafe {
            libc::setcontext(&self.context as *const libc::ucontext_t)
        };
        Errno::result(res).map(drop)
    }

    pub fn sigmask_mut(&mut self) -> &mut SigSet {
        unsafe { mem::transmute(&mut self.context.uc_sigmask) }
    }

    pub fn sigmask(&self) -> &SigSet {
        unsafe { mem::transmute(&self.context.uc_sigmask) }
    }
}
