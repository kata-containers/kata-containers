// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

use std::os::unix::{io::AsRawFd, net::UnixStream};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use event_manager::{
    EventManager, EventOps, EventSubscriber, Events, SubscriberOps, MAX_READY_EVENTS_CAPACITY,
};
use vmm_sys_util::epoll::EventSet;

#[derive(Debug)]
struct UnixStreamSubscriber {
    stream: UnixStream,
    rhup_count: AtomicU64,
    // When this flag is used, in the process function the subscriber will
    // unregister the fd where an error was received.
    // The errors that need to be handled: `EventSet::HANG_UP`, `EventSet::ERROR`.
    with_unregister_on_err: bool,
}

impl UnixStreamSubscriber {
    fn new(stream: UnixStream) -> UnixStreamSubscriber {
        Self {
            stream,
            rhup_count: AtomicU64::new(0),
            with_unregister_on_err: false,
        }
    }

    fn new_with_unregister_on_err(stream: UnixStream) -> UnixStreamSubscriber {
        Self {
            stream,
            rhup_count: AtomicU64::new(0),
            with_unregister_on_err: true,
        }
    }
}

impl EventSubscriber for UnixStreamSubscriber {
    fn process(&self, events: Events, ops: &mut EventOps<'_>) {
        if events.event_set().contains(EventSet::HANG_UP) {
            let _ = self.rhup_count.fetch_add(1, Ordering::Relaxed);
            if self.with_unregister_on_err {
                ops.remove(Events::empty(&self.stream)).unwrap();
            }
        }
    }

    fn init(&self, ops: &mut EventOps<'_>) {
        ops.add(Events::new(&self.stream, EventSet::IN)).unwrap();
    }
}

#[test]
fn test_handling_errors_in_subscriber() {
    let (sock1, sock2) = UnixStream::pair().unwrap();

    let mut event_manager = EventManager::<Arc<dyn EventSubscriber>>::new().unwrap();
    let subscriber = Arc::new(UnixStreamSubscriber::new(sock1));
    event_manager.add_subscriber(subscriber.clone());

    unsafe { libc::close(sock2.as_raw_fd()) };

    event_manager.run_with_timeout(100).unwrap();
    event_manager.run_with_timeout(100).unwrap();
    event_manager.run_with_timeout(100).unwrap();

    // Since the subscriber did not remove the event from its watch list, the
    // `EPOLLRHUP` error will continuously be a ready event each time `run` is called.
    // We called `run_with_timeout` 3 times, hence we expect `rhup_count` to be 3.
    assert_eq!(subscriber.rhup_count.load(Ordering::Relaxed), 3);

    let (sock1, sock2) = UnixStream::pair().unwrap();
    let subscriber_with_unregister =
        Arc::new(UnixStreamSubscriber::new_with_unregister_on_err(sock1));
    event_manager.add_subscriber(subscriber_with_unregister.clone());

    unsafe { libc::close(sock2.as_raw_fd()) };

    let ready_list_len = event_manager.run_with_timeout(100).unwrap();
    assert_eq!(ready_list_len, 2);
    // At this point the `subscriber_with_unregister` should not yield events anymore.
    // We expect the number of ready fds to be 1.
    let ready_list_len = event_manager.run_with_timeout(100).unwrap();
    assert_eq!(ready_list_len, 1);
}

#[test]
fn test_max_ready_list_size() {
    assert!(
        EventManager::<Arc<dyn EventSubscriber>>::new_with_capacity(MAX_READY_EVENTS_CAPACITY)
            .is_ok()
    );
    assert!(EventManager::<Arc<dyn EventSubscriber>>::new_with_capacity(
        MAX_READY_EVENTS_CAPACITY + 1
    )
    .is_err());
    assert!(EventManager::<Arc<dyn EventSubscriber>>::new_with_capacity(usize::MAX).is_err())
}
