// Copyright 2022 Alibaba Cloud. All Rights Reserved.
//
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

/// `VsockMuxer` is the device-facing component of multiple vsock backends. You
/// can add various of backends to VsockMuxer which implements the
/// `VsockBackend` trait. VsockMuxer can abstracts away the gory details of
/// translating between AF_VSOCK and the protocol of backends which you added.
/// It can also presents a clean interface to the rest of the vsock device
/// model.
///
/// The vsock muxer has two main roles:
/// 1. Vsock connection multiplexer: It's the muxer's job to create, manage, and
///    terminate `VsockConnection` objects. The muxer also routes packets to
///    their owning connections. It does so via a connection `HashMap`, keyed by
///    what is basically a (host_port, guest_port) tuple. Vsock packet traffic
///    needs to be inspected, in order to detect connection request packets
///    (leading to the creation of a new connection), and connection reset
///    packets (leading to the termination of an existing connection). All other
///    packets, though, must belong to an existing connection and, as such, the
///    muxer simply forwards them.
/// 2. Event dispatcher There are three event categories that the vsock backend
///    is interested it:
///    1. A new host-initiated connection is ready to be accepted from the
///       backends added to muxer;
///    2. Data is available for reading from a newly-accepted host-initiated
///       connection (i.e. the host is ready to issue a vsock connection
///       request, informing us of the destination port to which it wants to
///       connect);
///    3. Some event was triggered for a connected backend connection, that
///       belongs to a `VsockConnection`. The muxer gets notified about all of
///       these events, because, as a `VsockEpollListener` implementor, it gets
///       to register a nested epoll FD into the main VMM epolling loop. All
///       other pollable FDs are then registered under this nested epoll FD. To
///       route all these events to their handlers, the muxer uses another
///       `HashMap` object, mapping `RawFd`s to `EpollListener`s.
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::os::fd::FromRawFd;
use std::os::unix::io::{AsRawFd, RawFd};

use log::{debug, error, info, trace, warn};

use super::super::backend::{HybridStream, VsockBackend, VsockBackendType, VsockStream};

use super::super::csm::{ConnState, VsockConnection};
use super::super::defs::uapi;
use super::super::packet::VsockPacket;
use super::super::{Result as VsockResult, VsockChannel, VsockEpollListener, VsockError};
use super::muxer_killq::MuxerKillQ;
use super::muxer_rxq::MuxerRxQ;
use super::{defs, Error, Result, VsockGenericMuxer};

/// A unique identifier of a `VsockConnection` object. Connections are stored in
/// a hash map, keyed by a `ConnMapKey` object.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ConnMapKey {
    local_port: u32,
    pub(crate) peer_port: u32,
}

/// A muxer RX queue item.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MuxerRx {
    /// The packet must be fetched from the connection identified by
    /// `ConnMapKey`.
    ConnRx(ConnMapKey),
    /// The muxer must produce an RST packet.
    RstPkt { local_port: u32, peer_port: u32 },
}

enum ReadPortResult {
    PassFd,
    Connect(u32),
}

/// An epoll listener, registered under the muxer's nested epoll FD.
pub enum EpollListener {
    /// The listener is a `VsockConnection`, identified by `key`, and interested
    /// in the events in `evset`. Since `VsockConnection` implements
    /// `VsockEpollListener`, notifications will be forwarded to the listener
    /// via `VsockEpollListener::notify()`.
    Connection {
        key: ConnMapKey,
        evset: epoll::Events,
        backend: VsockBackendType,
    },
    /// A listener interested in new host-initiated connections.
    Backend(VsockBackendType),
    /// A listener interested in reading host "connect <port>" commands from a
    /// freshly connected host socket.
    LocalStream(Box<dyn VsockStream>),
    /// A listener interested in recvmsg from host to get the <port> and a
    /// socket/pipe fd.
    PassFdStream(Box<dyn VsockStream>),
}

/// The vsock connection multiplexer.
pub struct VsockMuxer {
    /// Guest CID.
    cid: u64,
    /// A hash map used to store the active connections.
    conn_map: HashMap<ConnMapKey, VsockConnection>,
    /// A hash map used to store epoll event listeners / handlers.
    listener_map: HashMap<RawFd, EpollListener>,
    /// The RX queue. Items in this queue are consumed by
    /// `VsockMuxer::recv_pkt()`, and produced
    /// - by `VsockMuxer::send_pkt()` (e.g. RST in response to a connection
    ///   request packet); and
    /// - in response to EPOLLIN events (e.g. data available to be read from an
    ///   AF_UNIX socket).
    rxq: MuxerRxQ,
    /// A queue used for terminating connections that are taking too long to
    /// shut down.
    killq: MuxerKillQ,
    /// The nested epoll FD, used to register epoll listeners.
    epoll_fd: RawFd,
    /// A hash set used to keep track of used host-side (local) ports, in order
    /// to assign local ports to host-initiated connections.
    local_port_set: HashSet<u32>,
    /// The last used host-side port.
    local_port_last: u32,
    /// backend implementations supported in muxer.
    backend_map: HashMap<VsockBackendType, Box<dyn VsockBackend>>,
    /// the backend which can accept peer-initiated connection.
    peer_backend: Option<VsockBackendType>,
}

impl VsockChannel for VsockMuxer {
    /// Deliver a vsock packet to the guest vsock driver.
    ///
    /// Retuns:
    /// - `Ok(())`: `pkt` has been successfully filled in; or
    /// - `Err(VsockError::NoData)`: there was no available data with which to fill in the packet.
    fn recv_pkt(&mut self, pkt: &mut VsockPacket) -> VsockResult<()> {
        // We'll look for instructions on how to build the RX packet in the RX
        // queue. If the queue is empty, that doesn't necessarily mean we don't
        // have any pending RX, since the queue might be out-of-sync. If that's
        // the case, we'll attempt to sync it first, and then try to pop
        // something out again.
        if self.rxq.is_empty() && !self.rxq.is_synced() {
            self.rxq = MuxerRxQ::from_conn_map(&self.conn_map);
        }

        while let Some(rx) = self.rxq.peek() {
            let res = match rx {
                // We need to build an RST packet, going from `local_port` to
                // `peer_port`.
                MuxerRx::RstPkt {
                    local_port,
                    peer_port,
                } => {
                    pkt.set_op(uapi::VSOCK_OP_RST)
                        .set_src_cid(uapi::VSOCK_HOST_CID)
                        .set_dst_cid(self.cid)
                        .set_src_port(local_port)
                        .set_dst_port(peer_port)
                        .set_len(0)
                        .set_type(uapi::VSOCK_TYPE_STREAM)
                        .set_flags(0)
                        .set_buf_alloc(0)
                        .set_fwd_cnt(0);
                    self.rxq.pop().unwrap();
                    trace!(
                        "vsock: muxer.recv[rxq.len={}, type={}, op={}, sp={}, sc={}, dp={}, dc={}]: {:?}",
                        self.rxq.len(),
                        pkt.type_(),
                        pkt.op(),
                        pkt.src_port(),
                        pkt.src_cid(),
                        pkt.dst_port(),
                        pkt.dst_cid(),
                        pkt.hdr()
                    );
                    return Ok(());
                }

                // We'll defer building the packet to this connection, that has
                // something to say.
                MuxerRx::ConnRx(key) => {
                    let mut conn_res = Err(VsockError::NoData);
                    let mut do_pop = true;
                    self.apply_conn_mutation(key, |conn| {
                        conn_res = conn.recv_pkt(pkt);
                        do_pop = !conn.has_pending_rx();
                    });
                    if do_pop {
                        self.rxq.pop().unwrap();
                    }
                    conn_res
                }
            };

            if res.is_ok() {
                // Inspect traffic, looking for RST packets, since that means we
                // have to terminate and remove this connection from the active
                // connection pool.
                if pkt.op() == uapi::VSOCK_OP_RST {
                    self.remove_connection(ConnMapKey {
                        local_port: pkt.src_port(),
                        peer_port: pkt.dst_port(),
                    });
                }

                trace!(
                    "vsock: muxer.recv[rxq.len={}, type={}, op={}, sp={}, sc={}, dp={}, dc={}]: {:?}",
                    self.rxq.len(),
                    pkt.type_(),
                    pkt.op(),
                    pkt.src_port(),
                    pkt.src_cid(),
                    pkt.dst_port(),
                    pkt.dst_cid(),
                    pkt.hdr()
                );
                return Ok(());
            }
        }

        Err(VsockError::NoData)
    }

