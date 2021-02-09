#![doc(
    html_root_url = "https://docs.rs/signal-hook-registry/1.2.1/signal-hook-registry/",
    test(attr(deny(warnings)))
)]
#![deny(missing_docs, warnings)]
#![allow(unknown_lints, renamed_and_remove_lints, bare_trait_objects)]

//! Backend of the [signal-hook] crate.
//!
//! The [signal-hook] crate tries to provide an API to the unix signals, which are a global
//! resource. Therefore, it is desirable an application contains just one version of the crate
//! which manages this global resource. But that makes it impossible to make breaking changes in
//! the API.
//!
//! Therefore, this crate provides very minimal and low level API to the signals that is unlikely
//! to have to change, while there may be multiple versions of the [signal-hook] that all use this
//! low-level API to provide different versions of the high level APIs.
//!
//! It is also possible some other crates might want to build a completely different API. This
//! split allows these crates to still reuse the same low-level routines in this crate instead of
//! going to the (much more dangerous) unix calls.
//!
//! # What this crate provides
//!
//! The only thing this crate does is multiplexing the signals. An application or library can add
//! or remove callbacks and have multiple callbacks for the same signal.
//!
//! It handles dispatching the callbacks and managing them in a way that uses only the
//! [async-signal-safe] functions inside the signal handler. Note that the callbacks are still run
//! inside the signal handler, so it is up to the caller to ensure they are also
//! [async-signal-safe].
//!
//! # What this is for
//!
//! This is a building block for other libraries creating reasonable abstractions on top of
//! signals. The [signal-hook] is the generally preferred way if you need to handle signals in your
//! application and provides several safe patterns of doing so.
//!
//! # Rust version compatibility
//!
//! Currently builds on 1.26.0 an newer and this is very unlikely to change. However, tests
//! require dependencies that don't build there, so tests need newer Rust version (they are run on
//! stable).
//!
//! # Portability
//!
//! This crate includes a limited support for Windows, based on `signal`/`raise` in the CRT.
//! There are differences in both API and behavior:
//!
//! - Due to lack of `siginfo_t`, we don't provide `register_sigaction` or `register_unchecked`.
//! - Due to lack of signal blocking, there's a race condition.
//!   After the call to `signal`, there's a moment where we miss a signal.
//!   That means when you register a handler, there may be a signal which invokes
//!   neither the default handler or the handler you register.
//! - Handlers registered by `signal` in Windows are cleared on first signal.
//!   To match behavior in other platforms, we re-register the handler each time the handler is
//!   called, but there's a moment where we miss a handler.
//!   That means when you receive two signals in a row, there may be a signal which invokes
//!   the default handler, nevertheless you certainly have registered the handler.
//!
//! [signal-hook]: https://docs.rs/signal-hook
//! [async-signal-safe]: http://www.man7.org/linux/man-pages/man7/signal-safety.7.html

extern crate arc_swap;
extern crate libc;

use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::io::Error;
use std::mem;
#[cfg(not(windows))]
use std::ptr;
// Once::new is now a const-fn. But it is not stable in all the rustc versions we want to support
// yet.
#[allow(deprecated)]
use std::sync::ONCE_INIT;
use std::sync::{Arc, Mutex, MutexGuard, Once};

use arc_swap::IndependentArcSwap;
#[cfg(not(windows))]
use libc::{c_int, c_void, sigaction, siginfo_t, sigset_t, SIG_BLOCK, SIG_SETMASK};
#[cfg(windows)]
use libc::{c_int, sighandler_t};

#[cfg(not(windows))]
use libc::{SIGFPE, SIGILL, SIGKILL, SIGSEGV, SIGSTOP};
#[cfg(windows)]
use libc::{SIGFPE, SIGILL, SIGSEGV};

// These constants are not defined in the current version of libc, but it actually
// exists in Windows CRT.
#[cfg(windows)]
const SIG_DFL: sighandler_t = 0;
#[cfg(windows)]
const SIG_IGN: sighandler_t = 1;
#[cfg(windows)]
const SIG_ERR: sighandler_t = !0;

// To simplify implementation. Not to be exposed.
#[cfg(windows)]
#[allow(non_camel_case_types)]
struct siginfo_t;

