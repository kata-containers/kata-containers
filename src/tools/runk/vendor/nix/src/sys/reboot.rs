//! Reboot/shutdown or enable/disable Ctrl-Alt-Delete.

use crate::Result;
use crate::errno::Errno;
use std::convert::Infallible;
use std::mem::drop;

libc_enum! {
    /// How exactly should the system be rebooted.
    ///
    /// See [`set_cad_enabled()`](fn.set_cad_enabled.html) for
    /// enabling/disabling Ctrl-Alt-Delete.
    #[repr(i32)]
    #[non_exhaustive]
    pub enum RebootMode {
        /// Halt the system.
        RB_HALT_SYSTEM,
        /// Execute a kernel that has been loaded earlier with
        /// [`kexec_load(2)`](https://man7.org/linux/man-pages/man2/kexec_load.2.html).
        RB_KEXEC,
        /// Stop the system and switch off power, if possible.
        RB_POWER_OFF,
        /// Restart the system.
        RB_AUTOBOOT,
        // we do not support Restart2.
        /// Suspend the system using software suspend.
        RB_SW_SUSPEND,
    }
}

/// Reboots or shuts down the system.
pub fn reboot(how: RebootMode) -> Result<Infallible> {
    unsafe {
        libc::reboot(how as libc::c_int)
    };
    Err(Errno::last())
}

/// Enable or disable the reboot keystroke (Ctrl-Alt-Delete).
///
/// Corresponds to calling `reboot(RB_ENABLE_CAD)` or `reboot(RB_DISABLE_CAD)` in C.
pub fn set_cad_enabled(enable: bool) -> Result<()> {
    let cmd = if enable {
        libc::RB_ENABLE_CAD
    } else {
        libc::RB_DISABLE_CAD
    };
    let res = unsafe {
        libc::reboot(cmd)
    };
    Errno::result(res).map(drop)
}