    /// Deliver a guest-generated packet to its destination in the vsock
    /// backend.
    ///
    /// This absorbs unexpected packets, handles RSTs (by dropping connections),
    /// and forwards all the rest to their owning `VsockConnection`.
    ///
    /// Returns: always `Ok(())` - the packet has been consumed, and its virtio
    /// TX buffers can be returned to the guest vsock driver.
    fn send_pkt(&mut self, pkt: &VsockPacket) -> VsockResult<()> {
        let conn_key = ConnMapKey {
            local_port: pkt.dst_port(),
            peer_port: pkt.src_port(),
        };

        trace!(
            "vsock: muxer.send[rxq.len={}, type={}, op={}, sp={}, sc={}, dp={}, dc={}]: {:?}",
            self.rxq.len(),
            pkt.type_(),
            pkt.op(),
            pkt.src_port(),
            pkt.src_cid(),
            pkt.dst_port(),
            pkt.dst_cid(),
            pkt.hdr()
        );

        // If this packet has an unsupported type (!=stream), we must send back
        // an RST.
        if pkt.type_() != uapi::VSOCK_TYPE_STREAM {
            self.enq_rst(pkt.dst_port(), pkt.src_port());
            return Ok(());
        }

        // We don't know how to handle packets addressed to other CIDs. We only
        // handle the host part of the guest - host communication here.
        if pkt.dst_cid() != uapi::VSOCK_HOST_CID {
            info!(
                "vsock: dropping guest packet for unknown CID: {:?}",
                pkt.hdr()
            );
            return Ok(());
        }

        if !self.conn_map.contains_key(&conn_key) {
            // This packet can't be routed to any active connection (based on
            // its src and dst ports). The only orphan / unroutable packets we
            // know how to handle are connection requests.
            if pkt.op() == uapi::VSOCK_OP_REQUEST {
                // Oh, this is a connection request!
                self.handle_peer_request_pkt(pkt);
            } else {
                // Send back an RST, to let the drive know we weren't expecting
                // this packet.
                self.enq_rst(pkt.dst_port(), pkt.src_port());
            }
            return Ok(());
        }

        // Right, we know where to send this packet, then (to `conn_key`).
        // However, if this is an RST, we have to forcefully terminate the
        // connection, so there's no point in forwarding it the packet.
        if pkt.op() == uapi::VSOCK_OP_RST {
            self.remove_connection(conn_key);
            return Ok(());
        }

        // Alright, everything looks in order - forward this packet to its
        // owning connection.
        let mut res: VsockResult<()> = Ok(());

        // For the hybrid connection, if it want to keep the connection
        // when the pipe peer closed, here it needs to update the epoll
        // listner to catch the events.
        let mut listener = None;
        let conn = self.conn_map.get_mut(&conn_key).unwrap();
        let pre_state = conn.state();
        let nfd: RawFd = conn.as_raw_fd();

        if pre_state == ConnState::LocalClosed && conn.keep() {
            conn.state = ConnState::Established;
            listener = Some(EpollListener::Connection {
                key: conn_key,
                evset: conn.get_polled_evset(),
                backend: conn.stream.backend_type(),
            });
        }

        if let Some(nlistener) = listener {
            self.add_listener(nfd, nlistener).unwrap_or_else(|err| {
                self.kill_connection(conn_key);
                warn!(
                    "vsock: error updating epoll listener for (lp={}, pp={}): {:?}",
                    conn_key.local_port, conn_key.peer_port, err
                );
            });
        }

        self.apply_conn_mutation(conn_key, |conn| {
            res = conn.send_pkt(pkt);
        });

        res
    }

    /// Check if the muxer has any pending RX data, with which to fill a
    /// guest-provided RX buffer.
    fn has_pending_rx(&self) -> bool {
        !self.rxq.is_empty() || !self.rxq.is_synced()
    }
}

impl AsRawFd for VsockMuxer {
    /// Get the FD to be registered for polling upstream (in the main VMM epoll
    /// loop, in this case).
    ///
    /// This will be the muxer's nested epoll FD.
    fn as_raw_fd(&self) -> RawFd {
        self.epoll_fd
    }
}

impl VsockEpollListener for VsockMuxer {
    /// Get the epoll events to be polled upstream.
    ///
    /// Since the polled FD is a nested epoll FD, we're only interested in
    /// EPOLLIN events (i.e. some event occurred on one of the FDs registered
    /// under our epoll FD).
    fn get_polled_evset(&self) -> epoll::Events {
        epoll::Events::EPOLLIN
    }

    /// Notify the muxer about a pending event having occurred under its nested
    /// epoll FD.
    fn notify(&mut self, _: epoll::Events) {
        trace!("vsock: muxer received kick");

        let mut epoll_events = vec![epoll::Event::new(epoll::Events::empty(), 0); 32];
        match epoll::wait(self.epoll_fd, 0, epoll_events.as_mut_slice()) {
            Ok(ev_cnt) => {
                for ev in &epoll_events[0..ev_cnt] {
                    self.handle_event(
                        ev.data as RawFd,
                        epoll::Events::from_bits(ev.events).unwrap(),
                    );
                }
            }
            Err(e) => {
                warn!("vsock: failed to consume muxer epoll event: {}", e);
            }
        }
    }
}

impl VsockGenericMuxer for VsockMuxer {
    /// add a backend for Muxer.
    fn add_backend(&mut self, backend: Box<dyn VsockBackend>, is_peer_backend: bool) -> Result<()> {
        let backend_type = backend.r#type();
        if self.backend_map.contains_key(&backend_type) {
            return Err(Error::BackendRegistered(backend_type));
        }
        self.add_listener(
            backend.as_raw_fd(),
            EpollListener::Backend(backend_type.clone()),
        )?;
        self.backend_map.insert(backend_type.clone(), backend);
        if is_peer_backend {
            self.peer_backend = Some(backend_type);
        }
        Ok(())
    }
}

impl VsockMuxer {
    /// Muxer constructor.
    pub fn new(cid: u64) -> Result<Self> {
        Ok(Self {
            cid,
            epoll_fd: epoll::create(false).map_err(Error::EpollFdCreate)?,
            rxq: MuxerRxQ::default(),
            conn_map: HashMap::with_capacity(defs::MAX_CONNECTIONS),
            listener_map: HashMap::with_capacity(defs::MAX_CONNECTIONS + 1),
            killq: MuxerKillQ::default(),
            local_port_last: (1u32 << 30) - 1,
            local_port_set: HashSet::with_capacity(defs::MAX_CONNECTIONS),
            backend_map: HashMap::new(),
            peer_backend: None,
        })
    }