// # Internal workings
//
// This uses a form of RCU. There's an atomic pointer to the current action descriptors (in the
// form of IndependentArcSwap, to be able to track what, if any, signal handlers still use the
// version). A signal handler takes a copy of the pointer and calls all the relevant actions.
//
// Modifications to that are protected by a mutex, to avoid juggling multiple signal handlers at
// once (eg. not calling sigaction concurrently). This should not be a problem, because modifying
// the signal actions should be initialization only anyway. To avoid all allocations and also
// deallocations inside the signal handler, after replacing the pointer, the modification routine
// needs to busy-wait for the reference count on the old pointer to drop to 1 and take ownership ‒
// that way the one deallocating is the modification routine, outside of the signal handler.

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct ActionId(u64);

/// An ID of registered action.
///
/// This is returned by all the registration routines and can be used to remove the action later on
/// with a call to [`unregister`](fn.unregister.html).
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SigId {
    signal: c_int,
    action: ActionId,
}

// This should be dyn Fn(...), but we want to support Rust 1.26.0 and that one doesn't allow them
// yet.
#[allow(unknown_lints, bare_trait_objects)]
type Action = Fn(&siginfo_t) + Send + Sync;

#[derive(Clone)]
struct Slot {
    #[cfg(windows)]
    prev: sighandler_t,
    #[cfg(not(windows))]
    prev: sigaction,
    // We use BTreeMap here, because we want to run the actions in the order they were inserted.
    // This works, because the ActionIds are assigned in an increasing order.
    actions: BTreeMap<ActionId, Arc<Action>>,
}

impl Slot {
    #[cfg(windows)]
    fn new(signal: libc::c_int) -> Result<Self, Error> {
        let old = unsafe { libc::signal(signal, handler as sighandler_t) };
        if old == SIG_ERR {
            return Err(Error::last_os_error());
        }
        Ok(Slot {
            prev: old,
            actions: BTreeMap::new(),
        })
    }

    #[cfg(not(windows))]
    fn new(signal: libc::c_int) -> Result<Self, Error> {
        // C data structure, expected to be zeroed out.
        let mut new: libc::sigaction = unsafe { mem::zeroed() };
        new.sa_sigaction = handler as usize;
        // Android is broken and uses different int types than the rest (and different depending on
        // the pointer width). This converts the flags to the proper type no matter what it is on
        // the given platform.
        let flags = libc::SA_RESTART | libc::SA_NOCLDSTOP;
        #[allow(unused_assignments)]
        let mut siginfo = flags;
        siginfo = libc::SA_SIGINFO as _;
        let flags = flags | siginfo;
        new.sa_flags = flags as _;
        // C data structure, expected to be zeroed out.
        let mut old: libc::sigaction = unsafe { mem::zeroed() };
        // FFI ‒ pointers are valid, it doesn't take ownership.
        if unsafe { libc::sigaction(signal, &new, &mut old) } != 0 {
            return Err(Error::last_os_error());
        }
        Ok(Slot {
            prev: old,
            actions: BTreeMap::new(),
        })
    }
}

type AllSignals = HashMap<c_int, Slot>;

struct GlobalData {
    all_signals: IndependentArcSwap<AllSignals>,
    rcu_lock: Mutex<u64>,
}

static mut GLOBAL_DATA: Option<GlobalData> = None;
#[allow(deprecated)]
static GLOBAL_INIT: Once = ONCE_INIT;

impl GlobalData {
    fn get() -> &'static Self {
        unsafe { GLOBAL_DATA.as_ref().unwrap() }
    }
    fn ensure() -> &'static Self {
        GLOBAL_INIT.call_once(|| unsafe {
            GLOBAL_DATA = Some(GlobalData {
                all_signals: IndependentArcSwap::from_pointee(HashMap::new()),
                rcu_lock: Mutex::new(0),
            });
        });
        Self::get()
    }
    fn load(&self) -> (AllSignals, MutexGuard<u64>) {
        let lock = self.rcu_lock.lock().unwrap();
        let signals = AllSignals::clone(&self.all_signals.load());
        (signals, lock)
    }
    fn store(&self, signals: AllSignals, lock: MutexGuard<u64>) {
        let signals = Arc::new(signals);
        // We are behind a mutex, so we can safely replace it without any RCU on the ArcSwap side.
        self.all_signals.store(signals);
        drop(lock);
    }
}

