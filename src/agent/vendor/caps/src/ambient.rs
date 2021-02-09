use errno;
use libc;

use super::Capability;
use errors::*;
use nr;

pub fn clear() -> Result<()> {
    let ret = unsafe { libc::prctl(nr::PR_CAP_AMBIENT, nr::PR_CAP_AMBIENT_CLEAR_ALL, 0, 0, 0) };
    match ret {
        0 => Ok(()),
        _ => Err(Error::from_kind(ErrorKind::Sys(errno::errno()))
            .chain_err(|| "PR_CAP_AMBIENT_CLEAR_ALL error")),
    }
}

pub fn drop(cap: Capability) -> Result<()> {
    let ret = unsafe {
        libc::prctl(
            nr::PR_CAP_AMBIENT,
            nr::PR_CAP_AMBIENT_LOWER,
            libc::c_uint::from(cap.index()),
            0,
            0,
        )
    };
    match ret {
        0 => Ok(()),
        _ => Err(Error::from_kind(ErrorKind::Sys(errno::errno()))
            .chain_err(|| "PR_CAP_AMBIENT_LOWER error")),
    }
}

pub fn has_cap(cap: Capability) -> Result<bool> {
    let ret = unsafe {
        libc::prctl(
            nr::PR_CAP_AMBIENT,
            nr::PR_CAP_AMBIENT_IS_SET,
            libc::c_uint::from(cap.index()),
            0,
            0,
        )
    };
    match ret {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::from_kind(ErrorKind::Sys(errno::errno()))
            .chain_err(|| "PR_CAP_AMBIENT_IS_SET error")),
    }
}

pub fn raise(cap: Capability) -> Result<()> {
    let ret = unsafe {
        libc::prctl(
            nr::PR_CAP_AMBIENT,
            nr::PR_CAP_AMBIENT_RAISE,
            libc::c_uint::from(cap.index()),
            0,
            0,
        )
    };
    match ret {
        0 => Ok(()),
        _ => Err(Error::from_kind(ErrorKind::Sys(errno::errno()))
            .chain_err(|| "PR_CAP_AMBIENT_RAISE error")),
    }
}

pub fn read() -> Result<super::CapsHashSet> {
    let mut res = super::CapsHashSet::new();
    for c in super::all() {
        if try!(has_cap(c)) {
            res.insert(c);
        }
    }
    Ok(res)
}

pub fn set(value: &super::CapsHashSet) -> Result<()> {
    for c in super::all() {
        if value.contains(&c) {
            try!(raise(c));
        } else {
            try!(drop(c));
        };
    }
    Ok(())
}
