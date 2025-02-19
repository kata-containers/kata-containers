// Copyright 2024 Kata Contributors
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::ffi::CStr;
use std::fs::File;
use std::io::{Error as IoError, Read, Result as IoResult, Write};
use std::net::UdpSocket;
use std::os::raw::*;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};

use libc::ifreq;
use vmm_sys_util::ioctl::{ioctl_with_mut_ref, ioctl_with_ref, ioctl_with_val};
use vmm_sys_util::{ioctl_ioc_nr, ioctl_iow_nr};
// As defined in the Linux UAPI:
// https://elixir.bootlin.com/linux/v4.17/source/include/uapi/linux/if.h#L33
pub(crate) const IFACE_NAME_MAX_LEN: usize = 16;

/// List of errors the tap implementation can throw.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Failed to create a socket.
    #[error("cannot create socket. {0}")]
    CreateSocket(#[source] IoError),

    /// Unable to create tap interface.
    #[error("cannot create tap device. {0}")]
    CreateTap(IoError),

    /// Invalid interface name.
    #[error("invalid network interface name")]
    InvalidIfname,

    /// ioctl failed.
    #[error("failure while issue Tap ioctl command. {0}")]
    IoctlError(#[source] IoError),

    /// Couldn't open /dev/net/tun.
    #[error("cannot open tap device. {0}")]
    OpenTun(#[source] IoError),
}

pub type Result<T> = ::std::result::Result<T, Error>;

const TUNTAP: ::std::os::raw::c_uint = 84;
ioctl_iow_nr!(TUNSETIFF, TUNTAP, 202, ::std::os::raw::c_int);
ioctl_iow_nr!(TUNSETOFFLOAD, TUNTAP, 208, ::std::os::raw::c_uint);
ioctl_iow_nr!(TUNSETVNETHDRSZ, TUNTAP, 216, ::std::os::raw::c_int);

/// Handle for a network tap interface.
///
/// For now, this simply wraps the file descriptor for the tap device so methods
/// can run ioctls on the interface. The tap interface fd will be closed when
/// Tap goes out of scope, and the kernel will clean up the interface automatically.
#[derive(Debug)]
pub struct Tap {
    /// tap device file handle
    pub tap_file: File,
    pub(crate) if_name: [std::os::raw::c_char; IFACE_NAME_MAX_LEN],
    pub(crate) if_flags: std::os::raw::c_short,
}

impl PartialEq for Tap {
    fn eq(&self, other: &Tap) -> bool {
        self.if_name == other.if_name
    }
}
fn create_socket() -> Result<UdpSocket> {
    // This is safe since we check the return value.
    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if sock < 0 {
        return Err(Error::CreateSocket(IoError::last_os_error()));
    }

    // This is safe; nothing else will use or hold onto the raw sock fd.
    Ok(unsafe { UdpSocket::from_raw_fd(sock) })
}

// Returns an array representing the contents of a null-terminated C string
// containing if_name.
pub fn build_terminated_if_name(if_name: &str) -> Result<[c_char; IFACE_NAME_MAX_LEN]> {
    // Convert the string slice to bytes, and shadow the variable,
    // since we no longer need the &str version.
    let if_name_bytes = if_name.as_bytes();

    if if_name_bytes.len() >= IFACE_NAME_MAX_LEN {
        return Err(Error::InvalidIfname);
    }

    let mut terminated_if_name = [0 as c_char; IFACE_NAME_MAX_LEN];
    for (i, &byte) in if_name_bytes.iter().enumerate() {
        terminated_if_name[i] = byte as c_char;
    }

    // 0 is the null terminator for c_char type
    terminated_if_name[if_name_bytes.len()] = 0 as c_char;

    Ok(terminated_if_name)
}

impl Tap {
    /// Create a TUN/TAP device given the interface name.
    /// # Arguments
    ///
    /// * `if_name` - the name of the interface.
    pub fn open_named(if_name: &str, multi_vq: bool) -> Result<Tap> {
        let terminated_if_name = build_terminated_if_name(if_name)?;

        // Initialize an `ifreq` structure with the given interface name
        // and configure its flags for setting up a network interface.
        let mut ifr = ifreq {
            ifr_name: terminated_if_name,
            ifr_ifru: libc::__c_anonymous_ifr_ifru {
                ifru_flags: (libc::IFF_TAP
                    | libc::IFF_NO_PI
                    | libc::IFF_VNET_HDR
                    | if multi_vq { libc::IFF_MULTI_QUEUE } else { 0 })
                    as c_short,
            },
        };
        Tap::create_tap_with_ifreq(&mut ifr)
    }

    fn create_tap_with_ifreq(ifr: &mut ifreq) -> Result<Tap> {
        let fd = unsafe {
            let dev_net_tun = CStr::from_bytes_with_nul(b"/dev/net/tun\0").unwrap_or_else(|_| {
                unreachable!("The string is guaranteed to be null-terminated and valid.")
            });

            // Open calls are safe because we use a CStr, which guarantees a
            // constant null-terminated string.
            libc::open(
                dev_net_tun.as_ptr(),
                libc::O_RDWR | libc::O_NONBLOCK | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(Error::OpenTun(IoError::last_os_error()));
        }

        // We just checked that the fd is valid.
        let tuntap = unsafe { File::from_raw_fd(fd) };

        // ioctl is safe since we call it with a valid tap fd and check the return
        // value.
        let ret = unsafe { ioctl_with_mut_ref(&tuntap, TUNSETIFF(), ifr) };
        if ret < 0 {
            return Err(Error::CreateTap(IoError::last_os_error()));
        }

        Ok(Tap {
            tap_file: tuntap,
            if_name: ifr.ifr_name,
            // This is safe since ifru_flags was correctly initialized earlier.
            if_flags: unsafe { ifr.ifr_ifru.ifru_flags },
        })
    }

    /// Change the origin tap into multiqueue taps.
    pub fn into_mq_taps(self, vq_pairs: usize) -> Result<Vec<Tap>> {
        let mut taps = Vec::with_capacity(vq_pairs);

        if vq_pairs == 1 {
            // vq_pairs cannot be less than 1, so only handle the case where it equals 1.
            taps.push(self);
            return Ok(taps);
        }

        // Add other socket into the origin tap interface.
        for _ in 0..vq_pairs - 1 {
            let mut ifr: ifreq = self.get_ifreq();
            let tap = Tap::create_tap_with_ifreq(&mut ifr)?;

            tap.enable()?;

            taps.push(tap);
        }

        taps.insert(0, self);
        Ok(taps)
    }

    /// Set the offload flags for the tap interface.
    pub fn set_offload(&self, flags: c_uint) -> Result<()> {
        // ioctl is safe. Called with a valid tap fd, and we check the return.
        let ret = unsafe { ioctl_with_val(&self.tap_file, TUNSETOFFLOAD(), c_ulong::from(flags)) };
        if ret < 0 {
            return Err(Error::IoctlError(IoError::last_os_error()));
        }

        Ok(())
    }

    /// Enable the tap interface.
    pub fn enable(&self) -> Result<()> {
        let sock = create_socket()?;

        let mut ifr = self.get_ifreq();
        ifr.ifr_ifru.ifru_flags = (libc::IFF_UP | libc::IFF_RUNNING) as i16;

        // ioctl is safe. Called with a valid sock fd, and we check the return.
        let ret = unsafe { ioctl_with_ref(&sock, c_ulong::from(libc::SIOCSIFFLAGS), &ifr) };
        if ret < 0 {
            return Err(Error::IoctlError(IoError::last_os_error()));
        }

        Ok(())
    }

    /// Set the size of the vnet hdr.
    pub fn set_vnet_hdr_size(&self, size: c_int) -> Result<()> {
        // ioctl is safe. Called with a valid tap fd, and we check the return.
        let ret = unsafe { ioctl_with_ref(&self.tap_file, TUNSETVNETHDRSZ(), &size) };
        if ret < 0 {
            return Err(Error::IoctlError(IoError::last_os_error()));
        }

        Ok(())
    }

    fn get_ifreq(&self) -> ifreq {
        let mut ifr_name = [0 as c_char; libc::IFNAMSIZ];
        ifr_name[..self.if_name.len()].copy_from_slice(&self.if_name);

        // Return an `ifreq` structure with the interface name and flags.
        ifreq {
            ifr_name,
            ifr_ifru: libc::__c_anonymous_ifr_ifru {
                ifru_flags: self.if_flags,
            },
        }
    }

    /// Get the origin flags when interface was created.
    pub fn if_flags(&self) -> u32 {
        self.if_flags as u32
    }
}

impl Read for Tap {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.tap_file.read(buf)
    }
}

impl Write for Tap {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.tap_file.write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

impl AsRawFd for Tap {
    fn as_raw_fd(&self) -> RawFd {
        self.tap_file.as_raw_fd()
    }
}
