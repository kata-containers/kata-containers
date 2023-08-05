// Copyright 2019 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::mem::ManuallyDrop;
use std::os::linux::fs::MetadataExt;
use std::os::unix::io::{AsRawFd, RawFd};

use log::{info, warn};
use virtio_bindings::bindings::virtio_blk::{VIRTIO_BLK_S_IOERR, VIRTIO_BLK_S_OK};

use super::{IoDataDesc, IoEngine, Ufile};

pub struct LocalFile<E> {
    pub(crate) file: ManuallyDrop<File>,
    no_drop: bool,
    capacity: u64,
    io_engine: E,
}

impl<E> LocalFile<E> {
    /// Creates a LocalFile instance.
    pub fn new(mut file: File, no_drop: bool, io_engine: E) -> io::Result<Self> {
        let capacity = file.seek(SeekFrom::End(0))?;

        Ok(Self {
            file: ManuallyDrop::new(file),
            no_drop,
            capacity,
            io_engine,
        })
    }
}

// Implement our own Drop for LocalFile, as we don't want to close LocalFile.file if no_drop is
// enabled.
impl<E> Drop for LocalFile<E> {
    fn drop(&mut self) {
        if self.no_drop {
            info!("LocalFile: no_drop is enabled, don't close file on drop");
        } else {
            // Close the raw fd directly.
            let fd = self.file.as_raw_fd();
            if let Err(e) = nix::unistd::close(fd) {
                warn!("LocalFile: failed to close disk file: {:?}", e);
            }
        }
    }
}

impl<E> Read for LocalFile<E> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.read(buf)
    }
}

impl<E> Write for LocalFile<E> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl<E> Seek for LocalFile<E> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.file.seek(pos)
    }
}

impl<E: IoEngine + Send> Ufile for LocalFile<E> {
    fn get_capacity(&self) -> u64 {
        self.capacity
    }

    fn get_max_size(&self) -> u32 {
        // Set max size to 1M to avoid interferes with rate limiter.
        0x100000
    }

    fn get_device_id(&self) -> io::Result<String> {
        let blk_metadata = self.file.metadata()?;
        // This is how kvmtool does it.
        Ok(format!(
            "{}{}{}",
            blk_metadata.st_dev(),
            blk_metadata.st_rdev(),
            blk_metadata.st_ino()
        ))
    }

    fn get_data_evt_fd(&self) -> RawFd {
        self.io_engine.event_fd().as_raw_fd()
    }

    fn io_read_submit(
        &mut self,
        offset: i64,
        iovecs: &mut Vec<IoDataDesc>,
        user_data: u16,
    ) -> io::Result<usize> {
        self.io_engine.readv(offset, iovecs, user_data as u64)
    }

    fn io_write_submit(
        &mut self,
        offset: i64,
        iovecs: &mut Vec<IoDataDesc>,
        user_data: u16,
    ) -> io::Result<usize> {
        self.io_engine.writev(offset, iovecs, user_data as u64)
    }