#[cfg(windows)]
extern "C" fn handler(sig: c_int) {
    if sig != SIGFPE {
        // Windows CRT `signal` resets handler every time, unless for SIGFPE.
        // Reregister the handler to retain maximal compatibility.
        // Problems:
        // - It's racy. But this is inevitably racy in Windows.
        // - Interacts poorly with handlers outside signal-hook-registry.
        let old = unsafe { libc::signal(sig, handler as sighandler_t) };
        if old == SIG_ERR {
            // MSDN doesn't describe which errors might occur,
            // but we can tell from the Linux manpage that
            // EINVAL (invalid signal number) is mostly the only case.
            // Therefore, this branch must not occur.
            // In any case we can do nothing useful in the signal handler,
            // so we're going to abort silently.
            unsafe {
                libc::abort();
            }
        }
    }

    let signals = GlobalData::get().all_signals.load_signal_safe();

    if let Some(ref slot) = signals.get(&sig) {
        let fptr = slot.prev;
        if fptr != 0 && fptr != SIG_DFL && fptr != SIG_IGN {
            // FFI ‒ calling the original signal handler.
            unsafe {
                let action = mem::transmute::<usize, extern "C" fn(c_int)>(fptr);
                action(sig);
            }
        }

        for action in slot.actions.values() {
            action(&siginfo_t);
        }
    }
}

#[cfg(not(windows))]
extern "C" fn handler(sig: c_int, info: *mut siginfo_t, data: *mut c_void) {
    let signals = GlobalData::get().all_signals.load_signal_safe();

    if let Some(ref slot) = signals.get(&sig) {
        let fptr = slot.prev.sa_sigaction;
        if fptr != 0 && fptr != libc::SIG_DFL && fptr != libc::SIG_IGN {
            // FFI ‒ calling the original signal handler.
            unsafe {
                // Android is broken and uses different int types than the rest (and different
                // depending on the pointer width). This converts the flags to the proper type no
                // matter what it is on the given platform.
                //
                // The trick is to create the same-typed variable as the sa_flags first and then
                // set it to the proper value (does Rust have a way to copy a type in a different
                // way?)
                #[allow(unused_assignments)]
                let mut siginfo = slot.prev.sa_flags;
                siginfo = libc::SA_SIGINFO as _;
                if slot.prev.sa_flags & siginfo == 0 {
                    let action = mem::transmute::<usize, extern "C" fn(c_int)>(fptr);
                    action(sig);
                } else {
                    type SigAction = extern "C" fn(c_int, *mut siginfo_t, *mut c_void);
                    let action = mem::transmute::<usize, SigAction>(fptr);
                    action(sig, info, data);
                }
            }
        }

        let info = unsafe { info.as_ref() };
        let info = info.unwrap_or_else(|| {
            // The info being null seems to be illegal according to POSIX, but has been observed on
            // some probably broken platform. We can't do anything about that, that is just broken,
            // but we are not allowed to panic in a signal handler, so we are left only with simply
            // aborting. We try to write a message what happens, but using the libc stuff
            // (`eprintln` is not guaranteed to be async-signal-safe).
            unsafe {
                const MSG: &[u8] =
                    b"Platform broken, got NULL as siginfo to signal handler. Aborting";
                libc::write(2, MSG.as_ptr() as *const _, MSG.len());
                libc::abort();
            }
        });

        for action in slot.actions.values() {
            action(info);
        }
    }
}

#[cfg(not(windows))]
fn block_signal(signal: c_int) -> Result<sigset_t, Error> {
    unsafe {
        // The mem::unitialized is deprecated because it is hard to use correctly in Rust. But
        // MaybeUninit is new and not supported by all the rustc's we want to support. Furthermore,
        // sigset_t is a C type anyway and rust limitations should not apply to it, right?
        #[allow(deprecated)]
        let mut newsigs: sigset_t = mem::uninitialized();
        libc::sigemptyset(&mut newsigs);
        libc::sigaddset(&mut newsigs, signal);
        #[allow(deprecated)]
        let mut oldsigs: sigset_t = mem::uninitialized();
        libc::sigemptyset(&mut oldsigs);
        if libc::sigprocmask(SIG_BLOCK, &newsigs, &mut oldsigs) == 0 {
            Ok(oldsigs)
        } else {
            Err(Error::last_os_error())
        }
    }
}

#[cfg(not(windows))]
fn restore_signals(signals: libc::sigset_t) -> Result<(), Error> {
    if unsafe { libc::sigprocmask(SIG_SETMASK, &signals, ptr::null_mut()) } == 0 {
        Ok(())
    } else {
        Err(Error::last_os_error())
    }
}

