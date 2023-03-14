// Copyright 2019 Intel Corporation. All Rights Reserved.
//
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Copyright 2017 The Chromium OS Authors. All rights reserved.
//
// SPDX-License-Identifier: (Apache-2.0 AND BSD-3-Clause)

//! Enums, traits and functions for working with
//! [`signal`](http://man7.org/linux/man-pages/man7/signal.7.html).

use libc::{
    c_int, c_void, pthread_kill, pthread_sigmask, pthread_t, sigaction, sigaddset, sigemptyset,
    sigfillset, siginfo_t, sigismember, sigpending, sigset_t, sigtimedwait, timespec, EAGAIN,
    EINTR, EINVAL, SIG_BLOCK, SIG_UNBLOCK,
};

use crate::errno;
use std::fmt::{self, Display};
use std::io;
use std::mem;
use std::os::unix::thread::JoinHandleExt;
use std::ptr::{null, null_mut};
use std::result;
use std::thread::JoinHandle;

/// The error cases enumeration for signal handling.
#[derive(Debug)]
pub enum Error {
    /// Couldn't create a sigset.
    CreateSigset(errno::Error),
    /// The wrapped signal has already been blocked.
    SignalAlreadyBlocked(c_int),
    /// Failed to check if the requested signal is in the blocked set already.
    CompareBlockedSignals(errno::Error),
    /// The signal could not be blocked.
    BlockSignal(errno::Error),
    /// The signal mask could not be retrieved.
    RetrieveSignalMask(c_int),
    /// The signal could not be unblocked.
    UnblockSignal(errno::Error),
    /// Failed to wait for given signal.
    ClearWaitPending(errno::Error),
    /// Failed to get pending signals.
    ClearGetPending(errno::Error),
    /// Failed to check if given signal is in the set of pending signals.
    ClearCheckPending(errno::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use self::Error::*;

        match self {
            CreateSigset(e) => write!(f, "couldn't create a sigset: {}", e),
            SignalAlreadyBlocked(num) => write!(f, "signal {} already blocked", num),
            CompareBlockedSignals(e) => write!(
                f,
                "failed to check whether requested signal is in the blocked set: {}",
                e,
            ),
            BlockSignal(e) => write!(f, "signal could not be blocked: {}", e),
            RetrieveSignalMask(errno) => write!(
                f,
                "failed to retrieve signal mask: {}",
                io::Error::from_raw_os_error(*errno),
            ),
            UnblockSignal(e) => write!(f, "signal could not be unblocked: {}", e),
            ClearWaitPending(e) => write!(f, "failed to wait for given signal: {}", e),
            ClearGetPending(e) => write!(f, "failed to get pending signals: {}", e),
            ClearCheckPending(e) => write!(
                f,
                "failed to check whether given signal is in the pending set: {}",
                e,
            ),
        }
    }
}

/// A simplified [Result](https://doc.rust-lang.org/std/result/enum.Result.html) type
/// for operations that can return [`Error`](Enum.error.html).
pub type SignalResult<T> = result::Result<T, Error>;

/// Public alias for a signal handler.
/// [`sigaction`](http://man7.org/linux/man-pages/man2/sigaction.2.html).
pub type SignalHandler =
    extern "C" fn(num: c_int, info: *mut siginfo_t, _unused: *mut c_void) -> ();

extern "C" {
    fn __libc_current_sigrtmin() -> c_int;
    fn __libc_current_sigrtmax() -> c_int;
}

/// Return the minimum (inclusive) real-time signal number.
#[allow(non_snake_case)]
pub fn SIGRTMIN() -> c_int {
    unsafe { __libc_current_sigrtmin() }
}

/// Return the maximum (inclusive) real-time signal number.
#[allow(non_snake_case)]
pub fn SIGRTMAX() -> c_int {
    unsafe { __libc_current_sigrtmax() }
}

/// Verify that a signal number is valid.
///
/// Supported signals range from `SIGHUP` to `SIGSYS` and from `SIGRTMIN` to `SIGRTMAX`.
/// We recommend using realtime signals `[SIGRTMIN(), SIGRTMAX()]` for VCPU threads.
///
/// # Arguments
///
/// * `num`: the signal number to be verified.
///
/// # Examples
///
/// ```
/// extern crate vmm_sys_util;
/// use vmm_sys_util::signal::validate_signal_num;
///
/// let num = validate_signal_num(1).unwrap();
/// ```
pub fn validate_signal_num(num: c_int) -> errno::Result<()> {
    if (libc::SIGHUP <= num && num <= libc::SIGSYS) || (SIGRTMIN() <= num && num <= SIGRTMAX()) {
        Ok(())
    } else {
        Err(errno::Error::new(EINVAL))
    }
}