    /// Handle/dispatch an epoll event to its listener.
    fn handle_event(&mut self, fd: RawFd, event_set: epoll::Events) {
        trace!(
            "vsock: muxer processing event: fd={}, evset={:?}",
            fd,
            event_set
        );

        match self.listener_map.get_mut(&fd) {
            // This event needs to be forwarded to a `VsockConnection` that is
            // listening for it.
            Some(EpollListener::Connection { key, evset: _, .. }) => {
                let key_copy = *key;

                // If the hybrid connection's local peer closed, then the epoll handler wouldn't
                // get the epollout event even when it's reopened again, thus it should be notified
                // when the guest send any data to try to active the epoll handler to generate the
                // epollout event for this connection.

                let mut need_rm = false;
                if let Some(conn) = self.conn_map.get_mut(&key_copy) {
                    if event_set.contains(epoll::Events::EPOLLERR) && conn.keep() {
                        conn.state = ConnState::LocalClosed;
                        need_rm = true;
                    }
                }
                if need_rm {
                    self.remove_listener(fd);
                }

                // The handling of this event will most probably mutate the
                // state of the receiving connection. We'll need to check for new
                // pending RX, event set mutation, and all that, so we're
                // wrapping the event delivery inside those checks.
                self.apply_conn_mutation(key_copy, |conn| {
                    conn.notify(event_set);
                });
            }

            // A new host-initiated connection is ready to be accepted.
            Some(EpollListener::Backend(backend_type)) => {
                if let Some(backend) = self.backend_map.get_mut(backend_type) {
                    if self.rxq.len() == defs::MAX_CONNECTIONS {
                        // If we're already maxed-out on connections, we'll just
                        // accept and immediately discard this potentially new
                        // one.
                        warn!("vsock: connection limit reached; refusing new host connection");
                        backend.accept().map(|_| 0).unwrap_or(0);
                        return;
                    }
                    backend
                        .accept()
                        .map_err(Error::BackendAccept)
                        .and_then(|stream| {
                            // Before forwarding this connection to a listening
                            // AF_VSOCK socket on the guest side, we need to
                            // know the destination port. We'll read that port
                            // from a "connect" command received on this socket,
                            // so the next step is to ask to be notified the
                            // moment we can read from it.

                            self.add_listener(
                                stream.as_raw_fd(),
                                EpollListener::LocalStream(stream),
                            )
                        })
                        .unwrap_or_else(|err| {
                            warn!("vsock: unable to accept local connection: {:?}", err);
                        });
                } else {
                    error!("vsock: unsable to find specific backend {:?}", backend_type)
                }
            }

            // Data is ready to be read from a host-initiated connection. That
            // would be the "connect" command that we're expecting.
            Some(EpollListener::LocalStream(_)) => {
                if let Some(EpollListener::LocalStream(mut stream)) = self.remove_listener(fd) {
                    Self::read_local_stream_port(&mut stream)
                        .and_then(|read_port_result| match read_port_result {
                            ReadPortResult::Connect(peer_port) => {
                                let local_port = self.allocate_local_port();
                                self.add_connection(
                                    ConnMapKey {
                                        local_port,
                                        peer_port,
                                    },
                                    VsockConnection::new_local_init(
                                        stream,
                                        uapi::VSOCK_HOST_CID,
                                        self.cid,
                                        local_port,
                                        peer_port,
                                        false,
                                    ),
                                )
                            }
                            ReadPortResult::PassFd => self.add_listener(
                                stream.as_raw_fd(),
                                EpollListener::PassFdStream(stream),
                            ),
                        })
                        .unwrap_or_else(|err| {
                            info!("vsock: error adding local-init connection: {:?}", err);
                        })
                }
            }

            Some(EpollListener::PassFdStream(_)) => {
                if let Some(EpollListener::PassFdStream(mut stream)) = self.remove_listener(fd) {
                    Self::passfd_read_port_and_fd(&mut stream)
                        .map(|(nfd, peer_port, keep)| {
                            (nfd, self.allocate_local_port(), peer_port, keep)
                        })
                        .and_then(|(nfd, local_port, peer_port, keep)| {
                            // Here we should make sure the nfd the sole owner to convert it
                            // into an UnixStream object, otherwise, it could cause memory unsafety.
                            let nstream = unsafe { File::from_raw_fd(nfd) };

                            let mut hybridstream = HybridStream {
                                hybrid_stream: nstream,
                                slave_stream: Some(stream),
                            };

                            hybridstream
                                .set_nonblocking(true)
                                .map_err(Error::BackendSetNonBlock)?;

                            self.add_connection(
                                ConnMapKey {
                                    local_port,
                                    peer_port,
                                },
                                VsockConnection::new_local_init(
                                    Box::new(hybridstream),
                                    uapi::VSOCK_HOST_CID,
                                    self.cid,
                                    local_port,
                                    peer_port,
                                    keep,
                                ),
                            )
                        })
                        .unwrap_or_else(|err| {
                            info!(
                                "vsock: error adding local-init passthrough fd connection: {:?}",
                                err
                            );
                        })
                }
            }

            _ => {
                info!(
                    "vsock: unexpected event: fd={:?}, evset={:?}",
                    fd, event_set
                );
            }
        }
    }

    /// Parse a host "connect" command, and extract the destination vsock port.
    fn read_local_stream_port(stream: &mut Box<dyn VsockStream>) -> Result<ReadPortResult> {
        let mut buf = [0u8; 32];

        // This is the minimum number of bytes that we should be able to read,
        // when parsing a valid connection request. I.e. `b"passfd\n"`, otherwise,
        // it would be `b"connect 0\n".len()`.
        const MIN_READ_LEN: usize = 7;

        // Bring in the minimum number of bytes that we should be able to read.
        stream
            .read(&mut buf[..MIN_READ_LEN])
            .map_err(Error::BackendRead)?;

        // Now, finish reading the destination port number if it's connect <port> command,
        // by bringing in one byte at a time, until we reach an EOL terminator (or our buffer
        // space runs out).  Yeah, not particularly proud of this approach, but it will have to
        // do for now.
        let mut blen = MIN_READ_LEN;
        while buf[blen - 1] != b'\n' && blen < buf.len() {
            stream
                .read_exact(&mut buf[blen..=blen])
                .map_err(Error::BackendRead)?;
            blen += 1;
        }

        let mut word_iter = std::str::from_utf8(&buf)
            .map_err(|_| Error::InvalidPortRequest)?
            .split_whitespace();

        word_iter
            .next()
            .ok_or(Error::InvalidPortRequest)
            .and_then(|word| {
                let key = word.to_lowercase();
                if key == "connect" {
                    Ok(true)
                } else if key == "passfd" {
                    Ok(false)
                } else {
                    Err(Error::InvalidPortRequest)
                }
            })
            .and_then(|connect| {
                if connect {
                    word_iter.next().ok_or(Error::InvalidPortRequest).map(Some)
                } else {
                    Ok(None)
                }
            })
            .and_then(|word| {
                word.map_or_else(
                    || Ok(ReadPortResult::PassFd),
                    |word| {
                        word.parse::<u32>()
                            .map_or(Err(Error::InvalidPortRequest), |word| {
                                Ok(ReadPortResult::Connect(word))
                            })
                    },
                )
            })
            .map_err(|_| Error::InvalidPortRequest)
    }

    fn passfd_read_port_and_fd(stream: &mut Box<dyn VsockStream>) -> Result<(RawFd, u32, bool)> {
        let mut buf = [0u8; 32];
        let mut fds = [0, 1];
        let (data_len, fd_len) = stream
            .recv_data_fd(&mut buf, &mut fds)
            .map_err(Error::BackendRead)?;

        if fd_len != 1 || fds[0] <= 0 {
            return Err(Error::InvalidPortRequest);
        }

        let mut port_iter = std::str::from_utf8(&buf[..data_len])
            .map_err(|_| Error::InvalidPortRequest)?
            .split_whitespace();

        let port = port_iter
            .next()
            .ok_or(Error::InvalidPortRequest)
            .and_then(|word| word.parse::<u32>().map_err(|_| Error::InvalidPortRequest))?;

        let keep = port_iter.next().is_some_and(|kp| kp == "keep");

        Ok((fds[0], port, keep))
    }

