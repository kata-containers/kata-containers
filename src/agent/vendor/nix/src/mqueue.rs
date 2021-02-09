//! Posix Message Queue functions
//!
//! [Further reading and details on the C API](http://man7.org/linux/man-pages/man7/mq_overview.7.html)

use crate::Result;
use crate::errno::Errno;

use libc::{self, c_char, c_long, mqd_t, size_t};
use std::ffi::CString;
use crate::sys::stat::Mode;
use std::mem;

libc_bitflags!{
    pub struct MQ_OFlag: libc::c_int {
        O_RDONLY;
        O_WRONLY;
        O_RDWR;
        O_CREAT;
        O_EXCL;
        O_NONBLOCK;
        O_CLOEXEC;
    }
}

libc_bitflags!{
    pub struct FdFlag: libc::c_int {
        FD_CLOEXEC;
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MqAttr {
    mq_attr: libc::mq_attr,
}

impl MqAttr {
    pub fn new(mq_flags: c_long,
               mq_maxmsg: c_long,
               mq_msgsize: c_long,
               mq_curmsgs: c_long)
               -> MqAttr
    {
        let mut attr = mem::MaybeUninit::<libc::mq_attr>::uninit();
        unsafe {
            let p = attr.as_mut_ptr();
            (*p).mq_flags = mq_flags;
            (*p).mq_maxmsg = mq_maxmsg;
            (*p).mq_msgsize = mq_msgsize;
            (*p).mq_curmsgs = mq_curmsgs;
            MqAttr { mq_attr: attr.assume_init() }
        }
    }

    pub fn flags(&self) -> c_long {
        self.mq_attr.mq_flags
    }
}


/// Open a message queue
///
/// See also [`mq_open(2)`](http://pubs.opengroup.org/onlinepubs/9699919799/functions/mq_open.html)
// The mode.bits cast is only lossless on some OSes
#[allow(clippy::cast_lossless)]
pub fn mq_open(name: &CString,
               oflag: MQ_OFlag,
               mode: Mode,
               attr: Option<&MqAttr>)
               -> Result<mqd_t> {
    let res = match attr {
        Some(mq_attr) => unsafe {
            libc::mq_open(name.as_ptr(),
                          oflag.bits(),
                          mode.bits() as libc::c_int,
                          &mq_attr.mq_attr as *const libc::mq_attr)
        },
        None => unsafe { libc::mq_open(name.as_ptr(), oflag.bits()) },
    };
    Errno::result(res)
}

/// Remove a message queue
///
/// See also [`mq_unlink(2)`](http://pubs.opengroup.org/onlinepubs/9699919799/functions/mq_unlink.html)
pub fn mq_unlink(name: &CString) -> Result<()> {
    let res = unsafe { libc::mq_unlink(name.as_ptr()) };
    Errno::result(res).map(drop)
}

/// Close a message queue
///
/// See also [`mq_close(2)`](http://pubs.opengroup.org/onlinepubs/9699919799/functions/mq_close.html)
pub fn mq_close(mqdes: mqd_t) -> Result<()> {
    let res = unsafe { libc::mq_close(mqdes) };
    Errno::result(res).map(drop)
}

/// Receive a message from a message queue
///
/// See also [`mq_receive(2)`](http://pubs.opengroup.org/onlinepubs/9699919799/functions/mq_receive.html)
pub fn mq_receive(mqdes: mqd_t, message: &mut [u8], msg_prio: &mut u32) -> Result<usize> {
    let len = message.len() as size_t;
    let res = unsafe {
        libc::mq_receive(mqdes,
                         message.as_mut_ptr() as *mut c_char,
                         len,
                         msg_prio as *mut u32)
    };
    Errno::result(res).map(|r| r as usize)
}

/// Send a message to a message queue
///
/// See also [`mq_send(2)`](http://pubs.opengroup.org/onlinepubs/9699919799/functions/mq_send.html)
pub fn mq_send(mqdes: mqd_t, message: &[u8], msq_prio: u32) -> Result<()> {
    let res = unsafe {
        libc::mq_send(mqdes,
                      message.as_ptr() as *const c_char,
                      message.len(),
                      msq_prio)
    };
    Errno::result(res).map(drop)
}

/// Get message queue attributes
///
/// See also [`mq_getattr(2)`](http://pubs.opengroup.org/onlinepubs/9699919799/functions/mq_getattr.html)
pub fn mq_getattr(mqd: mqd_t) -> Result<MqAttr> {
    let mut attr = mem::MaybeUninit::<libc::mq_attr>::uninit();
    let res = unsafe { libc::mq_getattr(mqd, attr.as_mut_ptr()) };
    Errno::result(res).map(|_| unsafe{MqAttr { mq_attr: attr.assume_init() }})
}

/// Set the attributes of the message queue. Only `O_NONBLOCK` can be set, everything else will be ignored
/// Returns the old attributes
/// It is recommend to use the `mq_set_nonblock()` and `mq_remove_nonblock()` convenience functions as they are easier to use
///
/// [Further reading](http://pubs.opengroup.org/onlinepubs/9699919799/functions/mq_setattr.html)
pub fn mq_setattr(mqd: mqd_t, newattr: &MqAttr) -> Result<MqAttr> {
    let mut attr = mem::MaybeUninit::<libc::mq_attr>::uninit();
    let res = unsafe {
        libc::mq_setattr(mqd, &newattr.mq_attr as *const libc::mq_attr, attr.as_mut_ptr())
    };
    Errno::result(res).map(|_| unsafe{ MqAttr { mq_attr: attr.assume_init() }})
}

/// Convenience function.
/// Sets the `O_NONBLOCK` attribute for a given message queue descriptor
/// Returns the old attributes
pub fn mq_set_nonblock(mqd: mqd_t) -> Result<MqAttr> {
    let oldattr = mq_getattr(mqd)?;
    let newattr = MqAttr::new(c_long::from(MQ_OFlag::O_NONBLOCK.bits()),
                              oldattr.mq_attr.mq_maxmsg,
                              oldattr.mq_attr.mq_msgsize,
                              oldattr.mq_attr.mq_curmsgs);
    mq_setattr(mqd, &newattr)
}

/// Convenience function.
/// Removes `O_NONBLOCK` attribute for a given message queue descriptor
/// Returns the old attributes
pub fn mq_remove_nonblock(mqd: mqd_t) -> Result<MqAttr> {
    let oldattr = mq_getattr(mqd)?;
    let newattr = MqAttr::new(0,
                              oldattr.mq_attr.mq_maxmsg,
                              oldattr.mq_attr.mq_msgsize,
                              oldattr.mq_attr.mq_curmsgs);
    mq_setattr(mqd, &newattr)
}
