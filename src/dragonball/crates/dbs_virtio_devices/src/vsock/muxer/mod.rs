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

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

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

/// The ports identifying one host/guest virtio-vsock connection.
///
/// Host resources such as sockets and epoll registrations cannot survive a
/// snapshot restore. Persisting the tuple lets a new muxer reset the stale
/// guest-side socket instead of attempting to recreate those resources.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct VsockConnectionId {
    /// Port allocated on the host side of the connection.
    pub local_port: u32,
    /// Port used by the guest side of the connection.
    pub peer_port: u32,
}

#[derive(Debug, Default)]
struct VsockSnapshotTrackerState {
    active: BTreeSet<VsockConnectionId>,
    pending_resets: BTreeMap<VsockConnectionId, usize>,
}

/// Connection state shared by the device and its activated muxer.
///
/// The muxer moves into the epoll handler on activation, while device state is
/// later saved through the original `Vsock` object. This tracker keeps only
/// connection identifiers, and is updated on connection lifecycle events
/// rather than on the per-packet data path.
#[derive(Debug, Default)]
pub(crate) struct VsockSnapshotTracker {
    state: Mutex<VsockSnapshotTrackerState>,
}

impl VsockSnapshotTracker {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, VsockSnapshotTrackerState>> {
        self.state
            .lock()
            .map_err(|_| Error::SnapshotTrackerPoisoned)
    }

    pub(crate) fn add_active(&self, id: VsockConnectionId) -> Result<()> {
        self.lock()?.active.insert(id);
        Ok(())
    }

    pub(crate) fn remove_active(&self, id: VsockConnectionId) -> Result<()> {
        self.lock()?.active.remove(&id);
        Ok(())
    }

    pub(crate) fn add_pending_reset(&self, id: VsockConnectionId) -> Result<()> {
        let mut state = self.lock()?;
        *state.pending_resets.entry(id).or_default() += 1;
        Ok(())
    }

    pub(crate) fn remove_pending_reset(&self, id: VsockConnectionId) -> Result<()> {
        let mut state = self.lock()?;
        if let Some(count) = state.pending_resets.get_mut(&id) {
            *count -= 1;
            if *count == 0 {
                state.pending_resets.remove(&id);
            }
        }
        Ok(())
    }

    /// Re-record a reset whose delivery to the guest did not complete.
    ///
    /// The obligation is restored rather than treated as fatal: the tuple goes
    /// back into `pending_resets`, so it is carried in the next snapshot and
    /// the restored muxer sends the reset again. Failing snapshot creation here
    /// would be both redundant and unrecoverable, since nothing ever clears
    /// such a condition for the life of the device.
    pub(crate) fn mark_reset_delivery_failed(&self, id: VsockConnectionId) -> Result<()> {
        let mut state = self.lock()?;
        *state.pending_resets.entry(id).or_default() += 1;
        Ok(())
    }

    pub(crate) fn snapshot_connections(&self) -> Result<Vec<VsockConnectionId>> {
        let state = self.lock()?;
        let mut connections = state.active.clone();
        connections.extend(state.pending_resets.keys().copied());
        Ok(connections.into_iter().collect())
    }
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

    /// Snapshot connection bookkeeping was poisoned by a panic.
    #[error("vsock snapshot connection tracker is poisoned")]
    SnapshotTrackerPoisoned,

    /// Restored connection state is invalid.
    #[error("invalid restored vsock connection state: {0}")]
    InvalidRestoreState(String),

    /// A muxer implementation does not support restoring connection resets.
    #[error("vsock muxer does not support restoring connection resets")]
    RestoreResetUnsupported,

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

    /// Queue resets for connections whose host endpoints were not restored.
    fn queue_restore_resets(&mut self, connections: &[VsockConnectionId]) -> Result<()> {
        if connections.is_empty() {
            Ok(())
        } else {
            Err(Error::RestoreResetUnsupported)
        }
    }

    /// Preserve a reset obligation when committing it to guest memory fails.
    fn mark_reset_delivery_failed(&mut self, _id: VsockConnectionId) -> Result<()> {
        Ok(())
    }
}
