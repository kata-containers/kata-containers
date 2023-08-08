// Copyright 2020 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! A simple wrapper over event_manager::EventManager to solve possible deadlock.

use anyhow::{anyhow, Result};
use std::sync::{Arc, Mutex};

pub use event_manager::{
    Error, EventManager, EventOps, EventSet, Events, MutEventSubscriber, RemoteEndpoint,
    SubscriberId, SubscriberOps,
};

/// Type of epoll subscriber.
pub type EpollSubscriber = Box<dyn MutEventSubscriber + Send>;

type EpollManagerImpl = Arc<Mutex<EventManager<EpollSubscriber>>>;

/// A wrapper struct over EventManager to solve possible deadlock.
///
/// It's a rather tough topic to deal with the epoll event manager in rust way.
/// The event_manager::EventManager is designed for single-threaded environment and it leaves
/// the task for concurrent access to the clients.
/// There are two types of threads involved, epoll worker thread and vCPU threads.
/// To reduce overhead, the epoll worker thread calls epoll::wait() without timeout, so the
/// worker thread will hold the EpollManagerImpl::Mutex for undetermined periods. When the vCPU
/// threads tries to activate virtio devices, they need to acquire the same EpollManagerImpl::Mutex.
/// Thus the vCPU threads may block for an undetermined time. To solve this issue, we perform
/// an kick()/try_lock() loop to wake up the epoll worker thread from sleeping.
#[derive(Clone)]
pub struct EpollManager {
    pub mgr: EpollManagerImpl,
    endpoint: Arc<Mutex<RemoteEndpoint<EpollSubscriber>>>,
}

impl EpollManager {
    /// Add a new epoll event subscriber.
    pub fn add_subscriber(&self, handler: EpollSubscriber) -> SubscriberId {
        let _ = self.endpoint.lock().unwrap().kick();
        if let Ok(mut mgr) = self.mgr.try_lock() {
            mgr.add_subscriber(handler)
        } else {
            return self
                .endpoint
                .lock()
                .unwrap()
                .call_blocking::<_, _, Error>(move |mgr| Ok(mgr.add_subscriber(handler)))
                .unwrap();
        }
    }

    /// Remove a given epoll event subscriber.
    pub fn remove_subscriber(&mut self, subscriber_id: SubscriberId) -> Result<EpollSubscriber> {
        let mut mgr = self
            .mgr
            .lock()
            .map_err(|e| anyhow!("EventManager lock fail. {:?}", e))?;
        mgr.remove_subscriber(subscriber_id)
            .map_err(|e| anyhow!("remove subscriber err. {:?}", e))
    }

    /// Add an epoll event to be monitored.
    pub fn add_event(
        &self,
        subscriber_id: SubscriberId,
        events: Events,
    ) -> std::result::Result<(), Error> {
        loop {
            let _ = self.endpoint.lock().unwrap().kick();
            if let Ok(mut mgr) = self.mgr.try_lock() {
                let mut ops = mgr.event_ops(subscriber_id)?;
                return ops.add(events);
            }
        }
    }

    /// Run the epoll polling loop.
    pub fn handle_events(&self, timeout: i32) -> std::result::Result<usize, Error> {
        // Do not expect poisoned lock.
        let mut guard = self.mgr.lock().unwrap();

        guard.run_with_timeout(timeout)
    }
}

impl Default for EpollManager {
    /// Create a new epoll manager.
    fn default() -> Self {
        let mgr = EventManager::new().expect("epoll_manager: failed create new instance");
        let endpoint = Arc::new(Mutex::new(mgr.remote_endpoint()));

        EpollManager {
            mgr: Arc::new(Mutex::new(mgr)),
            endpoint,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::io::AsRawFd;
    use vmm_sys_util::{epoll::EventSet, eventfd::EventFd};

    struct DummySubscriber {
        pub event: EventFd,
    }

    impl DummySubscriber {
        fn new() -> Self {
            Self {
                event: EventFd::new(0).unwrap(),
            }
        }
    }

    impl MutEventSubscriber for DummySubscriber {
        fn process(&mut self, events: Events, _ops: &mut EventOps) {
            let source = events.fd();
            let event_set = events.event_set();
            assert_ne!(source, self.event.as_raw_fd());
            match event_set {
                EventSet::IN => {
                    unreachable!()
                }
                EventSet::OUT => {
                    self.event.read().unwrap();
                }
                _ => {
                    unreachable!()
                }
            }
        }

        fn init(&mut self, _ops: &mut EventOps) {}
    }

    #[test]
    fn test_epoll_manager() {
        let mut epoll_manager = EpollManager::default();
        let epoll_manager_clone = epoll_manager.clone();
        let thread = std::thread::spawn(move || loop {
            let count = epoll_manager_clone.handle_events(-1).unwrap();
            if count == 0 {
                continue;
            }
            assert_eq!(count, 1);
            break;
        });
        let handler = DummySubscriber::new();
        let event = handler.event.try_clone().unwrap();
        let id = epoll_manager.add_subscriber(Box::new(handler));

        thread.join().unwrap();

        epoll_manager
            .add_event(id, Events::new(&event, EventSet::OUT))
            .unwrap();
        event.write(1).unwrap();

        let epoll_manager_clone = epoll_manager.clone();
        let thread = std::thread::spawn(move || loop {
            let count = epoll_manager_clone.handle_events(-1).unwrap();
            if count == 0 {
                continue;
            }
            assert_eq!(count, 2);
            break;
        });

        thread.join().unwrap();
        epoll_manager.remove_subscriber(id).unwrap();
    }
}
