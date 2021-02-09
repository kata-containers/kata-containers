use std::cell::UnsafeCell;
use std::env;
use std::ffi::OsString;
use std::fmt;
#[allow(deprecated)] // to allow for older Rust versions (<1.24)
use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};
use std::sync::Mutex;

pub use super::backtrace::Backtrace;

const GENERAL_BACKTRACE: &str = "RUST_BACKTRACE";
const FAILURE_BACKTRACE: &str = "RUST_FAILURE_BACKTRACE";

pub(super) struct InternalBacktrace {
    backtrace: Option<MaybeResolved>,
}

struct MaybeResolved {
    resolved: Mutex<bool>,
    backtrace: UnsafeCell<Backtrace>,
}

unsafe impl Send for MaybeResolved {}
unsafe impl Sync for MaybeResolved {}

impl InternalBacktrace {
    pub(super) fn new() -> InternalBacktrace {
        #[allow(deprecated)] // to allow for older Rust versions (<1.24)
        static ENABLED: AtomicUsize = ATOMIC_USIZE_INIT;

        match ENABLED.load(Ordering::SeqCst) {
            0 => {
                let enabled = is_backtrace_enabled(|var| env::var_os(var));
                ENABLED.store(enabled as usize + 1, Ordering::SeqCst);
                if !enabled {
                    return InternalBacktrace { backtrace: None }
                }
            }
            1 => return InternalBacktrace { backtrace: None },
            _ => {}
        }

        InternalBacktrace {
            backtrace: Some(MaybeResolved {
                resolved: Mutex::new(false),
                backtrace: UnsafeCell::new(Backtrace::new_unresolved()),
            }),
        }
    }

    pub(super) fn none() -> InternalBacktrace {
        InternalBacktrace { backtrace: None }
    }

    pub(super) fn as_backtrace(&self) -> Option<&Backtrace> {
        let bt = match self.backtrace {
            Some(ref bt) => bt,
            None => return None,
        };
        let mut resolved = bt.resolved.lock().unwrap();
        unsafe {
            if !*resolved {
                (*bt.backtrace.get()).resolve();
                *resolved = true;
            }
            Some(&*bt.backtrace.get())
        }
    }

    pub(super) fn is_none(&self) -> bool {
        self.backtrace.is_none()
    }
}

impl fmt::Debug for InternalBacktrace {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("InternalBacktrace")
            .field("backtrace", &self.as_backtrace())
            .finish()
    }
}

fn is_backtrace_enabled<F: Fn(&str) -> Option<OsString>>(get_var: F) -> bool {
    match get_var(FAILURE_BACKTRACE) {
        Some(ref val) if val != "0" => true,
        Some(ref val) if val == "0" => false,
        _ => match get_var(GENERAL_BACKTRACE) {
            Some(ref val) if val != "0" => true,
            _                           => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const YEA: Option<&str> = Some("1");
    const NAY: Option<&str> = Some("0");
    const NOT_SET: Option<&str> = None;

    macro_rules! test_enabled {
        (failure: $failure:ident, general: $general:ident => $result:expr) => {{
            assert_eq!(is_backtrace_enabled(|var| match var {
                FAILURE_BACKTRACE   => $failure.map(OsString::from),
                GENERAL_BACKTRACE   => $general.map(OsString::from),
                _                   => panic!()
            }), $result);
        }}
    }

    #[test]
    fn always_enabled_if_failure_is_set_to_yes() {
        test_enabled!(failure: YEA, general: YEA => true);
        test_enabled!(failure: YEA, general: NOT_SET => true);
        test_enabled!(failure: YEA, general: NAY => true);
    }

    #[test]
    fn never_enabled_if_failure_is_set_to_no() {
        test_enabled!(failure: NAY, general: YEA => false);
        test_enabled!(failure: NAY, general: NOT_SET => false);
        test_enabled!(failure: NAY, general: NAY => false);
    }

    #[test]
    fn follows_general_if_failure_is_not_set() {
        test_enabled!(failure: NOT_SET, general: YEA => true);
        test_enabled!(failure: NOT_SET, general: NOT_SET => false);
        test_enabled!(failure: NOT_SET, general: NAY => false);
    }
}
