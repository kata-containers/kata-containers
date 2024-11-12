// Copyright 2022 Alibaba Cloud. All Rights Reserved.
//
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

/// This module implements a muxer for vsock - a mediator between guest-side
/// AF_VSOCK sockets and host-side backends. The heavy lifting is performed by
/// `muxer::VsockMuxer`, a connection multiplexer that uses
/// `super::csm::VsockConnection` for handling vsock connection states. Check
/// out `muxer.rs` for a more detailed explanation of the inner workings of this
/// backend.
pub mod muxer_impl;
pub mod muxer_killq;
pub mod muxer_rxq;

use super::backend::{VsockBackend, VsockBackendType};
use super::{VsockChannel, VsockEpollListener};
pub use muxer_impl::VsockMuxer;

mod defs {
    /// Maximum number of established connections that we can handle.
    pub const MAX_CONNECTIONS: usize = 1023;

    /// Size of the muxer RX packet queue.
    pub const MUXER_RXQ_SIZE: usize = 256;

    /// Size of the muxer connection kill queue.
    pub const MUXER_KILLQ_SIZE: usize = 128;
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Error registering a new epoll-listening FD.
    #[error("error when registering a new epoll-listening FD: {0}")]
    EpollAdd(#[source] std::io::Error),

    /// Error creating an epoll FD.
    #[error("error when creating an epoll: {0}")]
    EpollFdCreate(#[source] std::io::Error),

    /// The host made an invalid vsock port connection request.
    #[error("invalid vsock prot connection request")]
    InvalidPortRequest,

    /// Cannot add muxer backend when vsock device is activated.
    #[error("cannot add muxer backend when vsock device is activated")]
    BackendAddAfterActivated,

    /// Error accepting a new connection from backend.
    #[error("error accepting a new connection from backend: {0}")]
    BackendAccept(#[source] std::io::Error),

    /// Error binding to the backend.
    #[error("error binding to the backend: {0}")]
    BackendBind(#[source] std::io::Error),

    /// Error connecting to a backend.
    #[error("error connecting to a backend: {0}")]
    BackendConnect(#[source] std::io::Error),

    /// Error set nonblock to a backend stream.
    #[error("error set nonblocking to a backend: {0}")]
    BackendSetNonBlock(#[source] std::io::Error),

    /// Error reading from backend.
    #[error("error reading from backend: {0}")]
    BackendRead(#[source] std::io::Error),

    /// Muxer connection limit reached.
    #[error("muxer reaches connection limit")]
    TooManyConnections,

    /// Backend type has been registered.
    #[error("backend type has been registered: {0:?}")]
    BackendRegistered(VsockBackendType),
}

/// The vsock generic muxer, which is basically an epoll-event-driven vsock
/// channel. Currently, the only implementation we have is
/// `vsock::muxer::muxer::VsockMuxer`, which translates guest-side vsock
/// connections to host-side connections with different backends.
pub trait VsockGenericMuxer: VsockChannel + VsockEpollListener + Send {
    fn add_backend(&mut self, backend: Box<dyn VsockBackend>, is_peer_backend: bool) -> Result<()>;
}