    fn io_complete(&mut self) -> io::Result<Vec<(u16, u32)>> {
        Ok(self
            .io_engine
            .complete()?
            .iter()
            .map(|(user_data, res)| {
                (
                    *user_data as u16,
                    if *res >= 0 {
                        VIRTIO_BLK_S_OK
                    } else {
                        VIRTIO_BLK_S_IOERR
                    },
                )
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::io::SeekFrom;

    use vmm_sys_util::tempfile::TempFile;

    use super::*;
    use crate::block::aio::Aio;
    use crate::block::io_uring::IoUring;
    use crate::epoll_helper::*;

    const STOP_EVENT: u32 = 0xfeed;

    struct TestHandler;

    impl EpollHelperHandler for TestHandler {
        fn handle_event(&mut self, _helper: &mut EpollHelper, event: &epoll::Event) -> bool {
            let slot = event.data as u32;
            slot == STOP_EVENT
        }
    }

    fn new_aio_engine() -> Aio {
        let temp_file = TempFile::new().unwrap();
        let aio = Aio::new(temp_file.as_file().as_raw_fd(), 128).unwrap();
        aio
    }

    fn new_iouring_engine() -> IoUring {
        let temp_file = TempFile::new().unwrap();
        let iouring = IoUring::new(temp_file.as_file().as_raw_fd(), 128).unwrap();
        iouring
    }

    #[test]
    fn test_new() {
        // Create with AIO.
        let file = TempFile::new().unwrap().into_file();
        let file_with_aio = LocalFile::new(file, false, new_aio_engine());
        assert!(file_with_aio.is_ok());

        // Create with IO_Uring.
        let file = TempFile::new().unwrap().into_file();
        let file_with_iouring = LocalFile::new(file, false, new_iouring_engine());
        assert!(file_with_iouring.is_ok());
    }

    fn have_target_fd(fd: i32, filename: &OsStr) -> bool {
        let mut path = std::path::PathBuf::from("/proc/self/fd");
        path.push(fd.to_string());
        if path.exists() {
            let entry = path.read_link().unwrap();
            if entry
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .contains(filename.to_str().unwrap())
            {
                return true;
            }
        }
        false
    }

    #[test]
    fn test_drop() {
        // Droped case.
        let tempfile = TempFile::new().unwrap();
        let filename = tempfile.as_path().file_name().unwrap().to_owned();
        let file = tempfile.into_file();
        let fd_of_file = file.as_raw_fd();
        let file_with_aio = LocalFile::new(file, false, new_aio_engine()).unwrap();

        assert!(have_target_fd(fd_of_file, &filename));
        drop(file_with_aio);
        assert!(!have_target_fd(fd_of_file, &filename));

        let tempfile = TempFile::new().unwrap();
        let filename = tempfile.as_path().file_name().unwrap().to_owned();
        let file = tempfile.into_file();
        let fd_of_file = file.as_raw_fd();
        let file_with_iouring = LocalFile::new(file, false, new_iouring_engine()).unwrap();

        assert!(have_target_fd(fd_of_file, &filename));
        drop(file_with_iouring);
        assert!(!have_target_fd(fd_of_file, &filename));

        // No-drop case.
        let tempfile = TempFile::new().unwrap();
        let filename = tempfile.as_path().file_name().unwrap().to_owned();
        let file = tempfile.into_file();
        let fd_of_file = file.as_raw_fd();
        let file_with_aio = LocalFile::new(file, true, new_aio_engine()).unwrap();

        assert!(have_target_fd(fd_of_file, &filename));
        drop(file_with_aio);
        assert!(have_target_fd(fd_of_file, &filename));

        let tempfile = TempFile::new().unwrap();
        let filename = tempfile.as_path().file_name().unwrap().to_owned();
        let file = tempfile.into_file();
        let fd_of_file = file.as_raw_fd();
        let file_with_iouring = LocalFile::new(file, true, new_iouring_engine()).unwrap();

        assert!(have_target_fd(fd_of_file, &filename));
        drop(file_with_iouring);
        assert!(have_target_fd(fd_of_file, &filename));
    }

    #[test]
    fn test_read_write_flush_seek() {
        let original_content = b"hello world";
        let size_of_content = original_content.len();
        let file = TempFile::new().unwrap().into_file();
        let mut file_with_aio = LocalFile::new(file, false, new_aio_engine()).unwrap();
        let bytes_write = file_with_aio.write(original_content).unwrap();
        assert_eq!(bytes_write, size_of_content);
        file_with_aio.flush().unwrap();
        file_with_aio.rewind().unwrap();
        let mut content = vec![0; 11];
        let bytes_read = file_with_aio.read(&mut content).unwrap();
        assert_eq!(bytes_read, size_of_content);
        assert_eq!(content, original_content);

        let original_content = b"hello world";
        let file = TempFile::new().unwrap().into_file();
        let mut file_with_iouring = LocalFile::new(file, false, new_iouring_engine()).unwrap();
        let bytes_write = file_with_iouring.write(original_content).unwrap();
        assert_eq!(bytes_write, size_of_content);
        file_with_iouring.flush().unwrap();
        let start: usize = 6;
        file_with_iouring
            .seek(SeekFrom::Start(start as u64))
            .unwrap();
        let mut content = vec![0; size_of_content - start];
        let bytes_read = file_with_iouring.read(&mut content).unwrap();
        assert_eq!(bytes_read, size_of_content - start);
        assert_eq!(content, original_content[start..]);
    }

    #[test]
    fn test_get_capacity() {
        let mut file = TempFile::new().unwrap().into_file();
        let original_content = b"hello world";
        let size_of_content = original_content.len();
        let bytes_write = file.write(original_content).unwrap();
        assert_eq!(bytes_write, size_of_content);
        file.rewind().unwrap();
        let file_with_aio = LocalFile::new(file, false, new_aio_engine()).unwrap();
        assert_eq!(file_with_aio.get_capacity(), size_of_content as u64);

        let mut file = TempFile::new().unwrap().into_file();
        let original_content = b"hello world";
        let size_of_content = original_content.len();
        let bytes_write = file.write(original_content).unwrap();
        assert_eq!(bytes_write, size_of_content);
        file.rewind().unwrap();
        let file_with_iouring = LocalFile::new(file, false, new_iouring_engine()).unwrap();
        assert_eq!(file_with_iouring.get_capacity(), size_of_content as u64);
    }

    #[test]
    fn test_get_max_capacity() {
        let file = TempFile::new().unwrap().into_file();
        let file_with_aio = LocalFile::new(file, false, new_aio_engine()).unwrap();
        assert_eq!(file_with_aio.get_max_size(), 0x100000);

        let file = TempFile::new().unwrap().into_file();
        let file_with_iouring = LocalFile::new(file, false, new_iouring_engine()).unwrap();
        assert_eq!(file_with_iouring.get_max_size(), 0x100000);
    }

    #[test]
    fn test_get_device_id() {
        let file = TempFile::new().unwrap().into_file();
        let file_with_aio = LocalFile::new(file, false, new_aio_engine()).unwrap();
        assert!(file_with_aio.get_device_id().is_ok());
        let metadata = file_with_aio.file.metadata().unwrap();
        assert_eq!(
            file_with_aio.get_device_id().unwrap(),
            format!(
                "{}{}{}",
                metadata.st_dev(),
                metadata.st_rdev(),
                metadata.st_ino()
            )
        );

        let file = TempFile::new().unwrap().into_file();
        let file_with_iouring = LocalFile::new(file, false, new_iouring_engine()).unwrap();
        assert!(file_with_iouring.get_device_id().is_ok());
        let metadata = file_with_iouring.file.metadata().unwrap();
        assert_eq!(
            file_with_iouring.get_device_id().unwrap(),
            format!(
                "{}{}{}",
                metadata.st_dev(),
                metadata.st_rdev(),
                metadata.st_ino()
            )
        );
    }

    #[test]
    fn test_get_data_evt_fd() {
        let file = TempFile::new().unwrap();
        let aio = Aio::new(file.as_file().as_raw_fd(), 128).unwrap();
        let file_with_aio = LocalFile::new(file.into_file(), false, aio).unwrap();
        assert_eq!(
            file_with_aio.get_data_evt_fd(),
            file_with_aio.io_engine.event_fd().as_raw_fd()
        );

        let file = TempFile::new().unwrap();
        let iouring = IoUring::new(file.as_file().as_raw_fd(), 128).unwrap();
        let file_with_iouring = LocalFile::new(file.into_file(), false, iouring).unwrap();
        assert_eq!(
            file_with_iouring.get_data_evt_fd(),
            file_with_iouring.io_engine.event_fd().as_raw_fd()
        );
    }

    #[test]
    fn test_io_write_submit() {
        // Test with Aio.
        let file = TempFile::new().unwrap();
        let aio = Aio::new(file.as_file().as_raw_fd(), 128).unwrap();
        let mut file_with_aio = LocalFile::new(file.into_file(), false, aio).unwrap();
        let buf = vec![0xffu8; 0xff];
        file_with_aio
            .io_write_submit(
                8,
                &mut vec![IoDataDesc {
                    data_addr: buf.as_ptr() as u64,
                    data_len: 0x8_usize,
                }],
                0x12,
            )
            .unwrap();
        let res = file_with_aio.io_complete().unwrap();

        for element in res {
            assert_eq!(element.0, 0x12);
            assert_eq!(element.1, VIRTIO_BLK_S_OK);
        }

        // Test with IoUring.
        let file = TempFile::new().unwrap();
        let iouring = IoUring::new(file.as_file().as_raw_fd(), 128).unwrap();
        let mut helper = EpollHelper::new().unwrap();
        helper
            .add_event(iouring.event_fd().as_raw_fd(), 0xfeed)
            .unwrap();
        let mut file_with_iouring = LocalFile::new(file.into_file(), false, iouring).unwrap();
        let mut handler = TestHandler;
        let buf = vec![0xffu8; 0xff];
        file_with_iouring
            .io_write_submit(
                8,
                &mut vec![IoDataDesc {
                    data_addr: buf.as_ptr() as u64,
                    data_len: 0x8_usize,
                }],
                0x12,
            )
            .unwrap();
        helper.run(&mut handler).unwrap();
        let res = file_with_iouring.io_complete().unwrap();

        for element in res {
            assert_eq!(element.0, 0x12);
            assert_eq!(element.1, VIRTIO_BLK_S_OK);
        }
    }

    #[test]
    fn test_io_read_submit() {
        // Test with Aio.
        let file = TempFile::new().unwrap();
        file.as_file().seek(SeekFrom::Start(0x120)).unwrap();
        file.as_file().write_all(&[0xeeu8; 0x20]).unwrap();
        let aio = Aio::new(file.as_file().as_raw_fd(), 128).unwrap();
        let mut file_with_aio = LocalFile::new(file.into_file(), false, aio).unwrap();
        let rbuf = vec![0u8; 0x100];
        let ret = file_with_aio.io_read_submit(
            -0x20,
            &mut vec![IoDataDesc {
                data_addr: rbuf.as_ptr() as u64,
                data_len: 0x20,
            }],
            0x456,
        );
        assert_eq!(ret.unwrap_err().kind(), io::ErrorKind::InvalidInput);

        file_with_aio
            .io_read_submit(
                0x120,
                &mut vec![IoDataDesc {
                    data_addr: rbuf.as_ptr() as u64,
                    data_len: 0x20,
                }],
                0x456,
            )
            .unwrap();
        let com_res = file_with_aio.io_complete().unwrap();
        for element in com_res {
            assert_eq!(element.0, 0x456);
            assert_eq!(element.1, VIRTIO_BLK_S_OK);
        }
        assert_eq!(&rbuf[..0x20], &vec![0xee; 0x20]);

        // Test with IoUring.
        let file = TempFile::new().unwrap();
        file.as_file().seek(SeekFrom::Start(0x120)).unwrap();
        file.as_file().write_all(&[0xeeu8; 0x20]).unwrap();
        let iouring = IoUring::new(file.as_file().as_raw_fd(), 128).unwrap();
        let mut helper = EpollHelper::new().unwrap();
        helper
            .add_event(iouring.event_fd().as_raw_fd(), 0xfeed)
            .unwrap();
        let mut file_with_iouring = LocalFile::new(file.into_file(), false, iouring).unwrap();
        let mut handler = TestHandler;
        let rbuf = vec![0u8; 0x100];

        file_with_iouring
            .io_read_submit(
                0x120,
                &mut vec![IoDataDesc {
                    data_addr: rbuf.as_ptr() as u64,
                    data_len: 0x20,
                }],
                0x456,
            )
            .unwrap();
        helper.run(&mut handler).unwrap();
        let com_res = file_with_iouring.io_complete().unwrap();
        for element in com_res {
            assert_eq!(element.0, 0x456);
            assert_eq!(element.1, VIRTIO_BLK_S_OK);
        }
        assert_eq!(&rbuf[..0x20], &vec![0xee; 0x20]);
    }
}