    /// Add a new connection to the active connection pool.
    fn add_connection(&mut self, key: ConnMapKey, conn: VsockConnection) -> Result<()> {
        // We might need to make room for this new connection, so let's sweep
        // the kill queue first.  It's fine to do this here because:
        // - unless the kill queue is out of sync, this is a pretty inexpensive
        //   operation; and
        // - we are under no pressure to respect any accurate timing for
        //   connection termination.
        self.sweep_killq();

        if self.conn_map.len() >= defs::MAX_CONNECTIONS {
            info!(
                "vsock: muxer connection limit reached ({})",
                defs::MAX_CONNECTIONS
            );
            return Err(Error::TooManyConnections);
        }

        self.add_listener(
            conn.as_raw_fd(),
            EpollListener::Connection {
                key,
                evset: conn.get_polled_evset(),
                backend: conn.stream.backend_type(),
            },
        )
        .map(|_| {
            if conn.has_pending_rx() {
                // We can safely ignore any error in adding a connection RX
                // indication. Worst case scenario, the RX queue will get
                // desynchronized, but we'll handle that the next time we need
                // to yield an RX packet.
                self.rxq.push(MuxerRx::ConnRx(key));
            }
            self.conn_map.insert(key, conn);
        })
    }

    /// Remove a connection from the active connection poll.
    fn remove_connection(&mut self, key: ConnMapKey) {
        if let Some(conn) = self.conn_map.remove(&key) {
            self.remove_listener(conn.as_raw_fd());
        }
        self.free_local_port(key.local_port);
    }

    /// Schedule a connection for immediate termination. I.e. as soon as we can
    /// also let our peer know we're dropping the connection, by sending it an
    /// RST packet.
    fn kill_connection(&mut self, key: ConnMapKey) {
        let mut had_rx = false;

        self.conn_map.entry(key).and_modify(|conn| {
            had_rx = conn.has_pending_rx();
            conn.kill();
        });
        // This connection will now have an RST packet to yield, so we need to
        // add it to the RX queue. However, there's no point in doing that if it
        // was already in the queue.
        if !had_rx {
            // We can safely ignore any error in adding a connection RX
            // indication. Worst case scenario, the RX queue will get
            // desynchronized, but we'll handle that the next time we need to
            // yield an RX packet.
            self.rxq.push(MuxerRx::ConnRx(key));
        }
    }

    /// Register a new epoll listener under the muxer's nested epoll FD.
    pub(crate) fn add_listener(&mut self, fd: RawFd, listener: EpollListener) -> Result<()> {
        let evset = match listener {
            EpollListener::Connection { evset, .. } => evset,
            EpollListener::LocalStream(_) => epoll::Events::EPOLLIN,
            EpollListener::Backend(_) => epoll::Events::EPOLLIN,
            EpollListener::PassFdStream(_) => epoll::Events::EPOLLIN,
        };

        epoll::ctl(
            self.epoll_fd,
            epoll::ControlOptions::EPOLL_CTL_ADD,
            fd,
            epoll::Event::new(evset, fd as u64),
        )
        .map(|_| {
            self.listener_map.insert(fd, listener);
        })
        .map_err(Error::EpollAdd)?;

        Ok(())
    }

    /// Remove (and return) a previously registered epoll listener.
    fn remove_listener(&mut self, fd: RawFd) -> Option<EpollListener> {
        let maybe_listener = self.listener_map.remove(&fd);

        if maybe_listener.is_some() {
            epoll::ctl(
                self.epoll_fd,
                epoll::ControlOptions::EPOLL_CTL_DEL,
                fd,
                epoll::Event::new(epoll::Events::empty(), 0),
            )
            .unwrap_or_else(|err| {
                warn!(
                    "vosck muxer: error removing epoll listener for fd {:?}: {:?}",
                    fd, err
                );
            });
        }

        maybe_listener
    }

    /// Allocate a host-side port to be assigned to a new host-initiated
    /// connection.
    fn allocate_local_port(&mut self) -> u32 {
        // TODO: this doesn't seem very space-efficient.
        // Mybe rewrite this to limit port range and use a bitmap?

        loop {
            self.local_port_last = (self.local_port_last + 1) & !(1 << 31) | (1 << 30);
            if self.local_port_set.insert(self.local_port_last) {
                break;
            }
        }
        self.local_port_last
    }

    /// Mark a previously used host-side port as free.
    fn free_local_port(&mut self, port: u32) {
        self.local_port_set.remove(&port);
    }

    /// Handle a new connection request comming from our peer (the guest vsock
    /// driver).
    ///
    /// This will attempt to connect to a host-side backend. If successful, a
    ///  new connection object will be created and added to the connection pool.
    ///  On failure, a new RST packet will be scheduled for delivery to the
    ///  guest.
    fn handle_peer_request_pkt(&mut self, pkt: &VsockPacket) {
        if self.peer_backend.is_none() {
            error!("no usable backend for peer request");
            self.enq_rst(pkt.dst_port(), pkt.src_port());
            return;
        }

        // safe to unwrap
        if let Some(backend) = self.backend_map.get(self.peer_backend.as_ref().unwrap()) {
            backend
                .connect(pkt.dst_port())
                .map_err(Error::BackendConnect)
                .and_then(|stream| {
                    self.add_connection(
                        ConnMapKey {
                            local_port: pkt.dst_port(),
                            peer_port: pkt.src_port(),
                        },
                        VsockConnection::new_peer_init(
                            stream,
                            uapi::VSOCK_HOST_CID,
                            self.cid,
                            pkt.dst_port(),
                            pkt.src_port(),
                            pkt.buf_alloc(),
                            false,
                        ),
                    )
                })
                .unwrap_or_else(|e| {
                    error!("peer request error: {:?}", e);
                    self.enq_rst(pkt.dst_port(), pkt.src_port());
                });
        } else {
            error!("no usable backend selected for peer request");
            self.enq_rst(pkt.dst_port(), pkt.src_port());
        }
    }

