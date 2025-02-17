// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::fs::File;
use std::io::{Error as IoError, Read, Result as IoResult, Write};
use std::net::UdpSocket;
use std::os::raw::*;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};

use vmm_sys_util::ioctl::{ioctl_with_mut_ref, ioctl_with_ref, ioctl_with_val};
use vmm_sys_util::{ioctl_ioc_nr, ioctl_iow_nr};

use crate::net::net_gen;

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
    #[error("cannot create tap devic. {0}")]
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
    pub(crate) if_name: [u8; IFACE_NAME_MAX_LEN],
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

// Returns a byte vector representing the contents of a null terminated C string which
// contains if_name.
fn build_terminated_if_name(if_name: &str) -> Result<[u8; IFACE_NAME_MAX_LEN]> {
    // Convert the string slice to bytes, and shadow the variable,
    // since we no longer need the &str version.
    let if_name = if_name.as_bytes();

    if if_name.len() >= IFACE_NAME_MAX_LEN {
        return Err(Error::InvalidIfname);
    }

    let mut terminated_if_name = [b'\0'; IFACE_NAME_MAX_LEN];
    terminated_if_name[..if_name.len()].copy_from_slice(if_name);

    Ok(terminated_if_name)
}

impl Tap {
    /// Create a TUN/TAP device given the interface name.
    /// # Arguments
    ///
    /// * `if_name` - the name of the interface.
    /// # Example
    ///
    /// ```no_run
    /// use dbs_utils::net::Tap;
    /// Tap::open_named("doc-test-tap", false).unwrap();
    /// ```
    pub fn open_named(if_name: &str, multi_vq: bool) -> Result<Tap> {
        let terminated_if_name = build_terminated_if_name(if_name)?;

        // This is pretty messy because of the unions used by ifreq. Since we
        // don't call as_mut on the same union field more than once, this block
        // is safe.
        let mut ifreq: net_gen::ifreq = Default::default();
        unsafe {
            let ifrn_name = ifreq.ifr_ifrn.ifrn_name.as_mut();
            ifrn_name.copy_from_slice(terminated_if_name.as_ref());
            let ifru_flags = ifreq.ifr_ifru.ifru_flags.as_mut();
            *ifru_flags = (net_gen::IFF_TAP
                | net_gen::IFF_NO_PI
                | net_gen::IFF_VNET_HDR
                | if multi_vq {
                    net_gen::IFF_MULTI_QUEUE
                } else {
                    0
                }) as c_short;
        }

        Tap::create_tap_with_ifreq(&mut ifreq)
    }

