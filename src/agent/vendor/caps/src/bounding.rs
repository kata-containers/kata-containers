use errno;
use libc;

use super::Capability;
use errors::*;
use nr;

pub fn clear() -> Result<()> {
    for c in super::all() {
        if try!(has_cap(c)) {
            try!(drop(c));
        }
    }
    Ok(())
}

pub fn drop(cap: Capability) -> Result<()> {
    let ret = unsafe { libc::prctl(nr::PR_CAPBSET_DROP, libc::c_uint::from(cap.index()), 0, 0) };
    match ret {
        0 => Ok(()),
        _ => {
            Err(Error::from_kind(ErrorKind::Sys(errno::errno()))
                .chain_err(|| "PR_CAPBSET_DROP error"))
        }
    }
}

pub fn has_cap(cap: Capability) -> Result<bool> {
    let ret = unsafe { libc::prctl(nr::PR_CAPBSET_READ, libc::c_uint::from(cap.index()), 0, 0) };
    match ret {
        0 => Ok(false),
        1 => Ok(true),
        _ => {
            Err(Error::from_kind(ErrorKind::Sys(errno::errno()))
                .chain_err(|| "PR_CAPBSET_READ error"))
        }
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