#[cfg(windows)]
fn without_signal<F: FnOnce() -> Result<(), Error>>(_signal: c_int, f: F) -> Result<(), Error> {
    // We don't have such a mechanism in Windows.
    f()
}

#[cfg(not(windows))]
fn without_signal<F: FnOnce() -> Result<(), Error>>(signal: c_int, f: F) -> Result<(), Error> {
    let old_signals = block_signal(signal)?;
    let result = f();
    let restored = restore_signals(old_signals);
    // In case of errors in both, prefer the one in result.
    result.and(restored)
}

/// List of forbidden signals.
///
/// Some signals are impossible to replace according to POSIX and some are so special that this
/// library refuses to handle them (eg. SIGSEGV). The routines panic in case registering one of
/// these signals is attempted.
///
/// See [`register`](fn.register.html).
pub const FORBIDDEN: &[c_int] = FORBIDDEN_IMPL;

#[cfg(windows)]
const FORBIDDEN_IMPL: &[c_int] = &[SIGILL, SIGFPE, SIGSEGV];
#[cfg(not(windows))]
const FORBIDDEN_IMPL: &[c_int] = &[SIGKILL, SIGSTOP, SIGILL, SIGFPE, SIGSEGV];

/// Registers an arbitrary action for the given signal.
///
/// This makes sure there's a signal handler for the given signal. It then adds the action to the
/// ones called each time the signal is delivered. If multiple actions are set for the same signal,
/// all are called, in the order of registration.
///
/// If there was a previous signal handler for the given signal, it is chained ‒ it will be called
/// as part of this library's signal handler, before any actions set through this function.
///
/// On success, the function returns an ID that can be used to remove the action again with
/// [`unregister`](fn.unregister.html).
///
/// # Panics
///
/// If the signal is one of (see [`FORBIDDEN`]):
///
/// * `SIGKILL`
/// * `SIGSTOP`
/// * `SIGILL`
/// * `SIGFPE`
/// * `SIGSEGV`
///
/// The first two are not possible to override (and the underlying C functions simply ignore all
/// requests to do so, which smells of possible bugs, or return errors). The rest can be set, but
/// generally needs very special handling to do so correctly (direct manipulation of the
/// application's address space, `longjmp` and similar). Unless you know very well what you're
/// doing, you'll shoot yourself into the foot and this library won't help you with that.
///
/// # Errors
///
/// Since the library manipulates signals using the low-level C functions, all these can return
/// errors. Generally, the errors mean something like the specified signal does not exist on the
/// given platform ‒ ofter a program is debugged and tested on a given OS, it should never return
/// an error.
///
/// However, if an error *is* returned, there are no guarantees if the given action was registered
/// or not.
///
/// # Safety
///
/// This function is unsafe, because the `action` is run inside a signal handler. The set of
/// functions allowed to be called from within is very limited (they are called signal-safe
/// functions by POSIX). These specifically do *not* contain mutexes and memory
/// allocation/deallocation. They *do* contain routines to terminate the program, to further
/// manipulate signals (by the low-level functions, not by this library) and to read and write file
/// descriptors. Calling program's own functions consisting only of these is OK, as is manipulating
/// program's variables ‒ however, as the action can be called on any thread that does not have the
/// given signal masked (by default no signal is masked on any thread), and mutexes are a no-go,
/// this is harder than it looks like at first.
///
/// As panicking from within a signal handler would be a panic across FFI boundary (which is
/// undefined behavior), the passed handler must not panic.
///
/// If you find these limitations hard to satisfy, choose from the helper functions in submodules
/// of this library ‒ these provide safe interface to use some common signal handling patters.
///
/// # Race condition
///
/// Currently, there's a short race condition. If this is the first action for the given signal,
/// there was another signal handler previously and the signal comes into a different thread during
/// this function, it can happen the original handler is not chained in this one instance.
///
/// This is considered unimportant problem, since most programs install their signal handlers
/// during startup, before the signals effectively do anything. If you want to avoid the race
/// condition completely, initialize all signal handling before starting any threads.
///
/// # Performance
///
/// Even when it is possible to repeatedly install and remove actions during the lifetime of a
/// program, the installation and removal is considered a slow operation and should not be done
/// very often. Also, there's limited (though huge) amount of distinct IDs (they are `u64`).
///
/// # Examples
///
/// ```rust
/// extern crate signal_hook;
///
/// use std::io::Error;
/// use std::process;
///
/// fn main() -> Result<(), Error> {
///     let signal = unsafe { signal_hook::register(signal_hook::SIGTERM, || process::abort()) }?;
///     // Stuff here...
///     signal_hook::unregister(signal); // Not really necessary.
///     Ok(())
/// }
/// ```
pub unsafe fn register<F>(signal: c_int, action: F) -> Result<SigId, Error>
where
    F: Fn() + Sync + Send + 'static,
{
    register_sigaction_impl(signal, move |_: &_| action())
}

