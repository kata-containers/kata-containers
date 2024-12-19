// Copyright (C) 2021 Alibaba Cloud Computing. All rights reserved.
// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause

use std::fmt::Debug;
use std::marker::PhantomData;
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::{mem, slice};

use libc::{c_void, iovec};
use vhost_rs::vhost_user::message::{
    VhostUserHeaderFlag, VhostUserInflight, VhostUserMemory, VhostUserMemoryRegion,
    VhostUserMsgValidator, VhostUserProtocolFeatures, VhostUserU64, VhostUserVirtioFeatures,
    VhostUserVringAddr, VhostUserVringState, MAX_MSG_SIZE,
};
use vhost_rs::vhost_user::Error;
use vmm_sys_util::sock_ctrl_msg::ScmSocket;
use vmm_sys_util::tempfile::TempFile;

pub const MAX_ATTACHED_FD_ENTRIES: usize = 32;

pub(crate) trait Req:
    Clone + Copy + Debug + PartialEq + Eq + PartialOrd + Ord + Into<u32>
{
    fn is_valid(&self) -> bool;
}

pub type Result<T> = std::result::Result<T, Error>;

/// Type of requests sending from masters to slaves.
#[repr(u32)]
#[allow(unused, non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MasterReq {
    /// Null operation.
    NOOP = 0,
    /// Get from the underlying vhost implementation the features bit mask.
    GET_FEATURES = 1,
    /// Enable features in the underlying vhost implementation using a bit mask.
    SET_FEATURES = 2,
    /// Set the current Master as an owner of the session.
    SET_OWNER = 3,
    /// No longer used.
    RESET_OWNER = 4,
    /// Set the memory map regions on the slave so it can translate the vring addresses.
    SET_MEM_TABLE = 5,
    /// Set logging shared memory space.
    SET_LOG_BASE = 6,
    /// Set the logging file descriptor, which is passed as ancillary data.
    SET_LOG_FD = 7,
    /// Set the size of the queue.
    SET_VRING_NUM = 8,
    /// Set the addresses of the different aspects of the vring.
    SET_VRING_ADDR = 9,
    /// Set the base offset in the available vring.
    SET_VRING_BASE = 10,
    /// Get the available vring base offset.
    GET_VRING_BASE = 11,
    /// Set the event file descriptor for adding buffers to the vring.
    SET_VRING_KICK = 12,
    /// Set the event file descriptor to signal when buffers are used.
    SET_VRING_CALL = 13,
    /// Set the event file descriptor to signal when error occurs.
    SET_VRING_ERR = 14,
    /// Get the protocol feature bit mask from the underlying vhost implementation.
    GET_PROTOCOL_FEATURES = 15,
    /// Enable protocol features in the underlying vhost implementation.
    SET_PROTOCOL_FEATURES = 16,
    /// Query how many queues the backend supports.
    GET_QUEUE_NUM = 17,
    /// Signal slave to enable or disable corresponding vring.
    SET_VRING_ENABLE = 18,
    /// Ask vhost user backend to broadcast a fake RARP to notify the migration is terminated
    /// for guest that does not support GUEST_ANNOUNCE.
    SEND_RARP = 19,
    /// Set host MTU value exposed to the guest.
    NET_SET_MTU = 20,
    /// Set the socket file descriptor for slave initiated requests.
    SET_SLAVE_REQ_FD = 21,
    /// Send IOTLB messages with struct vhost_iotlb_msg as payload.
    IOTLB_MSG = 22,
    /// Set the endianness of a VQ for legacy devices.
    SET_VRING_ENDIAN = 23,
    /// Fetch the contents of the virtio device configuration space.
    GET_CONFIG = 24,
    /// Change the contents of the virtio device configuration space.
    SET_CONFIG = 25,
    /// Create a session for crypto operation.
    CREATE_CRYPTO_SESSION = 26,
    /// Close a session for crypto operation.
    CLOSE_CRYPTO_SESSION = 27,
    /// Advise slave that a migration with postcopy enabled is underway.
    POSTCOPY_ADVISE = 28,
    /// Advise slave that a transition to postcopy mode has happened.
    POSTCOPY_LISTEN = 29,
    /// Advise that postcopy migration has now completed.
    POSTCOPY_END = 30,
    /// Get a shared buffer from slave.
    GET_INFLIGHT_FD = 31,
    /// Send the shared inflight buffer back to slave
    SET_INFLIGHT_FD = 32,
    /// Upper bound of valid commands.
    MAX_CMD = 33,
}