    /// Perform an action that might mutate a connection's state.
    ///
    /// This is used as shorthand for repetitive tasks that need to be performed
    /// after a connection object mutates. E.g.
    /// - update the connection's epoll listener;
    /// - schedule the connection to be queried for RX data;
    /// - kill the connection if an unrecoverable error occurs.
    fn apply_conn_mutation<F>(&mut self, key: ConnMapKey, mut_fn: F)
    where
        F: FnOnce(&mut VsockConnection),
    {
        if let Some(conn) = self.conn_map.get_mut(&key) {
            let had_rx = conn.has_pending_rx();
            let was_expiring = conn.will_expire();
            let prev_state = conn.state();
            let backend_type = conn.stream.backend_type();

            mut_fn(conn);

            // If this is a host-initiated connection that has just become
            // established, we'll have to send an ack message to the host end.
            if prev_state == ConnState::LocalInit && conn.state() == ConnState::Established {
                let msg = format!("OK {}\n", key.local_port);
                match conn.send_bytes_raw(msg.as_bytes()) {
                    Ok(written) if written == msg.len() => (),
                    Ok(_) => {
                        // If we can't write a dozen bytes to a pristine
                        // connection something must be really wrong. Killing
                        // it.
                        conn.kill();
                        warn!("vsock: unable to fully write connection ack msg.");
                    }
                    Err(err) => {
                        conn.kill();
                        warn!("vsock: unable to ack host connection [local_cid {}, peer_cid {}, local_port {}, peer_port {}]: {:?}", conn.local_cid, conn.peer_cid, conn.local_port, conn.peer_port, err);
                    }
                };
            }

            // If the connection wasn't previously scheduled for RX, add it to
            // our RX queue.
            if !had_rx && conn.has_pending_rx() {
                self.rxq.push(MuxerRx::ConnRx(key));
            }

            // If the connection wasn't previously scheduled for termination,
            // add it to the kill queue.
            if !was_expiring && conn.will_expire() {
                // It's safe to unwrap here, since `conn.will_expire()` already
                // guaranteed that an `conn.expiry` is available.
                self.killq.push(key, conn.expiry().unwrap());
            }

            let fd = conn.as_raw_fd();
            let new_evset = conn.get_polled_evset();
            if new_evset.is_empty() {
                // If the connection no longer needs epoll notifications, remove
                // its listener from our list.
                self.remove_listener(fd);
                return;
            }
            if let Some(EpollListener::Connection { evset, .. }) = self.listener_map.get_mut(&fd) {
                if *evset != new_evset {
                    // If the set of events that the connection is interested in
                    // has changed, we need to update its epoll listener.
                    debug!(
                        "vsock: updating listener for (lp={}, pp={}): old={:?}, new={:?}",
                        key.local_port, key.peer_port, *evset, new_evset
                    );

                    *evset = new_evset;
                    epoll::ctl(
                        self.epoll_fd,
                        epoll::ControlOptions::EPOLL_CTL_MOD,
                        fd,
                        epoll::Event::new(new_evset, fd as u64),
                    )
                    .unwrap_or_else(|err| {
                        // This really shouldn't happen, like, ever. However,
                        // "famous last words" and all that, so let's just kill
                        // it with fire, and walk away.
                        self.kill_connection(key);
                        warn!(
                            "vsock: error updating epoll listener for (lp={}, pp={}): {:?}",
                            key.local_port, key.peer_port, err
                        );
                    });
                }
            } else if conn.state() != ConnState::LocalClosed {
                // The connection had previously asked to be removed from the
                // listener map (by returning an empty event set via
                // `get_polled_fd()`), but now wants back in.
                self.add_listener(
                    fd,
                    EpollListener::Connection {
                        key,
                        evset: new_evset,
                        backend: backend_type,
                    },
                )
                .unwrap_or_else(|err| {
                    self.kill_connection(key);
                    warn!(
                        "vsock: error updating epoll listener for (lp={}, pp={}): {:?}",
                        key.local_port, key.peer_port, err
                    );
                });
            }
        }
    }

    /// Check if any connections have timed out, and if so, schedule them for
    /// immediate termination.
    fn sweep_killq(&mut self) {
        while let Some(key) = self.killq.pop() {
            // Connections don't get removed from the kill queue when their kill
            // timer is disarmed, since that would be a costly operation. This
            // means we must check if the connection has indeed expired, prior
            // to killing it.
            let mut kill = false;
            self.conn_map
                .entry(key)
                .and_modify(|conn| kill = conn.has_expired());
            if kill {
                self.kill_connection(key);
            }
        }

        if self.killq.is_empty() && !self.killq.is_synced() {
            self.killq = MuxerKillQ::from_conn_map(&self.conn_map);
            // If we've just re-created the kill queue, we can sweep it again;
            // maybe there's more to kill.
            self.sweep_killq();
        }
    }