/// Register a signal action.
///
/// This acts in the same way as [`register`], including the drawbacks, panics and performance
/// characteristics. The only difference is the provided action accepts a [`siginfo_t`] argument,
/// providing information about the received signal.
///
/// # Safety
///
/// See the details of [`register`].
#[cfg(not(windows))]
pub unsafe fn register_sigaction<F>(signal: c_int, action: F) -> Result<SigId, Error>
where
    F: Fn(&siginfo_t) + Sync + Send + 'static,
{
    register_sigaction_impl(signal, action)
}

unsafe fn register_sigaction_impl<F>(signal: c_int, action: F) -> Result<SigId, Error>
where
    F: Fn(&siginfo_t) + Sync + Send + 'static,
{
    assert!(
        !FORBIDDEN.contains(&signal),
        "Attempted to register forbidden signal {}",
        signal,
    );
    register_unchecked_impl(signal, action)
}

/// Register a signal action without checking for forbidden signals.
///
/// This acts in the same way as [`register_unchecked`], including the drawbacks, panics and
/// performance characteristics. The only difference is the provided action doesn't accept a
/// [`siginfo_t`] argument.
///
/// # Safety
///
/// See the details of [`register`].
pub unsafe fn register_signal_unchecked<F>(signal: c_int, action: F) -> Result<SigId, Error>
where
    F: Fn() + Sync + Send + 'static,
{
    register_unchecked_impl(signal, move |_: &_| action())
}

/// Register a signal action without checking for forbidden signals.
///
/// This acts the same way as [`register_sigaction`], but without checking for the [`FORBIDDEN`]
/// signals. All the signal passed are registered and it is up to the caller to make some sense of
/// them.
///
/// Note that you really need to know what you're doing if you change eg. the `SIGSEGV` signal
/// handler. Generally, you don't want to do that. But unlike the other functions here, this
/// function still allows you to do it.
///
/// # Safety
///
/// See the details of [`register`].
#[cfg(not(windows))]
pub unsafe fn register_unchecked<F>(signal: c_int, action: F) -> Result<SigId, Error>
where
    F: Fn(&siginfo_t) + Sync + Send + 'static,
{
    register_unchecked_impl(signal, action)
}

unsafe fn register_unchecked_impl<F>(signal: c_int, action: F) -> Result<SigId, Error>
where
    F: Fn(&siginfo_t) + Sync + Send + 'static,
{
    let globals = GlobalData::ensure();
    let (mut signals, mut lock) = globals.load();
    let id = ActionId(*lock);
    *lock += 1;
    let action = Arc::from(action);
    without_signal(signal, || {
        match signals.entry(signal) {
            Entry::Occupied(mut occupied) => {
                assert!(occupied.get_mut().actions.insert(id, action).is_none());
            }
            Entry::Vacant(place) => {
                let mut slot = Slot::new(signal)?;
                slot.actions.insert(id, action);
                place.insert(slot);
            }
        }

        globals.store(signals, lock);

        Ok(())
    })?;

    Ok(SigId { signal, action: id })
}

/// Removes a previously installed action.
///
/// This function does nothing if the action was already removed. It returns true if it was removed
/// and false if the action wasn't found.
///
/// It can unregister all the actions installed by [`register`](fn.register.html) as well as the
/// ones from helper submodules.
///
/// # Warning
///
/// This does *not* currently return the default/previous signal handler if the last action for a
/// signal was just unregistered. That means that if you replaced for example `SIGTERM` and then
/// removed the action, the program will effectively ignore `SIGTERM` signals from now on, not
/// terminate on them as is the default action. This is OK if you remove it as part of a shutdown,
/// but it is not recommended to remove termination actions during the normal runtime of
/// application (unless the desired effect is to create something that can be terminated only by
/// SIGKILL).
pub fn unregister(id: SigId) -> bool {
    let globals = GlobalData::ensure();
    let (mut signals, lock) = globals.load();
    let mut replace = false;
    if let Some(slot) = signals.get_mut(&id.signal) {
        replace = slot.actions.remove(&id.action).is_some();
    }
    if replace {
        globals.store(signals, lock);
    }
    replace
}