impl Into<u32> for MasterReq {
    fn into(self) -> u32 {
        self as u32
    }
}

impl Req for MasterReq {
    fn is_valid(&self) -> bool {
        (*self > MasterReq::NOOP) && (*self < MasterReq::MAX_CMD)
    }
}

// Given a slice of sizes and the `skip_size`, return the offset of `skip_size` in the slice.
// For example:
//     let iov_lens = vec![4, 4, 5];
//     let size = 6;
//     assert_eq!(get_sub_iovs_offset(&iov_len, size), (1, 2));
fn get_sub_iovs_offset(iov_lens: &[usize], skip_size: usize) -> (usize, usize) {
    let mut size = skip_size;
    let mut nr_skip = 0;

    for len in iov_lens {
        if size >= *len {
            size -= *len;
            nr_skip += 1;
        } else {
            break;
        }
    }
    (nr_skip, size)
}

/// Common message header for vhost-user requests and replies.
/// A vhost-user message consists of 3 header fields and an optional payload. All numbers are in the
/// machine native byte order.
#[repr(packed)]
#[derive(Copy)]
pub(crate) struct VhostUserMsgHeader<R: Req> {
    request: u32,
    flags: u32,
    size: u32,
    _r: PhantomData<R>,
}

impl<R: Req> Debug for VhostUserMsgHeader<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Point")
            .field("request", &{ self.request })
            .field("flags", &{ self.flags })
            .field("size", &{ self.size })
            .finish()
    }
}

impl<T: Req> VhostUserMsgValidator for VhostUserMsgHeader<T> {
    #[allow(clippy::if_same_then_else)]
    fn is_valid(&self) -> bool {
        if !self.get_code().is_valid() {
            return false;
        } else if self.size as usize > MAX_MSG_SIZE {
            return false;
        } else if self.get_version() != 0x1 {
            return false;
        } else if (self.flags & VhostUserHeaderFlag::RESERVED_BITS.bits()) != 0 {
            return false;
        }
        true
    }
}

impl<R: Req> Clone for VhostUserMsgHeader<R> {
    fn clone(&self) -> VhostUserMsgHeader<R> {
        *self
    }
}

impl<R: Req> VhostUserMsgHeader<R> {
    /// Create a new instance of `VhostUserMsgHeader`.
    pub fn new(request: R, flags: u32, size: u32) -> Self {
        // Default to protocol version 1
        let fl = (flags & VhostUserHeaderFlag::ALL_FLAGS.bits()) | 0x1;
        VhostUserMsgHeader {
            request: request.into(),
            flags: fl,
            size,
            _r: PhantomData,
        }
    }

    /// Get message type.
    pub fn get_code(&self) -> R {
        // It's safe because R is marked as repr(u32).
        unsafe { std::mem::transmute_copy::<u32, R>(&{ self.request }) }
    }

    /// Get message version number.
    pub fn get_version(&self) -> u32 {
        self.flags & 0x3
    }
}

impl<R: Req> Default for VhostUserMsgHeader<R> {
    fn default() -> Self {
        VhostUserMsgHeader {
            request: 0,
            flags: 0x1,
            size: 0,
            _r: PhantomData,
        }
    }
}

/// Unix domain socket endpoint for vhost-user connection.
pub(crate) struct Endpoint<R: Req> {
    sock: UnixStream,
    _r: PhantomData<R>,
}

