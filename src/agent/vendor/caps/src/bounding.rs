use crate::errors::CapsError;
use crate::nr;
use crate::runtime;
use crate::Capability;
use std::io::Error;

pub fn clear() -> Result<(), CapsError> {
    for c in super::all() {
        if has_cap(c)? {
            drop(c)?;
        }
    }
    Ok(())
}

pub fn drop(cap: Capability) -> Result<(), CapsError> {
    let ret = unsafe { libc::prctl(nr::PR_CAPBSET_DROP, libc::c_uint::from(cap.index()), 0, 0) };
    match ret {
        0 => Ok(()),
        _ => Err(CapsError::from(format!(
            "PR_CAPBSET_DROP failure: {}",
            Error::last_os_error()
        ))),
    }
}

pub fn has_cap(cap: Capability) -> Result<bool, CapsError> {
    let ret = unsafe { libc::prctl(nr::PR_CAPBSET_READ, libc::c_uint::from(cap.index()), 0, 0) };
    match ret {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(CapsError::from(format!(
            "PR_CAPBSET_READ failure: {}",
            Error::last_os_error()
        ))),
    }
}

pub fn read() -> Result<super::CapsHashSet, CapsError> {
    let mut res = super::CapsHashSet::new();
    for c in runtime::thread_all_supported() {
        if has_cap(c)? {
            res.insert(c);
        }
    }
    Ok(res)
}