/// Register the signal handler of `signum`.
///
/// # Safety
///
/// This is considered unsafe because the given handler will be called
/// asynchronously, interrupting whatever the thread was doing and therefore
/// must only do async-signal-safe operations.
///
/// # Arguments
///
/// * `num`: the signal number to be registered.
/// * `handler`: the signal handler function to register.
///
/// # Examples
///
/// ```
/// # extern crate libc;
/// extern crate vmm_sys_util;
/// # use libc::{c_int, c_void, siginfo_t, SA_SIGINFO};
/// use vmm_sys_util::signal::{register_signal_handler, SignalHandler};
///
/// extern "C" fn handle_signal(_: c_int, _: *mut siginfo_t, _: *mut c_void) {}
/// register_signal_handler(0, handle_signal);
/// ```

pub fn register_signal_handler(num: c_int, handler: SignalHandler) -> errno::Result<()> {
    validate_signal_num(num)?;

    // signum specifies the signal and can be any valid signal except
    // SIGKILL and SIGSTOP.
    // [`sigaction`](http://man7.org/linux/man-pages/man2/sigaction.2.html).
    if libc::SIGKILL == num || libc::SIGSTOP == num {
        return Err(errno::Error::new(EINVAL));
    }

    // Safe, because this is a POD struct.
    let mut act: sigaction = unsafe { mem::zeroed() };
    act.sa_sigaction = handler as *const () as usize;
    act.sa_flags = libc::SA_SIGINFO;

    // Block all signals while the `handler` is running.
    // Blocking other signals is needed to make sure the execution of
    // the handler continues uninterrupted if another signal comes.
    if unsafe { sigfillset(&mut act.sa_mask as *mut sigset_t) } < 0 {
        return errno::errno_result();
    }

    // Safe because the parameters are valid and we check the return value.
    match unsafe { sigaction(num, &act, null_mut()) } {
        0 => Ok(()),
        _ => errno::errno_result(),
    }
}

/// Create a `sigset` with given signals.
///
/// An array of signal numbers are added into the signal set by
/// [`sigaddset`](http://man7.org/linux/man-pages/man3/sigaddset.3p.html).
/// This is a helper function used when we want to manipulate signals.
///
/// # Arguments
///
/// * `signals`: signal numbers to be added to the new `sigset`.
///
/// # Examples
///
/// ```
/// # extern crate libc;
/// extern crate vmm_sys_util;
/// # use libc::sigismember;
/// use vmm_sys_util::signal::create_sigset;
///
/// let sigset = create_sigset(&[1]).unwrap();
///
/// unsafe {
///     assert_eq!(sigismember(&sigset, 1), 1);
/// }
/// ```
pub fn create_sigset(signals: &[c_int]) -> errno::Result<sigset_t> {
    // sigset will actually be initialized by sigemptyset below.
    let mut sigset: sigset_t = unsafe { mem::zeroed() };

    // Safe - return value is checked.
    let ret = unsafe { sigemptyset(&mut sigset) };
    if ret < 0 {
        return errno::errno_result();
    }

    for signal in signals {
        // Safe - return value is checked.
        let ret = unsafe { sigaddset(&mut sigset, *signal) };
        if ret < 0 {
            return errno::errno_result();
        }
    }

    Ok(sigset)
}

/// Retrieve the signal mask that is blocked of the current thread.
///
/// Use [`pthread_sigmask`](http://man7.org/linux/man-pages/man3/pthread_sigmask.3.html)
/// to fetch the signal mask which is blocked for the caller, return the signal mask as
/// a vector of c_int.
///
/// # Examples
///
/// ```
/// extern crate vmm_sys_util;
/// use vmm_sys_util::signal::{block_signal, get_blocked_signals};
///
/// block_signal(1).unwrap();
/// assert!(get_blocked_signals().unwrap().contains(&(1)));
/// ```
pub fn get_blocked_signals() -> SignalResult<Vec<c_int>> {
    let mut mask = Vec::new();

    // Safe - return values are checked.
    unsafe {
        let mut old_sigset: sigset_t = mem::zeroed();
        let ret = pthread_sigmask(SIG_BLOCK, null(), &mut old_sigset as *mut sigset_t);
        if ret < 0 {
            return Err(Error::RetrieveSignalMask(ret));
        }

        for num in 0..=SIGRTMAX() {
            if sigismember(&old_sigset, num) > 0 {
                mask.push(num);
            }
        }
    }

    Ok(mask)
}