    /// Enqueue an RST packet into `self.rxq`.
    ///
    /// Enqueue errors aren't propagated up the call chain, since there is
    /// nothing we can do to handle them. We do, however, log a warning, since
    /// not being able to enqueue an RST packet means we have to drop it, which
    /// is not normal operation.
    fn enq_rst(&mut self, local_port: u32, peer_port: u32) {
        let pushed = self.rxq.push(MuxerRx::RstPkt {
            local_port,
            peer_port,
        });
        if !pushed {
            warn!(
                "vsock: muxer.rxq full; dropping RST packet for lp={}, pp={}",
                local_port, peer_port
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Read, Write};
    use std::ops::Drop;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};

    use virtio_queue::QueueT;
    use vmm_sys_util::tempfile::TempFile;

    use super::super::super::backend::VsockUnixStreamBackend;
    use super::super::super::csm::defs as csm_defs;
    use super::super::super::defs::RXQ_EVENT;
    use super::super::super::tests::TestContext as VsockTestContext;
    use super::*;

    const PEER_CID: u64 = 3;
    const PEER_BUF_ALLOC: u32 = 64 * 1024;

    struct MuxerTestContext {
        _vsock_test_ctx: VsockTestContext,
        pkt: VsockPacket,
        muxer: VsockMuxer,
        host_sock_path: String,
    }

    impl Drop for MuxerTestContext {
        fn drop(&mut self) {
            std::fs::remove_file(self.host_sock_path.as_str()).unwrap();
        }
    }

    // Create a TempFile with a given prefix and return it as a nice String
    fn get_file(fprefix: &str) -> String {
        let listener_path = TempFile::new_with_prefix(fprefix).unwrap();
        listener_path
            .as_path()
            .as_os_str()
            .to_str()
            .unwrap()
            .to_owned()
    }

    impl MuxerTestContext {
        fn new(name: &str) -> Self {
            let vsock_test_ctx = VsockTestContext::new();
            let mut handler_ctx = vsock_test_ctx.create_event_handler_context();
            let pkt = VsockPacket::from_rx_virtq_head(
                &mut handler_ctx.queues[RXQ_EVENT as usize]
                    .queue_mut()
                    .pop_descriptor_chain(&vsock_test_ctx.mem)
                    .unwrap(),
            )
            .unwrap();

            let host_sock_path = get_file(name);
            let mut muxer = VsockMuxer::new(PEER_CID).unwrap();
            let uds_backend =
                Box::new(VsockUnixStreamBackend::new(host_sock_path.clone()).unwrap());
            muxer.add_backend(uds_backend, true).unwrap();
            Self {
                _vsock_test_ctx: vsock_test_ctx,
                pkt,
                muxer,
                host_sock_path,
            }
        }

        fn init_pkt(&mut self, local_port: u32, peer_port: u32, op: u16) -> &mut VsockPacket {
            for b in self.pkt.hdr_mut() {
                *b = 0;
            }
            self.pkt
                .set_type(uapi::VSOCK_TYPE_STREAM)
                .set_src_cid(PEER_CID)
                .set_dst_cid(uapi::VSOCK_HOST_CID)
                .set_src_port(peer_port)
                .set_dst_port(local_port)
                .set_op(op)
                .set_buf_alloc(PEER_BUF_ALLOC)
        }

        fn init_data_pkt(
            &mut self,
            local_port: u32,
            peer_port: u32,
            data: &[u8],
        ) -> &mut VsockPacket {
            assert!(data.len() <= self.pkt.buf().unwrap().len());
            self.init_pkt(local_port, peer_port, uapi::VSOCK_OP_RW)
                .set_len(data.len() as u32);
            self.pkt.buf_mut().unwrap()[..data.len()].copy_from_slice(data);
            &mut self.pkt
        }

        fn send(&mut self) {
            self.muxer.send_pkt(&self.pkt).unwrap();
        }

        fn recv(&mut self) {
            self.muxer.recv_pkt(&mut self.pkt).unwrap();
        }

        fn notify_muxer(&mut self) {
            self.muxer.notify(epoll::Events::EPOLLIN);
        }

        fn count_epoll_listeners(&self) -> (usize, usize) {
            let mut local_lsn_count = 0usize;
            let mut conn_lsn_count = 0usize;
            for key in self.muxer.listener_map.values() {
                match key {
                    EpollListener::LocalStream(_) => local_lsn_count += 1,
                    EpollListener::Connection { .. } => conn_lsn_count += 1,
                    _ => (),
                };
            }
            (local_lsn_count, conn_lsn_count)
        }

        fn create_local_listener(&self, port: u32) -> LocalListener {
            LocalListener::new(format!("{}_{}", self.host_sock_path, port))
        }

        fn local_connect(&mut self, peer_port: u32) -> (UnixStream, u32) {
            let (init_local_lsn_count, init_conn_lsn_count) = self.count_epoll_listeners();

            let mut stream = UnixStream::connect(self.host_sock_path.clone()).unwrap();
            stream.set_nonblocking(true).unwrap();
            // The muxer would now get notified of a new connection having arrived at its Unix
            // socket, so it can accept it.
            self.notify_muxer();

            // Just after having accepted a new local connection, the muxer should've added a new
            // `LocalStream` listener to its `listener_map`.
            let (local_lsn_count, _) = self.count_epoll_listeners();
            assert_eq!(local_lsn_count, init_local_lsn_count + 1);

            let buf = format!("CONNECT {peer_port}\n");
            stream.write_all(buf.as_bytes()).unwrap();
            // The muxer would now get notified that data is available for reading from the locally
            // initiated connection.
            self.notify_muxer();

            // Successfully reading and parsing the connection request should have removed the
            // LocalStream epoll listener and added a Connection epoll listener.
            let (local_lsn_count, conn_lsn_count) = self.count_epoll_listeners();
            assert_eq!(local_lsn_count, init_local_lsn_count);
            assert_eq!(conn_lsn_count, init_conn_lsn_count + 1);

            // A LocalInit connection should've been added to the muxer connection map.  A new
            // local port should also have been allocated for the new LocalInit connection.
            let local_port = self.muxer.local_port_last;
            let key = ConnMapKey {
                local_port,
                peer_port,
            };
            assert!(self.muxer.conn_map.contains_key(&key));
            assert!(self.muxer.local_port_set.contains(&local_port));

            // A connection request for the peer should now be available from the muxer.
            assert!(self.muxer.has_pending_rx());
            self.recv();
            assert_eq!(self.pkt.op(), uapi::VSOCK_OP_REQUEST);
            assert_eq!(self.pkt.dst_port(), peer_port);
            assert_eq!(self.pkt.src_port(), local_port);

            self.init_pkt(local_port, peer_port, uapi::VSOCK_OP_RESPONSE);
            self.send();

            let mut buf = vec![0u8; 32];
            let len = stream.read(&mut buf[..]).unwrap();
            assert_eq!(&buf[..len], format!("OK {local_port}\n").as_bytes());

            (stream, local_port)
        }
    }

    struct LocalListener {
        path: PathBuf,
        sock: UnixListener,
    }
    impl LocalListener {
        fn new<P: AsRef<Path> + Clone>(path: P) -> Self {
            let path_buf = path.as_ref().to_path_buf();
            let sock = UnixListener::bind(path).unwrap();
            sock.set_nonblocking(true).unwrap();
            Self {
                path: path_buf,
                sock,
            }
        }
        fn accept(&mut self) -> UnixStream {
            let (stream, _) = self.sock.accept().unwrap();
            stream.set_nonblocking(true).unwrap();
            stream
        }
    }
    impl Drop for LocalListener {
        fn drop(&mut self) {
            std::fs::remove_file(&self.path).unwrap();
        }
    }

    #[test]
    fn test_muxer_epoll_listener() {
        let ctx = MuxerTestContext::new("/tmp/muxer_epoll_listener");
        assert_eq!(ctx.muxer.as_raw_fd(), ctx.muxer.epoll_fd);
        assert_eq!(ctx.muxer.get_polled_evset(), epoll::Events::EPOLLIN);
    }

    #[test]
    fn test_bad_peer_pkt() {
        const LOCAL_PORT: u32 = 1026;
        const PEER_PORT: u32 = 1025;
        const SOCK_DGRAM: u16 = 2;

        let mut ctx = MuxerTestContext::new("/tmp/bad_peer_pkt");
        ctx.init_pkt(LOCAL_PORT, PEER_PORT, uapi::VSOCK_OP_REQUEST)
            .set_type(SOCK_DGRAM);
        ctx.send();

        // The guest sent a SOCK_DGRAM packet. Per the vsock spec, we need to reply with an RST
        // packet, since vsock only supports stream sockets.
        assert!(ctx.muxer.has_pending_rx());
        ctx.recv();
        assert_eq!(ctx.pkt.op(), uapi::VSOCK_OP_RST);
        assert_eq!(ctx.pkt.src_cid(), uapi::VSOCK_HOST_CID);
        assert_eq!(ctx.pkt.dst_cid(), PEER_CID);
        assert_eq!(ctx.pkt.src_port(), LOCAL_PORT);
        assert_eq!(ctx.pkt.dst_port(), PEER_PORT);

        // Any orphan (i.e. without a connection), non-RST packet, should be replied to with an
        // RST.
        let bad_ops = [
            uapi::VSOCK_OP_RESPONSE,
            uapi::VSOCK_OP_CREDIT_REQUEST,
            uapi::VSOCK_OP_CREDIT_UPDATE,
            uapi::VSOCK_OP_SHUTDOWN,
            uapi::VSOCK_OP_RW,
        ];
        for op in bad_ops.iter() {
            ctx.init_pkt(LOCAL_PORT, PEER_PORT, *op);
            ctx.send();
            assert!(ctx.muxer.has_pending_rx());
            ctx.recv();
            assert_eq!(ctx.pkt.op(), uapi::VSOCK_OP_RST);
            assert_eq!(ctx.pkt.src_port(), LOCAL_PORT);
            assert_eq!(ctx.pkt.dst_port(), PEER_PORT);
        }

        // Any packet addressed to anything other than VSOCK_VHOST_CID should get dropped.
        assert!(!ctx.muxer.has_pending_rx());
        ctx.init_pkt(LOCAL_PORT, PEER_PORT, uapi::VSOCK_OP_REQUEST)
            .set_dst_cid(uapi::VSOCK_HOST_CID + 1);
        ctx.send();
        assert!(!ctx.muxer.has_pending_rx());
    }

    #[test]
    fn test_peer_connection() {
        const LOCAL_PORT: u32 = 1026;
        const PEER_PORT: u32 = 1025;

        let mut ctx = MuxerTestContext::new("/tmp/peer_connection");

        // Test peer connection refused.
        ctx.init_pkt(LOCAL_PORT, PEER_PORT, uapi::VSOCK_OP_REQUEST);
        ctx.send();
        ctx.recv();
        assert_eq!(ctx.pkt.op(), uapi::VSOCK_OP_RST);
        assert_eq!(ctx.pkt.len(), 0);
        assert_eq!(ctx.pkt.src_cid(), uapi::VSOCK_HOST_CID);
        assert_eq!(ctx.pkt.dst_cid(), PEER_CID);
        assert_eq!(ctx.pkt.src_port(), LOCAL_PORT);
        assert_eq!(ctx.pkt.dst_port(), PEER_PORT);

        // Test peer connection accepted.
        let mut listener = ctx.create_local_listener(LOCAL_PORT);
        ctx.init_pkt(LOCAL_PORT, PEER_PORT, uapi::VSOCK_OP_REQUEST);
        ctx.send();
        assert_eq!(ctx.muxer.conn_map.len(), 1);
        let mut stream = listener.accept();
        ctx.recv();
        assert_eq!(ctx.pkt.op(), uapi::VSOCK_OP_RESPONSE);
        assert_eq!(ctx.pkt.len(), 0);
        assert_eq!(ctx.pkt.src_cid(), uapi::VSOCK_HOST_CID);
        assert_eq!(ctx.pkt.dst_cid(), PEER_CID);
        assert_eq!(ctx.pkt.src_port(), LOCAL_PORT);
        assert_eq!(ctx.pkt.dst_port(), PEER_PORT);
        let key = ConnMapKey {
            local_port: LOCAL_PORT,
            peer_port: PEER_PORT,
        };
        assert!(ctx.muxer.conn_map.contains_key(&key));

        // Test guest -> host data flow.
        let data = [1, 2, 3, 4];
        ctx.init_data_pkt(LOCAL_PORT, PEER_PORT, &data);
        ctx.send();
        let mut buf = vec![0; data.len()];
        stream.read_exact(buf.as_mut_slice()).unwrap();
        assert_eq!(buf.as_slice(), data);

        // Test host -> guest data flow.
        let data = [5u8, 6, 7, 8];
        stream.write_all(&data).unwrap();

        // When data is available on the local stream, an EPOLLIN event would normally be delivered
        // to the muxer's nested epoll FD. For testing only, we can fake that event notification
        // here.
        ctx.notify_muxer();
        // After being notified, the muxer should've figured out that RX data was available for one
        // of its connections, so it should now be reporting that it can fill in an RX packet.
        assert!(ctx.muxer.has_pending_rx());
        ctx.recv();
        assert_eq!(ctx.pkt.op(), uapi::VSOCK_OP_RW);
        assert_eq!(ctx.pkt.buf().unwrap()[..data.len()], data);
        assert_eq!(ctx.pkt.src_port(), LOCAL_PORT);
        assert_eq!(ctx.pkt.dst_port(), PEER_PORT);

        assert!(!ctx.muxer.has_pending_rx());
    }

    #[test]
    fn test_local_connection() {
        let mut ctx = MuxerTestContext::new("/tmp/local_connection");
        let peer_port = 1025;
        let (mut stream, local_port) = ctx.local_connect(peer_port);

        // Test guest -> host data flow.
        let data = [1, 2, 3, 4];
        ctx.init_data_pkt(local_port, peer_port, &data);
        ctx.send();

        let mut buf = vec![0u8; data.len()];
        stream.read_exact(buf.as_mut_slice()).unwrap();
        assert_eq!(buf.as_slice(), &data);

        // Test host -> guest data flow.
        let data = [5, 6, 7, 8];
        stream.write_all(&data).unwrap();
        ctx.notify_muxer();

        assert!(ctx.muxer.has_pending_rx());
        ctx.recv();
        assert_eq!(ctx.pkt.op(), uapi::VSOCK_OP_RW);
        assert_eq!(ctx.pkt.src_port(), local_port);
        assert_eq!(ctx.pkt.dst_port(), peer_port);
        assert_eq!(ctx.pkt.buf().unwrap()[..data.len()], data);
    }

    #[test]
    fn test_local_close() {
        let peer_port = 1025;
        let mut ctx = MuxerTestContext::new("/tmp/local_close");
        let local_port;
        {
            let (_stream, local_port_) = ctx.local_connect(peer_port);
            local_port = local_port_;
        }
        // Local var `_stream` was now dropped, thus closing the local stream. After the muxer gets
        // notified via EPOLLIN, it should attempt to gracefully shutdown the connection, issuing a
        // VSOCK_OP_SHUTDOWN with both no-more-send and no-more-recv indications set.
        ctx.notify_muxer();
        assert!(ctx.muxer.has_pending_rx());
        ctx.recv();
        assert_eq!(ctx.pkt.op(), uapi::VSOCK_OP_SHUTDOWN);
        assert_ne!(ctx.pkt.flags() & uapi::VSOCK_FLAGS_SHUTDOWN_SEND, 0);
        assert_ne!(ctx.pkt.flags() & uapi::VSOCK_FLAGS_SHUTDOWN_RCV, 0);
        assert_eq!(ctx.pkt.src_port(), local_port);
        assert_eq!(ctx.pkt.dst_port(), peer_port);

        // The connection should get removed (and its local port freed), after the peer replies
        // with an RST.
        ctx.init_pkt(local_port, peer_port, uapi::VSOCK_OP_RST);
        ctx.send();
        let key = ConnMapKey {
            local_port,
            peer_port,
        };
        assert!(!ctx.muxer.conn_map.contains_key(&key));
        assert!(!ctx.muxer.local_port_set.contains(&local_port));
    }

    #[test]
    fn test_peer_close() {
        let peer_port = 1025;
        let local_port = 1026;
        let mut ctx = MuxerTestContext::new("/tmp/peer_close");

        let mut sock = ctx.create_local_listener(local_port);
        ctx.init_pkt(local_port, peer_port, uapi::VSOCK_OP_REQUEST);
        ctx.send();
        let mut stream = sock.accept();

        assert!(ctx.muxer.has_pending_rx());
        ctx.recv();
        assert_eq!(ctx.pkt.op(), uapi::VSOCK_OP_RESPONSE);
        assert_eq!(ctx.pkt.src_port(), local_port);
        assert_eq!(ctx.pkt.dst_port(), peer_port);
        let key = ConnMapKey {
            local_port,
            peer_port,
        };
        assert!(ctx.muxer.conn_map.contains_key(&key));

        // Emulate a full shutdown from the peer (no-more-send + no-more-recv).
        ctx.init_pkt(local_port, peer_port, uapi::VSOCK_OP_SHUTDOWN)
            .set_flag(uapi::VSOCK_FLAGS_SHUTDOWN_SEND)
            .set_flag(uapi::VSOCK_FLAGS_SHUTDOWN_RCV);
        ctx.send();

        // Now, the muxer should remove the connection from its map, and reply with an RST.
        assert!(ctx.muxer.has_pending_rx());
        ctx.recv();
        assert_eq!(ctx.pkt.op(), uapi::VSOCK_OP_RST);
        assert_eq!(ctx.pkt.src_port(), local_port);
        assert_eq!(ctx.pkt.dst_port(), peer_port);
        let key = ConnMapKey {
            local_port,
            peer_port,
        };
        assert!(!ctx.muxer.conn_map.contains_key(&key));

        // The muxer should also drop / close the local Unix socket for this connection.
        let mut buf = vec![0u8; 16];
        assert_eq!(stream.read(buf.as_mut_slice()).unwrap(), 0);
    }

    #[test]
    fn test_muxer_rxq() {
        let mut ctx = MuxerTestContext::new("/tmp/muxer_rxq");
        let local_port = 1026;
        let peer_port_first = 1025;
        let mut listener = ctx.create_local_listener(local_port);
        let mut streams: Vec<UnixStream> = Vec::new();

        for peer_port in peer_port_first..peer_port_first + defs::MUXER_RXQ_SIZE {
            ctx.init_pkt(local_port, peer_port as u32, uapi::VSOCK_OP_REQUEST);
            ctx.send();
            streams.push(listener.accept());
        }

        // The muxer RX queue should now be full (with connection reponses), but still
        // synchronized.
        assert!(ctx.muxer.rxq.is_synced());

        // One more queued reply should desync the RX queue.
        ctx.init_pkt(
            local_port,
            (peer_port_first + defs::MUXER_RXQ_SIZE) as u32,
            uapi::VSOCK_OP_REQUEST,
        );
        ctx.send();
        assert!(!ctx.muxer.rxq.is_synced());

        // With an out-of-sync queue, an RST should evict any non-RST packet from the queue, and
        // take its place. We'll check that by making sure that the last packet popped from the
        // queue is an RST.
        ctx.init_pkt(
            local_port + 1,
            peer_port_first as u32,
            uapi::VSOCK_OP_REQUEST,
        );
        ctx.send();

        for peer_port in peer_port_first..peer_port_first + defs::MUXER_RXQ_SIZE - 1 {
            ctx.recv();
            assert_eq!(ctx.pkt.op(), uapi::VSOCK_OP_RESPONSE);
            // The response order should hold. The evicted response should have been the last
            // enqueued.
            assert_eq!(ctx.pkt.dst_port(), peer_port as u32);
        }
        // There should be one more packet in the queue: the RST.
        assert_eq!(ctx.muxer.rxq.len(), 1);
        ctx.recv();
        assert_eq!(ctx.pkt.op(), uapi::VSOCK_OP_RST);

        // The queue should now be empty, but out-of-sync, so the muxer should report it has some
        // pending RX.
        assert!(ctx.muxer.rxq.is_empty());
        assert!(!ctx.muxer.rxq.is_synced());
        assert!(ctx.muxer.has_pending_rx());

        // The next recv should sync the queue back up. It should also yield one of the two
        // responses that are still left:
        // - the one that desynchronized the queue; and
        // - the one that got evicted by the RST.
        ctx.recv();
        assert!(ctx.muxer.rxq.is_synced());
        assert_eq!(ctx.pkt.op(), uapi::VSOCK_OP_RESPONSE);

        assert!(ctx.muxer.has_pending_rx());
        ctx.recv();
        assert_eq!(ctx.pkt.op(), uapi::VSOCK_OP_RESPONSE);
    }

    #[test]
    fn test_muxer_killq() {
        let mut ctx = MuxerTestContext::new("/tmp/muxer_killq");
        let local_port = 1026;
        let peer_port_first = 1025;
        let peer_port_last = peer_port_first + defs::MUXER_KILLQ_SIZE;
        let mut listener = ctx.create_local_listener(local_port);

        for peer_port in peer_port_first..=peer_port_last {
            ctx.init_pkt(local_port, peer_port as u32, uapi::VSOCK_OP_REQUEST);
            ctx.send();
            ctx.notify_muxer();
            ctx.recv();
            assert_eq!(ctx.pkt.op(), uapi::VSOCK_OP_RESPONSE);
            assert_eq!(ctx.pkt.src_port(), local_port);
            assert_eq!(ctx.pkt.dst_port(), peer_port as u32);
            {
                let _stream = listener.accept();
            }
            ctx.notify_muxer();
            ctx.recv();
            assert_eq!(ctx.pkt.op(), uapi::VSOCK_OP_SHUTDOWN);
            assert_eq!(ctx.pkt.src_port(), local_port);
            assert_eq!(ctx.pkt.dst_port(), peer_port as u32);
            // The kill queue should be synchronized, up until the `defs::MUXER_KILLQ_SIZE`th
            // connection we schedule for termination.
            assert_eq!(
                ctx.muxer.killq.is_synced(),
                peer_port < peer_port_first + defs::MUXER_KILLQ_SIZE
            );
        }

        assert!(!ctx.muxer.killq.is_synced());
        assert!(!ctx.muxer.has_pending_rx());

        // Wait for the kill timers to expire.
        std::thread::sleep(std::time::Duration::from_millis(
            csm_defs::CONN_SHUTDOWN_TIMEOUT_MS,
        ));

        // Trigger a kill queue sweep, by requesting a new connection.
        ctx.init_pkt(
            local_port,
            peer_port_last as u32 + 1,
            uapi::VSOCK_OP_REQUEST,
        );
        ctx.send();

        // After sweeping the kill queue, it should now be synced (assuming the RX queue is larger
        // than the kill queue, since an RST packet will be queued for each killed connection).
        assert!(ctx.muxer.killq.is_synced());
        assert!(ctx.muxer.has_pending_rx());
        // There should be `defs::MUXER_KILLQ_SIZE` RSTs in the RX queue, from terminating the
        // dying connections in the recent killq sweep.
        for _p in peer_port_first..peer_port_last {
            ctx.recv();
            assert_eq!(ctx.pkt.op(), uapi::VSOCK_OP_RST);
            assert_eq!(ctx.pkt.src_port(), local_port);
        }

        // There should be one more packet in the RX queue: the connection response our request
        // that triggered the kill queue sweep.
        ctx.recv();
        assert_eq!(ctx.pkt.op(), uapi::VSOCK_OP_RESPONSE);
        assert_eq!(ctx.pkt.dst_port(), peer_port_last as u32 + 1);

        assert!(!ctx.muxer.has_pending_rx());
    }

    #[test]
    fn test_regression_handshake() {
        // Address one of the issues found while fixing the following issue:
        // https://github.com/firecracker-microvm/firecracker/issues/1751
        // This test checks that the handshake message is not accounted for
        let mut ctx = MuxerTestContext::new("/tmp/regression_handshake");
        let peer_port = 1025;

        // Create a local connection.
        let (_, local_port) = ctx.local_connect(peer_port);

        // Get the connection from the connection map.
        let key = ConnMapKey {
            local_port,
            peer_port,
        };
        let conn = ctx.muxer.conn_map.get_mut(&key).unwrap();

        // Check that fwd_cnt is 0 - "OK ..." was not accounted for.
        assert_eq!(conn.fwd_cnt().0, 0);
    }

    #[test]
    fn test_regression_rxq_pop() {
        // Address one of the issues found while fixing the following issue:
        // https://github.com/firecracker-microvm/firecracker/issues/1751
        // This test checks that a connection is not popped out of the muxer
        // rxq when multiple flags are set
        let mut ctx = MuxerTestContext::new("/tmp/regression_rxq_pop");
        let peer_port = 1025;
        let (mut stream, local_port) = ctx.local_connect(peer_port);

        // Send some data.
        let data = [5u8, 6, 7, 8];
        stream.write_all(&data).unwrap();
        ctx.notify_muxer();

        // Get the connection from the connection map.
        let key = ConnMapKey {
            local_port,
            peer_port,
        };
        let conn = ctx.muxer.conn_map.get_mut(&key).unwrap();

        // Forcefully insert another flag.
        conn.insert_credit_update();

        // Call recv twice in order to check that the connection is still
        // in the rxq.
        assert!(ctx.muxer.has_pending_rx());
        ctx.recv();
        assert!(ctx.muxer.has_pending_rx());
        ctx.recv();

        // Since initially the connection had two flags set, now there should
        // not be any pending RX in the muxer.
        assert!(!ctx.muxer.has_pending_rx());
    }

    #[test]
    fn test_add_backend_to_muxer() {
        let host_sock_path_1 = String::from("/tmp/host_sock_path_muxer_1_1");
        let host_sock_path_2 = String::from("/tmp/host_sock_path_muxer_1_2");
        let host_sock_path_3 = String::from("/tmp/host_sock_path_muxer_1_3");
        fs::remove_file(Path::new(&host_sock_path_1)).unwrap_or_default();
        fs::remove_file(Path::new(&host_sock_path_2)).unwrap_or_default();
        fs::remove_file(Path::new(&host_sock_path_3)).unwrap_or_default();

        let mut muxer_1 = VsockMuxer::new(PEER_CID).unwrap();
        let uds_backend_1 =
            Box::new(VsockUnixStreamBackend::new(host_sock_path_1.clone()).unwrap());
        let uds_backend_2 =
            Box::new(VsockUnixStreamBackend::new(host_sock_path_2.clone()).unwrap());

        // add uds backend, ok
        assert!(muxer_1.add_backend(uds_backend_1, false).is_ok());
        // add another uds backend, err
        assert!(muxer_1.add_backend(uds_backend_2, false).is_err());

        let mut muxer_2 = VsockMuxer::new(PEER_CID).unwrap();
        let uds_backend_3 =
            Box::new(VsockUnixStreamBackend::new(host_sock_path_3.clone()).unwrap());
        assert!(muxer_2.add_backend(uds_backend_3, true).is_ok());
        // peer_backend need to be uds backend
        assert!(muxer_2.peer_backend == Some(VsockBackendType::UnixStream));

        fs::remove_file(Path::new(&host_sock_path_1)).unwrap_or_default();
        fs::remove_file(Path::new(&host_sock_path_2)).unwrap_or_default();
        fs::remove_file(Path::new(&host_sock_path_3)).unwrap_or_default();
    }
}
