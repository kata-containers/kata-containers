use std::cmp;
use std::io;
use std::os::unix::io::AsRawFd;
use libc;
use iovec::IoVec;
use iovec::unix as iovec;

pub trait VecIo {
    fn readv(&self, bufs: &mut [&mut IoVec]) -> io::Result<usize>;

    fn writev(&self, bufs: &[&IoVec]) -> io::Result<usize>;
}

impl<T: AsRawFd> VecIo for T {
    fn readv(&self, bufs: &mut [&mut IoVec]) -> io::Result<usize> {
        unsafe {
            let slice = iovec::as_os_slice_mut(bufs);
            let len = cmp::min(<libc::c_int>::max_value() as usize, slice.len());
            let rc = libc::readv(self.as_raw_fd(),
                                 slice.as_ptr(),
                                 len as libc::c_int);
            if rc < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(rc as usize)
            }
        }
    }

    fn writev(&self, bufs: &[&IoVec]) -> io::Result<usize> {
        unsafe {
            let slice = iovec::as_os_slice(bufs);
            let len = cmp::min(<libc::c_int>::max_value() as usize, slice.len());
            let rc = libc::writev(self.as_raw_fd(),
                                  slice.as_ptr(),
                                  len as libc::c_int);
            if rc < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(rc as usize)
            }
        }
    }
}