/// Removes all previously installed actions for a given signal.
///
/// This is similar to the [`unregister`](fn.unregister.html) function, with the sole difference it
/// removes all actions for the given signal.
///
/// Returns if any hooks were actually removed (returns false if there was no hook registered for
/// the signal).
///
/// # Warning
///
/// Similar to [`unregister`](fn.unregister.html), this does not manipulate the signal handler in
/// the OS, it only removes the hooks on the Rust side.
///
/// Furthermore, this will remove *all* signal hooks of the given signal. These may have been
/// registered by some library or unrelated part of the program. Therefore, this should be only
/// used by the top-level application.
pub fn unregister_signal(signal: c_int) -> bool {
    let globals = GlobalData::ensure();
    let (mut signals, lock) = globals.load();
    let mut replace = false;
    if let Some(slot) = signals.get_mut(&signal) {
        if !slot.actions.is_empty() {
            slot.actions.clear();
            replace = true;
        }
    }
    if replace {
        globals.store(signals, lock);
    }
    replace
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[cfg(not(windows))]
    use libc::{pid_t, SIGUSR1, SIGUSR2};

    #[cfg(windows)]
    use libc::SIGTERM as SIGUSR1;
    #[cfg(windows)]
    use libc::SIGTERM as SIGUSR2;

    use super::*;

    #[test]
    #[should_panic]
    fn panic_forbidden() {
        let _ = unsafe { register(SIGILL, || ()) };
    }

    /// Registering the forbidden signals is allowed in the _unchecked version.
    #[test]
    fn forbidden_raw() {
        unsafe { register_signal_unchecked(SIGFPE, || std::process::abort()).unwrap() };
    }

    #[test]
    fn signal_without_pid() {
        let status = Arc::new(AtomicUsize::new(0));
        let action = {
            let status = Arc::clone(&status);
            move || {
                status.store(1, Ordering::Relaxed);
            }
        };
        unsafe {
            register(SIGUSR2, action).unwrap();
            libc::raise(SIGUSR2);
        }
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(100));
            let current = status.load(Ordering::Relaxed);
            match current {
                // Not yet
                0 => continue,
                // Good, we are done with the correct result
                _ if current == 1 => return,
                _ => panic!("Wrong result value {}", current),
            }
        }
        panic!("Timed out waiting for the signal");
    }

    #[test]
    #[cfg(not(windows))]
    fn signal_with_pid() {
        let status = Arc::new(AtomicUsize::new(0));
        let action = {
            let status = Arc::clone(&status);
            move |siginfo: &siginfo_t| {
                // Hack: currently, libc exposes only the first 3 fields of siginfo_t. The pid
                // comes somewhat later on. Therefore, we do a Really Ugly Hack and define our
                // own structure (and hope it is correct on all platforms). But hey, this is
                // only the tests, so we are going to get away with this.
                #[repr(C)]
                struct SigInfo {
                    _fields: [c_int; 3],
                    #[cfg(all(target_pointer_width = "64", target_os = "linux"))]
                    _pad: c_int,
                    pid: pid_t,
                }
                let s: &SigInfo = unsafe {
                    (siginfo as *const _ as usize as *const SigInfo)
                        .as_ref()
                        .unwrap()
                };
                status.store(s.pid as usize, Ordering::Relaxed);
            }
        };
        let pid;
        unsafe {
            pid = libc::getpid();
            register_sigaction(SIGUSR2, action).unwrap();
            libc::raise(SIGUSR2);
        }
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(100));
            let current = status.load(Ordering::Relaxed);
            match current {
                // Not yet (PID == 0 doesn't happen)
                0 => continue,
                // Good, we are done with the correct result
                _ if current == pid as usize => return,
                _ => panic!("Wrong status value {}", current),
            }
        }
        panic!("Timed out waiting for the signal");
    }

    /// Check that registration works as expected and that unregister tells if it did or not.
    #[test]
    fn register_unregister() {
        let signal = unsafe { register(SIGUSR1, || ()).unwrap() };
        // It was there now, so we can unregister
        assert!(unregister(signal));
        // The next time unregistering does nothing and tells us so.
        assert!(!unregister(signal));
    }
}
