mod types;

#[cfg(not(windows))]
pub(crate) mod syscalls;

#[cfg(not(target_os = "wasi"))]
pub use types::{ClockId, DynamicClockId};
#[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
pub use types::{Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags};
pub use types::{Nsecs, Secs, Timespec};
