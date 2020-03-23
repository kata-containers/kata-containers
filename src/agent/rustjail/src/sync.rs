// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::errors::*;
use nix::errno::Errno;
use nix::unistd;
use nix::Error;
use std::mem;
use std::os::unix::io::RawFd;

pub const SYNC_SUCCESS: i32 = 1;
pub const SYNC_FAILED: i32 = 2;
pub const SYNC_DATA: i32 = 3;

const DATA_SIZE: usize = 100;
const MSG_SIZE: usize = mem::size_of::<i32>();

#[macro_export]
macro_rules! log_child {
    ($fd:expr, $($arg:tt)+) => ({
        let lfd = $fd;
        let mut log_str = format_args!($($arg)+).to_string();
        log_str.push('\n');
        write_count(lfd, log_str.as_bytes(), log_str.len());
    })
}

pub fn write_count(fd: RawFd, buf: &[u8], count: usize) -> Result<usize> {
    let mut len = 0;

    loop {
        match unistd::write(fd, &buf[len..]) {
            Ok(l) => {
                len += l;
                if len == count {
                    break;
                }
            }

            Err(e) => {
                if e != Error::from_errno(Errno::EINTR) {
                    return Err(e.into());
                }
            }
        }
    }

    Ok(len)
}

fn read_count(fd: RawFd, count: usize) -> Result<Vec<u8>> {
    let mut v: Vec<u8> = vec![0; count];
    let mut len = 0;

    loop {
        match unistd::read(fd, &mut v[len..]) {
            Ok(l) => {
                len += l;
                if len == count || l == 0 {
                    break;
                }
            }

            Err(e) => {
                if e != Error::from_errno(Errno::EINTR) {
                    return Err(e.into());
                }
            }
        }
    }

    Ok(v[0..len].to_vec())
}

pub fn read_sync(fd: RawFd) -> Result<Vec<u8>> {
    let buf = read_count(fd, MSG_SIZE)?;
    if buf.len() != MSG_SIZE {
        return Err(ErrorKind::ErrorCode(format!(
            "process: {} failed to receive sync message from peer: got msg length: {}, expected: {}",
            std::process::id(),
            buf.len(),
            MSG_SIZE
        ))
        .into());
    }
    let buf_array: [u8; MSG_SIZE] = [buf[0], buf[1], buf[2], buf[3]];
    let msg: i32 = i32::from_be_bytes(buf_array);
    match msg {
        SYNC_SUCCESS => return Ok(Vec::new()),
        SYNC_DATA => {
            let buf = read_count(fd, MSG_SIZE)?;
            let buf_array: [u8; MSG_SIZE] = [buf[0], buf[1], buf[2], buf[3]];
            let msg_length: i32 = i32::from_be_bytes(buf_array);
            let data_buf = read_count(fd, msg_length as usize)?;

            return Ok(data_buf);
        }
        SYNC_FAILED => {
            let mut error_buf = vec![];
            loop {
                let buf = read_count(fd, DATA_SIZE)?;

                error_buf.extend(&buf);
                if DATA_SIZE == buf.len() {
                    continue;
                } else {
                    break;
                }
            }

            let error_str = match std::str::from_utf8(&error_buf) {
                Ok(v) => v,
                Err(e) => {
                    return Err(ErrorKind::ErrorCode(format!(
                        "receive error message from child process failed: {:?}",
                        e
                    ))
                    .into())
                }
            };

            return Err(ErrorKind::ErrorCode(String::from(error_str)).into());
        }
        _ => return Err(ErrorKind::ErrorCode("error in receive sync message".to_string()).into()),
    }
}

pub fn write_sync(fd: RawFd, msg_type: i32, data_str: &str) -> Result<()> {
    let buf = msg_type.to_be_bytes();

    let count = write_count(fd, &buf, MSG_SIZE)?;
    if count != MSG_SIZE {
        return Err(ErrorKind::ErrorCode("error in send sync message".to_string()).into());
    }

    match msg_type {
        SYNC_FAILED => match write_count(fd, data_str.as_bytes(), data_str.len()) {
            Ok(_count) => unistd::close(fd)?,
            Err(e) => {
                unistd::close(fd)?;
                return Err(
                    ErrorKind::ErrorCode("error in send message to process".to_string()).into(),
                );
            }
        },
        SYNC_DATA => {
            let length: i32 = data_str.len() as i32;
            match write_count(fd, &length.to_be_bytes(), MSG_SIZE) {
                Ok(_count) => (),
                Err(e) => {
                    unistd::close(fd)?;
                    return Err(ErrorKind::ErrorCode(
                        "error in send message to process".to_string(),
                    )
                    .into());
                }
            }

            match write_count(fd, data_str.as_bytes(), data_str.len()) {
                Ok(_count) => (),
                Err(e) => {
                    unistd::close(fd)?;
                    return Err(ErrorKind::ErrorCode(
                        "error in send message to process".to_string(),
                    )
                    .into());
                }
            }
        }

        _ => (),
    };

    Ok(())
}