impl<R: Req> Endpoint<R> {
    /// Create a new stream by connecting to server at `str`.
    ///
    /// # Return:
    /// * - the new Endpoint object on success.
    /// * - SocketConnect: failed to connect to peer.
    pub fn connect(path: &str) -> Result<Self> {
        let sock = UnixStream::connect(path).map_err(Error::SocketConnect)?;
        Ok(Self::from_stream(sock))
    }

    /// Create an endpoint from a stream object.
    pub fn from_stream(sock: UnixStream) -> Self {
        Endpoint {
            sock,
            _r: PhantomData,
        }
    }

    /// Sends bytes from scatter-gather vectors over the socket with optional attached file
    /// descriptors.
    ///
    /// # Return:
    /// * - number of bytes sent on success
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    pub fn send_iovec(&mut self, iovs: &[&[u8]], fds: Option<&[RawFd]>) -> Result<usize> {
        let rfds = match fds {
            Some(rfds) => rfds,
            _ => &[],
        };
        self.sock.send_with_fds(iovs, rfds).map_err(Into::into)
    }

    /// Sends all bytes from scatter-gather vectors over the socket with optional attached file
    /// descriptors. Will loop until all data has been transfered.
    ///
    /// # Return:
    /// * - number of bytes sent on success
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    pub fn send_iovec_all(&mut self, iovs: &[&[u8]], fds: Option<&[RawFd]>) -> Result<usize> {
        let mut data_sent = 0;
        let mut data_total = 0;
        let iov_lens: Vec<usize> = iovs.iter().map(|iov| iov.len()).collect();
        for len in &iov_lens {
            data_total += len;
        }

        while (data_total - data_sent) > 0 {
            let (nr_skip, offset) = get_sub_iovs_offset(&iov_lens, data_sent);
            let iov = &iovs[nr_skip][offset..];

            let data = &[&[iov], &iovs[(nr_skip + 1)..]].concat();
            let sfds = if data_sent == 0 { fds } else { None };

            let sent = self.send_iovec(data, sfds);
            match sent {
                Ok(0) => return Ok(data_sent),
                Ok(n) => data_sent += n,
                Err(e) => match e {
                    Error::SocketRetry(_) => {}
                    _ => return Err(e),
                },
            }
        }
        Ok(data_sent)
    }

    /// Sends a header-only message with optional attached file descriptors.
    ///
    /// # Return:
    /// * - number of bytes sent on success
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    /// * - PartialMessage: received a partial message.
    pub fn send_header(
        &mut self,
        hdr: &VhostUserMsgHeader<R>,
        fds: Option<&[RawFd]>,
    ) -> Result<()> {
        // Safe because there can't be other mutable referance to hdr.
        let iovs = unsafe {
            [slice::from_raw_parts(
                hdr as *const VhostUserMsgHeader<R> as *const u8,
                mem::size_of::<VhostUserMsgHeader<R>>(),
            )]
        };
        let bytes = self.send_iovec_all(&iovs[..], fds)?;
        if bytes != mem::size_of::<VhostUserMsgHeader<R>>() {
            return Err(Error::PartialMessage);
        }
        Ok(())
    }

    /// Send a message with header and body. Optional file descriptors may be attached to
    /// the message.
    ///
    /// # Return:
    /// * - number of bytes sent on success
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    /// * - PartialMessage: received a partial message.
    pub fn send_message<T: Sized>(
        &mut self,
        hdr: &VhostUserMsgHeader<R>,
        body: &T,
        fds: Option<&[RawFd]>,
    ) -> Result<()> {
        // Safe because there can't be other mutable referance to hdr and body.
        let iovs = unsafe {
            [
                slice::from_raw_parts(
                    hdr as *const VhostUserMsgHeader<R> as *const u8,
                    mem::size_of::<VhostUserMsgHeader<R>>(),
                ),
                slice::from_raw_parts(body as *const T as *const u8, mem::size_of::<T>()),
            ]
        };

        let bytes = self.send_iovec_all(&iovs[..], fds)?;
        if bytes != mem::size_of::<VhostUserMsgHeader<R>>() + mem::size_of::<T>() {
            return Err(Error::PartialMessage);
        }
        Ok(())
    }