/// Mask a given signal.
///
/// Set the given signal `num` as blocked.
/// If signal is already blocked, the call will fail with
/// [`SignalAlreadyBlocked`](enum.Error.html#variant.SignalAlreadyBlocked).
///
/// # Arguments
///
/// * `num`: the signal to be masked.
///
/// # Examples
///
/// ```
/// extern crate vmm_sys_util;
/// use vmm_sys_util::signal::block_signal;
///
/// block_signal(1).unwrap();
/// ```
// Allowing comparison chain because rewriting it with match makes the code less readable.
// Also, the risk of having non-exhaustive checks is low.
#[allow(clippy::comparison_chain)]
pub fn block_signal(num: c_int) -> SignalResult<()> {
    let sigset = create_sigset(&[num]).map_err(Error::CreateSigset)?;

    // Safe - return values are checked.
    unsafe {
        let mut old_sigset: sigset_t = mem::zeroed();
        let ret = pthread_sigmask(SIG_BLOCK, &sigset, &mut old_sigset as *mut sigset_t);
        if ret < 0 {
            return Err(Error::BlockSignal(errno::Error::last()));
        }
        // Check if the given signal is already blocked.
        let ret = sigismember(&old_sigset, num);
        if ret < 0 {
            return Err(Error::CompareBlockedSignals(errno::Error::last()));
        } else if ret > 0 {
            return Err(Error::SignalAlreadyBlocked(num));
        }
    }
    Ok(())
}

/// Unmask a given signal.
///
/// # Arguments
///
/// * `num`: the signal to be unmasked.
///
/// # Examples
///
/// ```
/// extern crate vmm_sys_util;
/// use vmm_sys_util::signal::{block_signal, get_blocked_signals, unblock_signal};
///
/// block_signal(1).unwrap();
/// assert!(get_blocked_signals().unwrap().contains(&(1)));
/// unblock_signal(1).unwrap();
/// ```
pub fn unblock_signal(num: c_int) -> SignalResult<()> {
    let sigset = create_sigset(&[num]).map_err(Error::CreateSigset)?;

    // Safe - return value is checked.
    let ret = unsafe { pthread_sigmask(SIG_UNBLOCK, &sigset, null_mut()) };
    if ret < 0 {
        return Err(Error::UnblockSignal(errno::Error::last()));
    }
    Ok(())
}

/// Clear a pending signal.
///
/// # Arguments
///
/// * `num`: the signal to be cleared.
///
/// # Examples
///
/// ```
/// # extern crate libc;
/// extern crate vmm_sys_util;
/// # use libc::{pthread_kill, sigismember, sigpending, sigset_t};
/// # use std::mem;
/// # use std::thread;
/// # use std::time::Duration;
/// use vmm_sys_util::signal::{block_signal, clear_signal, Killable};
///
/// block_signal(1).unwrap();
/// let killable = thread::spawn(move || {
///     thread::sleep(Duration::from_millis(100));
///     unsafe {
///         let mut chkset: sigset_t = mem::zeroed();
///         sigpending(&mut chkset);
///         assert_eq!(sigismember(&chkset, 1), 1);
///     }
/// });
/// unsafe {
///     pthread_kill(killable.pthread_handle(), 1);
/// }
/// clear_signal(1).unwrap();
/// ```
pub fn clear_signal(num: c_int) -> SignalResult<()> {
    let sigset = create_sigset(&[num]).map_err(Error::CreateSigset)?;

    while {
        // This is safe as we are rigorously checking return values
        // of libc calls.
        unsafe {
            let mut siginfo: siginfo_t = mem::zeroed();
            let ts = timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            // Attempt to consume one instance of pending signal. If signal
            // is not pending, the call will fail with EAGAIN or EINTR.
            let ret = sigtimedwait(&sigset, &mut siginfo, &ts);
            if ret < 0 {
                let e = errno::Error::last();
                match e.errno() {
                    EAGAIN | EINTR => {}
                    _ => {
                        return Err(Error::ClearWaitPending(errno::Error::last()));
                    }
                }
            }

            // This sigset will be actually filled with `sigpending` call.
            let mut chkset: sigset_t = mem::zeroed();
            // See if more instances of the signal are pending.
            let ret = sigpending(&mut chkset);
            if ret < 0 {
                return Err(Error::ClearGetPending(errno::Error::last()));
            }

            let ret = sigismember(&chkset, num);
            if ret < 0 {
                return Err(Error::ClearCheckPending(errno::Error::last()));
            }

            // This is do-while loop condition.
            ret != 0
        }
    } {}

    Ok(())
}

/// Trait for threads that can be signalled via `pthread_kill`.
///
/// Note that this is only useful for signals between `SIGRTMIN()` and
/// `SIGRTMAX()` because these are guaranteed to not be used by the C
/// runtime.
///
/// # Safety
///
/// This is marked unsafe because the implementation of this trait must
/// guarantee that the returned `pthread_t` is valid and has a lifetime at
/// least that of the trait object.
pub unsafe trait Killable {
    /// Cast this killable thread as `pthread_t`.
    fn pthread_handle(&self) -> pthread_t;

