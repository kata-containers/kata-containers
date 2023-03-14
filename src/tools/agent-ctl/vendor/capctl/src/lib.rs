#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod err;
mod sys;

pub mod caps;
pub mod prctl;

pub use caps::*;
pub use err::*;
pub use prctl::*;

#[inline]
unsafe fn raw_prctl(
    option: libc::c_int,
    arg2: libc::c_ulong,
    arg3: libc::c_ulong,
    arg4: libc::c_ulong,
    arg5: libc::c_ulong,
) -> Result<libc::c_int> {
    #[cfg(not(feature = "sc"))]
    return {
        let res = libc::prctl(option, arg2, arg3, arg4, arg5);

        if res >= 0 {
            Ok(res)
        } else {
            Err(Error::last())
        }
    };

    #[cfg(feature = "sc")]
    return sc_res_decode(sc::syscall!(PRCTL, option, arg2, arg3, arg4, arg5))
        .map(|res| res as libc::c_int);
}

#[inline]
unsafe fn raw_prctl_opt(
    option: libc::c_int,
    arg2: libc::c_ulong,
    arg3: libc::c_ulong,
    arg4: libc::c_ulong,
    arg5: libc::c_ulong,
) -> Option<libc::c_int> {
    #[cfg(not(feature = "sc"))]
    return {
        let res = libc::prctl(option, arg2, arg3, arg4, arg5);

        if res >= 0 {
            Some(res)
        } else {
            None
        }
    };

    #[cfg(feature = "sc")]
    return {
        let res = sc::syscall!(PRCTL, option, arg2, arg3, arg4, arg5);

        if res > -4096isize as usize {
            None
        } else {
            Some(res as libc::c_int)
        }
    };
}

#[cfg(feature = "sc")]
#[inline]
fn sc_res_decode(res: usize) -> Result<usize> {
    if res > -4096isize as usize {
        Err(Error::from_code((!res + 1) as i32))
    } else {
        Ok(res)
    }
}