    /// Reads bytes from the socket into the given scatter/gather vectors with optional attached
    /// file descriptors.
    ///
    /// The underlying communication channel is a Unix domain socket in STREAM mode. It's a little
    /// tricky to pass file descriptors through such a communication channel. Let's assume that a
    /// sender sending a message with some file descriptors attached. To successfully receive those
    /// attached file descriptors, the receiver must obey following rules:
    ///   1) file descriptors are attached to a message.
    ///   2) message(packet) boundaries must be respected on the receive side.
    /// In other words, recvmsg() operations must not cross the packet boundary, otherwise the
    /// attached file descriptors will get lost.
    ///
    /// # Return:
    /// * - (number of bytes received, [received fds]) on success
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    pub fn recv_into_iovec(&mut self, iovs: &mut [iovec]) -> Result<(usize, Option<Vec<RawFd>>)> {
        let mut fd_array = vec![0; MAX_ATTACHED_FD_ENTRIES];
        let (bytes, fds) = unsafe { self.sock.recv_with_fds(iovs, &mut fd_array)? };
        let rfds = match fds {
            0 => None,
            n => {
                let mut fds = Vec::with_capacity(n);
                fds.extend_from_slice(&fd_array[0..n]);
                Some(fds)
            }
        };

        Ok((bytes, rfds))
    }

    /// Reads all bytes from the socket into the given scatter/gather vectors with optional
    /// attached file descriptors. Will loop until all data has been transfered.
    ///
    /// The underlying communication channel is a Unix domain socket in STREAM mode. It's a little
    /// tricky to pass file descriptors through such a communication channel. Let's assume that a
    /// sender sending a message with some file descriptors attached. To successfully receive those
    /// attached file descriptors, the receiver must obey following rules:
    ///   1) file descriptors are attached to a message.
    ///   2) message(packet) boundaries must be respected on the receive side.
    /// In other words, recvmsg() operations must not cross the packet boundary, otherwise the
    /// attached file descriptors will get lost.
    ///
    /// # Return:
    /// * - (number of bytes received, [received fds]) on success
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    pub fn recv_into_iovec_all(
        &mut self,
        iovs: &mut [iovec],
    ) -> Result<(usize, Option<Vec<RawFd>>)> {
        let mut data_read = 0;
        let mut data_total = 0;
        let mut rfds = None;
        let iov_lens: Vec<usize> = iovs.iter().map(|iov| iov.iov_len).collect();
        for len in &iov_lens {
            data_total += len;
        }

        while (data_total - data_read) > 0 {
            let (nr_skip, offset) = get_sub_iovs_offset(&iov_lens, data_read);
            let iov = &mut iovs[nr_skip];

            let mut data = [
                &[iovec {
                    iov_base: (iov.iov_base as usize + offset) as *mut c_void,
                    iov_len: iov.iov_len - offset,
                }],
                &iovs[(nr_skip + 1)..],
            ]
            .concat();

            let res = self.recv_into_iovec(&mut data);
            match res {
                Ok((0, _)) => return Ok((data_read, rfds)),
                Ok((n, fds)) => {
                    if data_read == 0 {
                        rfds = fds;
                    }
                    data_read += n;
                }
                Err(e) => match e {
                    Error::SocketRetry(_) => {}
                    _ => return Err(e),
                },
            }
        }
        Ok((data_read, rfds))
    }

    /// Receive a header-only message with optional attached file descriptors.
    /// Note, only the first MAX_ATTACHED_FD_ENTRIES file descriptors will be
    /// accepted and all other file descriptor will be discard silently.
    ///
    /// # Return:
    /// * - (message header, [received fds]) on success.
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    /// * - PartialMessage: received a partial message.
    /// * - InvalidMessage: received a invalid message.
    pub fn recv_header(&mut self) -> Result<(VhostUserMsgHeader<R>, Option<Vec<RawFd>>)> {
        let mut hdr = VhostUserMsgHeader::default();
        let mut iovs = [iovec {
            iov_base: (&mut hdr as *mut VhostUserMsgHeader<R>) as *mut c_void,
            iov_len: mem::size_of::<VhostUserMsgHeader<R>>(),
        }];
        let (bytes, rfds) = self.recv_into_iovec_all(&mut iovs[..])?;

        if bytes != mem::size_of::<VhostUserMsgHeader<R>>() {
            return Err(Error::PartialMessage);
        } else if !hdr.is_valid() {
            return Err(Error::InvalidMessage);
        }

        Ok((hdr, rfds))
    }