    fn create_tap_with_ifreq(ifreq: &mut net_gen::ifreq) -> Result<Tap> {
        let fd = unsafe {
            // Open calls are safe because we give a constant null-terminated
            // string and verify the result.
            libc::open(
                b"/dev/net/tun\0".as_ptr() as *const c_char,
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
        let ret = unsafe { ioctl_with_mut_ref(&tuntap, TUNSETIFF(), ifreq) };

        if ret < 0 {
            return Err(Error::CreateTap(IoError::last_os_error()));
        }

        // Safe since only the name is accessed, and it's cloned out.
        Ok(Tap {
            tap_file: tuntap,
            if_name: unsafe { *ifreq.ifr_ifrn.ifrn_name.as_ref() },
            if_flags: unsafe { *ifreq.ifr_ifru.ifru_flags.as_ref() },
        })
    }

    /// Change the origin tap into multiqueue taps.
    pub fn into_mq_taps(self, vq_pairs: usize) -> Result<Vec<Tap>> {
        let mut taps = Vec::new();

        if vq_pairs <= 1 {
            taps.push(self);
            return Ok(taps);
        }

        // Add other socket into the origin tap interface
        for _ in 0..vq_pairs - 1 {
            let mut ifreq = self.get_ifreq();
            let tap = Tap::create_tap_with_ifreq(&mut ifreq)?;

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

        let mut ifreq = self.get_ifreq();

        // We only access one field of the ifru union, hence this is safe.
        unsafe {
            let ifru_flags = ifreq.ifr_ifru.ifru_flags.as_mut();
            *ifru_flags =
                (net_gen::net_device_flags_IFF_UP | net_gen::net_device_flags_IFF_RUNNING) as i16;
        }

        // ioctl is safe. Called with a valid sock fd, and we check the return.
        let ret =
            unsafe { ioctl_with_ref(&sock, c_ulong::from(net_gen::sockios::SIOCSIFFLAGS), &ifreq) };
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

    fn get_ifreq(&self) -> net_gen::ifreq {
        let mut ifreq: net_gen::ifreq = Default::default();

        // This sets the name of the interface, which is the only entry
        // in a single-field union.
        unsafe {
            let ifrn_name = ifreq.ifr_ifrn.ifrn_name.as_mut();
            ifrn_name.clone_from_slice(&self.if_name);

            let flags = ifreq.ifr_ifru.ifru_flags.as_mut();
            *flags = self.if_flags;
        }

        ifreq
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

mod tests {
    #![allow(dead_code)]

    use std::mem;
    use std::net::Ipv4Addr;
    use std::str;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    const SUBNET_MASK: &str = "255.255.255.0";
    const TAP_IP_PREFIX: &str = "192.168.241.";
    const FAKE_MAC: &str = "12:34:56:78:9a:bc";

    // We skip the first 10 bytes because the IFF_VNET_HDR flag is set when the interface
    // is created, and the legacy header is 10 bytes long without a certain flag which
    // is not set in Tap::new().
    const VETH_OFFSET: usize = 10;
    static NEXT_IP: AtomicUsize = AtomicUsize::new(1);

    // Create a sockaddr_in from an IPv4 address, and expose it as
    // an opaque sockaddr suitable for usage by socket ioctls.
    fn create_sockaddr(ip_addr: Ipv4Addr) -> net_gen::sockaddr {
        // IPv4 addresses big-endian (network order), but Ipv4Addr will give us
        // a view of those bytes directly so we can avoid any endian trickiness.
        let addr_in = net_gen::sockaddr_in {
            sin_family: net_gen::AF_INET as u16,
            sin_port: 0,
            sin_addr: unsafe { mem::transmute(ip_addr.octets()) },
            __pad: [0; 8usize],
        };

        unsafe { mem::transmute(addr_in) }
    }
    impl Tap {
        // We do not run unit tests in parallel so we should have no problem
        // assigning the same IP.

        /// Create a new tap interface.
        pub fn new() -> Result<Tap> {
            // The name of the tap should be {module_name}{index} so that
            // we make sure it stays different when tests are run concurrently.
            let next_ip = NEXT_IP.fetch_add(1, Ordering::SeqCst);
            Self::open_named(&format!("dbs_tap{next_ip}"), false)
        }

        /// Set the host-side IP address for the tap interface.
        pub fn set_ip_addr(&self, ip_addr: Ipv4Addr) -> Result<()> {
            let sock = create_socket()?;
            let addr = create_sockaddr(ip_addr);

            let mut ifreq = self.get_ifreq();

            // We only access one field of the ifru union, hence this is safe.
            unsafe {
                let ifru_addr = ifreq.ifr_ifru.ifru_addr.as_mut();
                *ifru_addr = addr;
            }

            // ioctl is safe. Called with a valid sock fd, and we check the return.
            let ret = unsafe {
                ioctl_with_ref(&sock, c_ulong::from(net_gen::sockios::SIOCSIFADDR), &ifreq)
            };
            if ret < 0 {
                return Err(Error::IoctlError(IoError::last_os_error()));
            }

            Ok(())
        }

        /// Set the netmask for the subnet that the tap interface will exist on.
        pub fn set_netmask(&self, netmask: Ipv4Addr) -> Result<()> {
            let sock = create_socket()?;
            let addr = create_sockaddr(netmask);

            let mut ifreq = self.get_ifreq();

            // We only access one field of the ifru union, hence this is safe.
            unsafe {
                let ifru_addr = ifreq.ifr_ifru.ifru_addr.as_mut();
                *ifru_addr = addr;
            }

            // ioctl is safe. Called with a valid sock fd, and we check the return.
            let ret = unsafe {
                ioctl_with_ref(
                    &sock,
                    c_ulong::from(net_gen::sockios::SIOCSIFNETMASK),
                    &ifreq,
                )
            };
            if ret < 0 {
                return Err(Error::IoctlError(IoError::last_os_error()));
            }

            Ok(())
        }
    }

    fn tap_name_to_string(tap: &Tap) -> String {
        let null_pos = tap.if_name.iter().position(|x| *x == 0).unwrap();
        str::from_utf8(&tap.if_name[..null_pos])
            .unwrap()
            .to_string()
    }

    #[test]
    fn test_tap_name() {
        // Sanity check that the assumed max iface name length is correct.
        assert_eq!(
            IFACE_NAME_MAX_LEN,
            net_gen::ifreq__bindgen_ty_1::default()
                .bindgen_union_field
                .len()
        );

        // 16 characters - too long.
        let name = "a123456789abcdef";
        match Tap::open_named(name, false) {
            Err(Error::InvalidIfname) => (),
            _ => panic!("Expected Error::InvalidIfname"),
        };

        // 15 characters - OK.
        let name = "a123456789abcde";
        let tap = Tap::open_named(name, false).unwrap();
        assert_eq!(
            name,
            std::str::from_utf8(&tap.if_name[0..(IFACE_NAME_MAX_LEN - 1)]).unwrap()
        );
    }

    #[test]
    fn test_tap_partial_eq() {
        assert_ne!(Tap::new().unwrap(), Tap::new().unwrap());
    }

    #[test]
    fn test_tap_configure() {
        // `fetch_add` adds to the current value, returning the previous value.
        let next_ip = NEXT_IP.fetch_add(1, Ordering::SeqCst);

        let tap = Tap::new().unwrap();
        let ip_addr: Ipv4Addr = format!("{TAP_IP_PREFIX}{next_ip}").parse().unwrap();
        let netmask: Ipv4Addr = SUBNET_MASK.parse().unwrap();

        let ret = tap.set_ip_addr(ip_addr);
        assert!(ret.is_ok());
        let ret = tap.set_netmask(netmask);
        assert!(ret.is_ok());
    }

    #[test]
    #[ignore = "Issue #10821 - IO Safety violation: owned file descriptor already closed"]
    fn test_set_options() {
        // This line will fail to provide an initialized FD if the test is not run as root.
        let tap = Tap::new().unwrap();
        tap.set_vnet_hdr_size(16).unwrap();
        tap.set_offload(0).unwrap();

        let faulty_tap = Tap {
            tap_file: unsafe { File::from_raw_fd(i32::MAX) },
            if_name: [0x01; 16],
            if_flags: 0,
        };
        assert!(faulty_tap.set_vnet_hdr_size(16).is_err());
        assert!(faulty_tap.set_offload(0).is_err());
    }

    #[test]
    fn test_tap_enable() {
        let tap = Tap::new().unwrap();
        let ret = tap.enable();
        assert!(ret.is_ok());
    }

    #[test]
    fn test_tap_get_ifreq() {
        let tap = Tap::new().unwrap();
        let ret = tap.get_ifreq();
        assert_eq!(
            "__BindgenUnionField",
            format!("{:?}", ret.ifr_ifrn.ifrn_name)
        );
    }

    #[test]
    fn test_raw_fd() {
        let tap = Tap::new().unwrap();
        assert_eq!(tap.as_raw_fd(), tap.tap_file.as_raw_fd());
    }
}
