//! mlock / munlock

#![cfg(feature = "use_os")]


/// Cross-platform `mlock`.
///
/// * Unix `mlock`.
/// * Windows `VirtualLock`.
pub unsafe fn mlock(addr: *mut u8, len: usize) -> bool {
    #[cfg(unix)] {
        #[cfg(target_os = "linux")]
        libc::madvise(addr as *mut libc::c_void, len, libc::MADV_DONTDUMP);

        #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
        libc::madvise(addr as *mut libc::c_void, len, libc::MADV_NOCORE);

        libc::mlock(addr as *mut libc::c_void, len) == 0
    }

    #[cfg(windows)] {
        windows_sys::Win32::System::Memory::VirtualLock(addr.cast(), len) != 0
    }
}

/// Cross-platform `munlock`.
///
/// * Unix `munlock`.
/// * Windows `VirtualUnlock`.
pub unsafe fn munlock(addr: *mut u8, len: usize) -> bool {
    crate::memzero(addr, len);

    #[cfg(unix)] {
        #[cfg(target_os = "linux")]
        libc::madvise(addr as *mut libc::c_void, len, libc::MADV_DODUMP);

        #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
        libc::madvise(addr as *mut libc::c_void, len, libc::MADV_CORE);

        libc::munlock(addr as *mut libc::c_void, len) == 0
    }

    #[cfg(windows)] {
        windows_sys::Win32::System::Memory::VirtualUnlock(addr.cast(), len) != 0
    }
}