    /// Receive a message with optional attached file descriptors.
    /// Note, only the first MAX_ATTACHED_FD_ENTRIES file descriptors will be
    /// accepted and all other file descriptor will be discard silently.
    ///
    /// # Return:
    /// * - (message header, message body, [received fds]) on success.
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    /// * - PartialMessage: received a partial message.
    /// * - InvalidMessage: received a invalid message.
    pub fn recv_body<T: Sized + Default + VhostUserMsgValidator>(
        &mut self,
    ) -> Result<(VhostUserMsgHeader<R>, T, Option<Vec<RawFd>>)> {
        let mut hdr = VhostUserMsgHeader::default();
        let mut body: T = Default::default();
        let mut iovs = [
            iovec {
                iov_base: (&mut hdr as *mut VhostUserMsgHeader<R>) as *mut c_void,
                iov_len: mem::size_of::<VhostUserMsgHeader<R>>(),
            },
            iovec {
                iov_base: (&mut body as *mut T) as *mut c_void,
                iov_len: mem::size_of::<T>(),
            },
        ];
        let (bytes, rfds) = self.recv_into_iovec_all(&mut iovs[..])?;

        let total = mem::size_of::<VhostUserMsgHeader<R>>() + mem::size_of::<T>();
        if bytes != total {
            return Err(Error::PartialMessage);
        } else if !hdr.is_valid() || !body.is_valid() {
            return Err(Error::InvalidMessage);
        }

        Ok((hdr, body, rfds))
    }

    /// Send a message with header, body and payload. Optional file descriptors
    /// may also be attached to the message.
    ///
    /// # Return:
    /// * - number of bytes sent on success
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    /// * - OversizedMsg: message size is too big.
    /// * - PartialMessage: received a partial message.
    /// * - IncorrectFds: wrong number of attached fds.
    pub fn send_message_with_payload<T: Sized, P: Sized>(
        &mut self,
        hdr: &VhostUserMsgHeader<R>,
        body: &T,
        payload: &[P],
        fds: Option<&[RawFd]>,
    ) -> Result<()> {
        let len = payload.len() * mem::size_of::<P>();
        if len > MAX_MSG_SIZE - mem::size_of::<T>() {
            return Err(Error::OversizedMsg);
        }
        if let Some(fd_arr) = fds {
            if fd_arr.len() > MAX_ATTACHED_FD_ENTRIES {
                return Err(Error::IncorrectFds);
            }
        }

        // Safe because there can't be other mutable reference to hdr, body and payload.
        let iovs = unsafe {
            [
                slice::from_raw_parts(
                    hdr as *const VhostUserMsgHeader<R> as *const u8,
                    mem::size_of::<VhostUserMsgHeader<R>>(),
                ),
                slice::from_raw_parts(body as *const T as *const u8, mem::size_of::<T>()),
                slice::from_raw_parts(payload.as_ptr() as *const u8, len),
            ]
        };
        let total = mem::size_of::<VhostUserMsgHeader<R>>() + mem::size_of::<T>() + len;
        let len = self.send_iovec_all(&iovs, fds)?;
        if len != total {
            return Err(Error::PartialMessage);
        }
        Ok(())
    }

