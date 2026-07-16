// Copyright 2022 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use std::io;
use std::os::unix::io::{AsRawFd, RawFd};

use vmm_sys_util::aio::{IoContext, IoControlBlock, IoEvent, IOCB_FLAG_RESFD};
use vmm_sys_util::aio::{IOCB_CMD_PREADV, IOCB_CMD_PWRITEV};
use vmm_sys_util::eventfd::EventFd;

use super::IoEngine;
use crate::block::IoDataDesc;

/// Use AIO to perform asynchronous IO requests.
pub struct Aio {
    fd: RawFd,
    aio_evtfd: EventFd,
    aio_context: IoContext,
}

impl Aio {
    /// Creates a new Aio instence.
    ///
    /// # Arguments
    /// * `nr_events`: maximum number of concurrently processing IO operations.
    pub fn new(fd: RawFd, nr_events: u32) -> io::Result<Self> {
        let aio_context = IoContext::new(nr_events)?;
        Ok(Self {
            fd,
            aio_evtfd: EventFd::new(0)?,
            aio_context,
        })
    }
}

impl IoEngine for Aio {
    fn event_fd(&self) -> &EventFd {
        &self.aio_evtfd
    }

    // NOTE: aio doesn't seem to support negative offsets.
    fn readv(
        &mut self,
        offset: i64,
        iovecs: &mut Vec<IoDataDesc>,
        user_data: u64,
    ) -> io::Result<usize> {
        let iocbs = [&mut IoControlBlock {
            aio_fildes: self.fd as u32,
            aio_lio_opcode: IOCB_CMD_PREADV as u16,
            aio_resfd: self.aio_evtfd.as_raw_fd() as u32,
            aio_flags: IOCB_FLAG_RESFD,
            aio_buf: iovecs.as_mut_ptr() as u64,
            aio_offset: offset,
            aio_nbytes: iovecs.len() as u64,
            aio_data: user_data,
            ..Default::default()
        }];

        self.aio_context.submit(&iocbs[..])
    }

    fn writev(
        &mut self,
        offset: i64,
        iovecs: &mut Vec<IoDataDesc>,
        user_data: u64,
    ) -> io::Result<usize> {
        let iocbs = [&mut IoControlBlock {
            aio_fildes: self.fd as u32,
            aio_lio_opcode: IOCB_CMD_PWRITEV as u16,
            aio_resfd: self.aio_evtfd.as_raw_fd() as u32,
            aio_flags: IOCB_FLAG_RESFD,
            aio_buf: iovecs.as_mut_ptr() as u64,
            aio_offset: offset,
            aio_nbytes: iovecs.len() as u64,
            aio_data: user_data,
            ..Default::default()
        }];

        self.aio_context.submit(&iocbs[..])
    }

    // For currently supported LocalFile and TdcFile backend, it must not return temporary errors
    // and may only return permanent errors. So the virtio-blk driver layer will not try to
    // recover and only pass errors up onto the device manager. When changing the error handling
    // policy, please do help to update BlockEpollHandler::io_complete().
    fn complete(&mut self) -> io::Result<Vec<(u64, i64)>> {
        let count = self.aio_evtfd.read()?;
        let mut v = Vec::with_capacity(count as usize);
        if count > 0 {
            let mut events =
                vec![
                    unsafe { std::mem::MaybeUninit::<IoEvent>::zeroed().assume_init() };
                    count as usize
                ];
            while v.len() < count as usize {
                let r = self.aio_context.get_events(1, &mut events[0..], None)?;
                for event in events.iter().take(r) {
                    let index = event.data;
                    let res2 = event.res;
                    v.push((index, res2));
                }
            }
        }
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Seek, SeekFrom, Write};

    use vmm_sys_util::tempfile::TempFile;

    use super::*;

    #[test]
    fn aio_engine() {
        let temp_file = TempFile::new().unwrap();
        let mut aio = Aio::new(temp_file.as_file().as_raw_fd(), 128).unwrap();
        let buf = vec![0xffu8; 0x1000];
        aio.writev(
            0,
            &mut vec![IoDataDesc {
                data_addr: buf.as_ptr() as u64,
                data_len: 0x10,
            }],
            0x123,
        )
        .unwrap();
        let com_res = aio.complete().unwrap();
        for cr in com_res {
            assert_eq!(cr.0, 0x123);
            assert_eq!(cr.1, 0x10);
        }
        let mut rbuf = vec![0u8; 0x100];
        let rn = temp_file.as_file().read(&mut rbuf).unwrap();
        assert_eq!(rn, 0x10);
        assert_eq!(&rbuf[..0x10], &vec![0xff; 0x10]);

        //temp_file.as_file().seek(SeekFrom::End(0x20)).unwrap();
        temp_file.as_file().seek(SeekFrom::Start(0x120)).unwrap();
        temp_file.as_file().write_all(&[0xeeu8; 0x20]).unwrap();

        let rbuf = vec![0u8; 0x100];
        let ret = aio.readv(
            -0x20,
            &mut vec![IoDataDesc {
                data_addr: rbuf.as_ptr() as u64,
                data_len: 0x20,
            }],
            0x456,
        );
        assert_eq!(ret.unwrap_err().kind(), io::ErrorKind::InvalidInput);
        aio.readv(
            0x120,
            &mut vec![IoDataDesc {
                data_addr: rbuf.as_ptr() as u64,
                data_len: 0x20,
            }],
            0x456,
        )
        .unwrap();
        let com_res = aio.complete().unwrap();
        for cr in com_res {
            assert_eq!(cr.0, 0x456);
            assert_eq!(cr.1, 0x20);
        }
        assert_eq!(&rbuf[..0x20], &vec![0xee; 0x20]);
    }
}