    /// Send a signal to this killable thread.
    ///
    /// # Arguments
    ///
    /// * `num`: specify the signal
    fn kill(&self, num: c_int) -> errno::Result<()> {
        validate_signal_num(num)?;

        // Safe because we ensure we are using a valid pthread handle,
        // a valid signal number, and check the return result.
        let ret = unsafe { pthread_kill(self.pthread_handle(), num) };
        if ret < 0 {
            return errno::errno_result();
        }
        Ok(())
    }
}

// Safe because we fulfill our contract of returning a genuine pthread handle.
unsafe impl<T> Killable for JoinHandle<T> {
    fn pthread_handle(&self) -> pthread_t {
        // JoinHandleExt::as_pthread_t gives c_ulong, convert it to the
        // type that the libc crate expects
        assert_eq!(mem::size_of::<pthread_t>(), mem::size_of::<usize>());
        self.as_pthread_t() as usize as pthread_t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    // Reserve for each vcpu signal.
    static mut SIGNAL_HANDLER_CALLED: bool = false;

    extern "C" fn handle_signal(_: c_int, _: *mut siginfo_t, _: *mut c_void) {
        unsafe {
            // In the tests, there only uses vcpu signal.
            SIGNAL_HANDLER_CALLED = true;
        }
    }

    fn is_pending(signal: c_int) -> bool {
        unsafe {
            let mut chkset: sigset_t = mem::zeroed();
            sigpending(&mut chkset);
            sigismember(&chkset, signal) == 1
        }
    }

    #[test]
    fn test_register_signal_handler() {
        // testing bad value
        assert!(register_signal_handler(libc::SIGKILL, handle_signal).is_err());
        assert!(register_signal_handler(libc::SIGSTOP, handle_signal).is_err());
        assert!(register_signal_handler(SIGRTMAX() + 1, handle_signal).is_err());
        format!("{:?}", register_signal_handler(SIGRTMAX(), handle_signal));
        assert!(register_signal_handler(SIGRTMIN(), handle_signal).is_ok());
        assert!(register_signal_handler(libc::SIGSYS, handle_signal).is_ok());
    }

    #[test]
    #[allow(clippy::empty_loop)]
    fn test_killing_thread() {
        let killable = thread::spawn(|| thread::current().id());
        let killable_id = killable.join().unwrap();
        assert_ne!(killable_id, thread::current().id());

        // We install a signal handler for the specified signal; otherwise the whole process will
        // be brought down when the signal is received, as part of the default behaviour. Signal
        // handlers are global, so we install this before starting the thread.
        register_signal_handler(SIGRTMIN(), handle_signal)
            .expect("failed to register vcpu signal handler");

        let killable = thread::spawn(|| loop {});

        let res = killable.kill(SIGRTMAX() + 1);
        assert!(res.is_err());
        format!("{:?}", res);

        unsafe {
            assert!(!SIGNAL_HANDLER_CALLED);
        }

        assert!(killable.kill(SIGRTMIN()).is_ok());

        // We're waiting to detect that the signal handler has been called.
        const MAX_WAIT_ITERS: u32 = 20;
        let mut iter_count = 0;
        loop {
            thread::sleep(Duration::from_millis(100));

            if unsafe { SIGNAL_HANDLER_CALLED } {
                break;
            }

            iter_count += 1;
            // timeout if we wait too long
            assert!(iter_count <= MAX_WAIT_ITERS);
        }

        // Our signal handler doesn't do anything which influences the killable thread, so the
        // previous signal is effectively ignored. If we were to join killable here, we would block
        // forever as the loop keeps running. Since we don't join, the thread will become detached
        // as the handle is dropped, and will be killed when the process/main thread exits.
    }

    #[test]
    fn test_block_unblock_signal() {
        let signal = SIGRTMIN();

        // Check if it is blocked.
        unsafe {
            let mut sigset: sigset_t = mem::zeroed();
            pthread_sigmask(SIG_BLOCK, null(), &mut sigset as *mut sigset_t);
            assert_eq!(sigismember(&sigset, signal), 0);
        }

        block_signal(signal).unwrap();
        assert!(get_blocked_signals().unwrap().contains(&(signal)));

        unblock_signal(signal).unwrap();
        assert!(!get_blocked_signals().unwrap().contains(&(signal)));
    }

    #[test]
    fn test_clear_pending() {
        let signal = SIGRTMIN() + 1;

        block_signal(signal).unwrap();

        // Block the signal, which means it won't be delivered until it is
        // unblocked. Pending between the time when the signal which is set as blocked
        // is generated and when is delivered.
        let killable = thread::spawn(move || {
            loop {
                // Wait for the signal being killed.
                thread::sleep(Duration::from_millis(100));
                if is_pending(signal) {
                    clear_signal(signal).unwrap();
                    assert!(!is_pending(signal));
                    break;
                }
            }
        });

        // Send a signal to the thread.
        assert!(killable.kill(SIGRTMIN() + 1).is_ok());
        killable.join().unwrap();
    }
}