    /// Receive a message with optional payload and attached file descriptors.
    /// Note, only the first MAX_ATTACHED_FD_ENTRIES file descriptors will be
    /// accepted and all other file descriptor will be discard silently.
    ///
    /// # Return:
    /// * - (message header, message body, size of payload, [received fds]) on success.
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    /// * - PartialMessage: received a partial message.
    /// * - InvalidMessage: received a invalid message.
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::type_complexity))]
    pub fn recv_payload_into_buf<T: Sized + Default + VhostUserMsgValidator>(
        &mut self,
        buf: &mut [u8],
    ) -> Result<(VhostUserMsgHeader<R>, T, usize, Option<Vec<RawFd>>)> {
        let mut hdr = VhostUserMsgHeader::default();
        let mut body: T = Default::default();
        let mut iovs = [
            iovec {
                iov_base: (&mut hdr as *mut VhostUserMsgHeader<R>) as *mut c_void,
                iov_len: mem::size_of::<VhostUserMsgHeader<R>>(),
            },
            iovec {
                iov_base: (&mut body as *mut T) as *mut c_void,
                iov_len: mem::size_of::<T>(),
            },
            iovec {
                iov_base: buf.as_mut_ptr() as *mut c_void,
                iov_len: buf.len(),
            },
        ];
        let (bytes, rfds) = self.recv_into_iovec_all(&mut iovs[..])?;

        let total = mem::size_of::<VhostUserMsgHeader<R>>() + mem::size_of::<T>();
        if bytes < total {
            return Err(Error::PartialMessage);
        } else if !hdr.is_valid() || !body.is_valid() {
            return Err(Error::InvalidMessage);
        }

        Ok((hdr, body, bytes - total, rfds))
    }
}

impl<T: Req> AsRawFd for Endpoint<T> {
    fn as_raw_fd(&self) -> RawFd {
        self.sock.as_raw_fd()
    }
}

