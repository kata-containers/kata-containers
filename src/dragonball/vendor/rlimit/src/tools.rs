use std::io;

/// Returns the value of `kern.maxfilesperproc` by sysctl.
/// # Errors
/// Returns an error if any syscall failed.
// #begin-codegen KERN_MAXFILESPERPROC
// generated from rust-lang/libc ec88c377ab1695d7bdd721332382e7cecc07b7e3
#[cfg(any(
    any(target_os = "macos", target_os = "ios"),
    target_os = "dragonfly",
    target_os = "freebsd",
))]
// #end-codegen KERN_MAXFILESPERPROC
fn get_kern_max_files_per_proc() -> io::Result<u64> {
    use std::mem;
    use std::ptr;

    let mut mib = [libc::CTL_KERN, libc::KERN_MAXFILESPERPROC];
    let mut max_files_per_proc: libc::c_int = 0;
    let mut oldlen = mem::size_of::<libc::c_int>();
    let ret = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            2,
            &mut max_files_per_proc as *mut libc::c_int as *mut libc::c_void,
            &mut oldlen,
            ptr::null_mut(),
            0,
        )
    };

    if ret < 0 {
        return Err(io::Error::last_os_error());
    }

    debug_assert!(max_files_per_proc >= 0);
    Ok(max_files_per_proc as u64)
}

/// Try to increase NOFILE limit and return the current soft limit.
///
/// `lim` is the expected limit which can be up to [`u64::MAX`].
///
/// This function does nothing and returns `Ok(lim)`
/// if `RLIMIT_NOFILE` does not exist on current platform.
///
/// # Errors
/// Returns an error if any syscall failed.
pub fn increase_nofile_limit(lim: u64) -> io::Result<u64> {
    // #begin-codegen RLIMIT_NOFILE
    // generated from rust-lang/libc ec88c377ab1695d7bdd721332382e7cecc07b7e3
    #[cfg(any(
        all(target_os = "linux", target_env = "gnu"),
        all(
            target_os = "linux",
            target_env = "musl",
            any(
                target_arch = "x86",
                target_arch = "mips",
                target_arch = "powerpc",
                target_arch = "hexagon",
                target_arch = "arm"
            )
        ),
        all(
            target_os = "linux",
            target_env = "musl",
            any(
                target_arch = "x86_64",
                target_arch = "aarch64",
                target_arch = "mips64",
                target_arch = "powerpc64"
            )
        ),
        all(target_os = "linux", target_env = "uclibc"),
        any(target_os = "freebsd", target_os = "dragonfly"),
        any(target_os = "macos", target_os = "ios"),
        any(target_os = "openbsd", target_os = "netbsd"),
        target_os = "android",
        target_os = "emscripten",
        target_os = "fuchsia",
        target_os = "haiku",
        target_os = "solarish",
    ))]
    // #end-codegen RLIMIT_NOFILE
    {
        use super::Resource;

        let (soft, hard) = Resource::NOFILE.get()?;

        if soft >= hard {
            return Ok(hard);
        }

        if soft >= lim {
            return Ok(soft);
        }

        let mut lim = lim;

        lim = lim.min(hard);

        // #begin-codegen KERN_MAXFILESPERPROC
        // generated from rust-lang/libc ec88c377ab1695d7bdd721332382e7cecc07b7e3
        #[cfg(any(
            any(target_os = "macos", target_os = "ios"),
            target_os = "dragonfly",
            target_os = "freebsd",
        ))]
        // #end-codegen KERN_MAXFILESPERPROC
        {
            lim = lim.min(get_kern_max_files_per_proc()?)
        }

        Resource::NOFILE.set(lim, hard)?;

        Ok(lim)
    }

    // #begin-codegen not RLIMIT_NOFILE
    // generated from rust-lang/libc ec88c377ab1695d7bdd721332382e7cecc07b7e3
    #[cfg(not(any(
        all(target_os = "linux", target_env = "gnu"),
        all(
            target_os = "linux",
            target_env = "musl",
            any(
                target_arch = "x86",
                target_arch = "mips",
                target_arch = "powerpc",
                target_arch = "hexagon",
                target_arch = "arm"
            )
        ),
        all(
            target_os = "linux",
            target_env = "musl",
            any(
                target_arch = "x86_64",
                target_arch = "aarch64",
                target_arch = "mips64",
                target_arch = "powerpc64"
            )
        ),
        all(target_os = "linux", target_env = "uclibc"),
        any(target_os = "freebsd", target_os = "dragonfly"),
        any(target_os = "macos", target_os = "ios"),
        any(target_os = "openbsd", target_os = "netbsd"),
        target_os = "android",
        target_os = "emscripten",
        target_os = "fuchsia",
        target_os = "haiku",
        target_os = "solarish",
    )))]
    // #end-codegen not RLIMIT_NOFILE
    {
        Ok(lim)
    }
}
