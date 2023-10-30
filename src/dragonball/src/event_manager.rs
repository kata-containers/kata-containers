// Copyright (C) 2020-2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

//! Event manager to manage and handle IO events and requests from API server .

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use dbs_utils::epoll_manager::{
    EpollManager, EventOps, EventSet, Events, MutEventSubscriber, SubscriberId,
};
use log::{error, warn};
use vmm_sys_util::eventfd::EventFd;

use crate::error::{EpollError, Result};
use crate::vmm::Vmm;

// Statically assigned epoll slot for VMM events.
pub(crate) const EPOLL_EVENT_EXIT: u32 = 0;
pub(crate) const EPOLL_EVENT_API_REQUEST: u32 = 1;

/// Shared information between vmm::vmm_thread_event_loop() and VmmEpollHandler.
#[derive(Debug)]
pub(crate) struct EventContext {
    pub api_event_fd: EventFd,
    pub api_event_triggered: bool,
    pub exit_evt_triggered: bool,
}

impl EventContext {
    /// Create a new instance of [`EventContext`].
    pub fn new(api_event_fd: EventFd) -> Result<Self> {
        Ok(EventContext {
            api_event_fd,
            api_event_triggered: false,
            exit_evt_triggered: false,
        })
    }
}

/// Event manager for VMM to handle API requests and IO events.
pub struct EventManager {
    epoll_mgr: EpollManager,
    subscriber_id: SubscriberId,
    vmm_event_count: Arc<AtomicUsize>,
}

impl Drop for EventManager {
    fn drop(&mut self) {
        // Vmm -> Vm -> EpollManager ->  VmmEpollHandler -> Vmm
        // We need to remove VmmEpollHandler to break the circular reference
        // so that Vmm can drop.
        self.epoll_mgr
            .remove_subscriber(self.subscriber_id)
            .map_err(|e| {
                error!("event_manager: remove_subscriber err. {:?}", e);
                e
            })
            .ok();
    }
}

impl EventManager {
    /// Create a new event manager associated with the VMM object.
    pub fn new(vmm: &Arc<Mutex<Vmm>>, epoll_mgr: EpollManager) -> Result<Self> {
        let vmm_event_count = Arc::new(AtomicUsize::new(0));
        let handler: Box<dyn MutEventSubscriber + Send> = Box::new(VmmEpollHandler {
            vmm: vmm.clone(),
            vmm_event_count: vmm_event_count.clone(),
        });
        let subscriber_id = epoll_mgr.add_subscriber(handler);

        Ok(EventManager {
            epoll_mgr,
            subscriber_id,
            vmm_event_count,
        })
    }

    /// Get the underlying epoll event manager.
    pub fn epoll_manager(&self) -> EpollManager {
        self.epoll_mgr.clone()
    }

    /// Registry the eventfd for exit notification.
    pub fn register_exit_eventfd(
        &mut self,
        exit_evt: &EventFd,
    ) -> std::result::Result<(), EpollError> {
        let events = Events::with_data(exit_evt, EPOLL_EVENT_EXIT, EventSet::IN);

        self.epoll_mgr
            .add_event(self.subscriber_id, events)
            .map_err(EpollError::EpollMgr)
    }

    /// Poll pending events and invoke registered event handler.
    ///
    /// # Arguments:
    /// * timeout: maximum time in milliseconds to wait
    pub fn handle_events(&self, timeout: i32) -> std::result::Result<usize, EpollError> {
        self.epoll_mgr
            .handle_events(timeout)
            .map_err(EpollError::EpollMgr)
    }

    /// Fetch the VMM event count and reset it to zero.
    pub fn fetch_vmm_event_count(&self) -> usize {
        self.vmm_event_count.swap(0, Ordering::AcqRel)
    }
}

struct VmmEpollHandler {
    vmm: Arc<Mutex<Vmm>>,
    vmm_event_count: Arc<AtomicUsize>,
}

impl MutEventSubscriber for VmmEpollHandler {
    fn process(&mut self, events: Events, _ops: &mut EventOps) {
        // Do not try to recover when the lock has already been poisoned.
        // And be careful to avoid deadlock between process() and vmm::vmm_thread_event_loop().
        let mut vmm = self.vmm.lock().unwrap();

        match events.data() {
            EPOLL_EVENT_API_REQUEST => {
                if let Err(e) = vmm.event_ctx.api_event_fd.read() {
                    error!("event_manager: failed to read API eventfd, {:?}", e);
                }
                vmm.event_ctx.api_event_triggered = true;
                self.vmm_event_count.fetch_add(1, Ordering::AcqRel);
            }
            EPOLL_EVENT_EXIT => {
                let vm = vmm.get_vm().unwrap();
                match vm.get_reset_eventfd() {
                    Some(ev) => {
                        if let Err(e) = ev.read() {
                            error!("event_manager: failed to read exit eventfd, {:?}", e);
                        }
                    }
                    None => warn!("event_manager: leftover exit event in epoll context!"),
                }
                vmm.event_ctx.exit_evt_triggered = true;
                self.vmm_event_count.fetch_add(1, Ordering::AcqRel);
            }
            _ => error!("event_manager: unknown epoll slot number {}", events.data()),
        }
    }

    fn init(&mut self, ops: &mut EventOps) {
        // Do not expect poisoned lock.
        let vmm = self.vmm.lock().unwrap();
        let events = Events::with_data(
            &vmm.event_ctx.api_event_fd,
            EPOLL_EVENT_API_REQUEST,
            EventSet::IN,
        );
        if let Err(e) = ops.add(events) {
            error!(
                "event_manager: failed to register epoll event for API server, {:?}",
                e
            );
        }
    }
}