// Negotiate process from slave.
pub(crate) fn negotiate_slave(
    slave: &mut Endpoint<MasterReq>,
    pfeatures: VhostUserProtocolFeatures,
    has_protocol_mq: bool,
    queue_num: u64,
) {
    // set owner
    let (hdr, rfds) = slave.recv_header().unwrap();
    assert_eq!(hdr.get_code(), MasterReq::SET_OWNER);
    assert!(rfds.is_none());

    // get features
    let vfeatures = 0x15 | VhostUserVirtioFeatures::PROTOCOL_FEATURES.bits();
    let hdr = VhostUserMsgHeader::new(MasterReq::GET_FEATURES, 0x4, 8);
    let msg = VhostUserU64::new(vfeatures);
    slave.send_message(&hdr, &msg, None).unwrap();
    let (hdr, _rfds) = slave.recv_header().unwrap();
    assert_eq!(hdr.get_code(), MasterReq::GET_FEATURES);

    // set features
    let (hdr, _msg, rfds) = slave.recv_body::<VhostUserU64>().unwrap();
    assert_eq!(hdr.get_code(), MasterReq::SET_FEATURES);
    assert!(rfds.is_none());

    // get vhost-user protocol features
    let code = MasterReq::GET_PROTOCOL_FEATURES;
    let (hdr, rfds) = slave.recv_header().unwrap();
    assert_eq!(hdr.get_code(), code);
    assert!(rfds.is_none());
    let hdr = VhostUserMsgHeader::new(code, 0x4, 8);
    let msg = VhostUserU64::new(pfeatures.bits());
    slave.send_message(&hdr, &msg, None).unwrap();

    // set vhost-user protocol features
    let (hdr, _msg, rfds) = slave.recv_body::<VhostUserU64>().unwrap();
    assert_eq!(hdr.get_code(), MasterReq::SET_PROTOCOL_FEATURES);
    assert!(rfds.is_none());

    // set number of queues
    if has_protocol_mq {
        let (hdr, rfds) = slave.recv_header().unwrap();
        assert_eq!(hdr.get_code(), MasterReq::GET_QUEUE_NUM);
        assert!(rfds.is_none());
        let hdr = VhostUserMsgHeader::new(MasterReq::GET_QUEUE_NUM, 0x4, 8);
        let msg = VhostUserU64::new(queue_num);
        slave.send_message(&hdr, &msg, None).unwrap();
    }

    //  set vring call
    for _i in 0..queue_num {
        let (hdr, _msg, rfds) = slave.recv_body::<VhostUserU64>().unwrap();
        assert_eq!(hdr.get_code(), MasterReq::SET_VRING_CALL);
        assert!(rfds.is_some());
    }

    // set mem table
    let mut region_buf: Vec<u8> = vec![0u8; mem::size_of::<VhostUserMemoryRegion>()];
    let (hdr, _msg, _payload, rfds) = slave
        .recv_payload_into_buf::<VhostUserMemory>(&mut region_buf)
        .unwrap();
    assert_eq!(hdr.get_code(), MasterReq::SET_MEM_TABLE);
    assert!(rfds.is_some());

    if pfeatures.contains(VhostUserProtocolFeatures::INFLIGHT_SHMFD) {
        // get inflight fd
        let (hdr, _msg, rfds) = slave.recv_body::<VhostUserInflight>().unwrap();
        assert_eq!(hdr.get_code(), MasterReq::GET_INFLIGHT_FD);
        assert!(rfds.is_none());
        let msg = VhostUserInflight {
            mmap_size: 0x100,
            mmap_offset: 0x0,
            ..Default::default()
        };
        let inflight_file = TempFile::new().unwrap().into_file();
        inflight_file.set_len(0x100).unwrap();
        let fds = [inflight_file.as_raw_fd()];
        let hdr = VhostUserMsgHeader::new(
            MasterReq::GET_INFLIGHT_FD,
            VhostUserHeaderFlag::REPLY.bits(),
            std::mem::size_of::<VhostUserInflight>() as u32,
        );
        slave.send_message(&hdr, &msg, Some(&fds)).unwrap();

        // set inflight fd
        let (hdr, _msg, rfds) = slave.recv_body::<VhostUserInflight>().unwrap();
        assert_eq!(hdr.get_code(), MasterReq::SET_INFLIGHT_FD);
        assert!(rfds.is_some());
        let hdr = VhostUserMsgHeader::new(
            MasterReq::GET_INFLIGHT_FD,
            VhostUserHeaderFlag::REPLY.bits(),
            std::mem::size_of::<VhostUserInflight>() as u32,
        );
        slave.send_header(&hdr, None).unwrap();
    } else {
        slave.send_header(&hdr, None).unwrap();
    }

    // set vring num
    for _i in 0..queue_num {
        let (hdr, _msg, rfds) = slave.recv_body::<VhostUserVringState>().unwrap();
        slave.send_header(&hdr, None).unwrap();
        assert!(rfds.is_none());
    }

    // set vring base
    for _i in 0..queue_num {
        let (hdr, _msg, rfds) = slave.recv_body::<VhostUserVringState>().unwrap();
        assert_eq!(hdr.get_code(), MasterReq::SET_VRING_BASE);
        assert!(rfds.is_none());
        slave.send_header(&hdr, None).unwrap();
    }

    // set vring addr
    for _i in 0..queue_num {
        let (hdr, _msg, rfds) = slave.recv_body::<VhostUserVringAddr>().unwrap();
        assert_eq!(hdr.get_code(), MasterReq::SET_VRING_ADDR);
        assert!(rfds.is_none());
        slave.send_header(&hdr, None).unwrap();
    }

    // set vring kick
    for _i in 0..queue_num {
        let (hdr, _msg, rfds) = slave.recv_body::<VhostUserU64>().unwrap();
        assert_eq!(hdr.get_code(), MasterReq::SET_VRING_KICK);
        assert!(rfds.is_some());
    }

    // set vring call
    for _i in 0..queue_num {
        let (hdr, _msg, rfds) = slave.recv_body::<VhostUserU64>().unwrap();
        assert_eq!(hdr.get_code(), MasterReq::SET_VRING_CALL);
        assert!(rfds.is_some());
    }

    // set vring enable
    for _i in 0..queue_num {
        let (hdr, _msg, rfds) = slave.recv_body::<VhostUserVringState>().unwrap();
        assert_eq!(hdr.get_code(), MasterReq::SET_VRING_ENABLE);
        assert!(rfds.is_none());
        slave.send_header(&hdr, None).unwrap();
    }
}
