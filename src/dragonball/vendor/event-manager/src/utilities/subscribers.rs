// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause
//
// Cargo thinks that some of the methods in this module are not used because
// not all of them are used in all the separate integration test modules.
// Cargo bug: https://github.com/rust-lang/rust/issues/46379
// Let's allow dead code so that we don't get warnings all the time.
/// This module defines common subscribers that showcase the usage of the event-manager.
///
/// 1. CounterSubscriber:
///     - a dummy subscriber that increments a counter on event
///     - only uses one event
///     - it has to be explicitly mutated; for this reason it implements `MutEventSubscriber`.
///
/// 2. CounterSubscriberWithData:
///     - a dummy subscriber that increments a counter on events
///     - this subscriber takes care of multiple events and makes use of `Events::with_data` so
///       that in the `process` function it identifies the trigger of an event using the data
///       instead of the file descriptor
///     - it has to be explicitly mutated; for this reason it implements `MutEventSubscriber`.
///
/// 3. CounterInnerMutSubscriber:
///     - a dummy subscriber that increments a counter on events
///     - the subscriber makes use of inner mutability; multi-threaded applications might want to
///       use inner mutability instead of having something heavy weight (i.e. Arc<Mutex>).
///     - this subscriber implement `EventSubscriber`.
use std::fmt::{Display, Formatter, Result};
use std::os::unix::io::AsRawFd;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use vmm_sys_util::{epoll::EventSet, eventfd::EventFd};

use crate::{EventOps, EventSubscriber, Events, MutEventSubscriber};

/// A `Counter` is a helper structure for creating subscribers that increment a value
/// each time an event is triggered.
/// The `Counter` allows users to assert and de-assert an event, and to query the counter value.
pub struct Counter {
    event_fd: EventFd,
    counter: u64,
}

impl Display for Counter {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(
            f,
            "(event_fd = {}, counter = {})",
            self.event_fd.as_raw_fd(),
            self.counter
        )
    }
}

impl Counter {
    pub fn new() -> Self {
        Self {
            event_fd: EventFd::new(0).unwrap(),
            counter: 0,
        }
    }

    pub fn trigger_event(&mut self) {
        let _ = self.event_fd.write(1);
    }

    pub fn clear_event(&self) {
        let _ = self.event_fd.read();
    }

    pub fn counter(&self) -> u64 {
        self.counter
    }
}

impl Default for Counter {
    fn default() -> Self {
        Self::new()
    }
}

// A dummy subscriber that increments a counter whenever it processes
// a new request.
pub struct CounterSubscriber(Counter);

impl std::ops::Deref for CounterSubscriber {
    type Target = Counter;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for CounterSubscriber {
    fn deref_mut(&mut self) -> &mut Counter {
        &mut self.0
    }
}

impl CounterSubscriber {
    pub fn new() -> Self {
        CounterSubscriber(Counter::new())
    }
}

impl MutEventSubscriber for CounterSubscriber {
    fn process(&mut self, events: Events, event_ops: &mut EventOps) {
        match events.event_set() {
            EventSet::IN => {
                self.counter += 1;
            }
            EventSet::ERROR => {
                eprintln!("Got error on the monitored event.");
            }
            EventSet::HANG_UP => {
                event_ops
                    .remove(events)
                    .unwrap_or(eprintln!("Encountered error during cleanup"));
                panic!("Cannot continue execution. Associated fd was closed.");
            }
            _ => {
                eprintln!(
                    "Received spurious event from the event manager {:#?}.",
                    events.event_set()
                );
            }
        }
    }

    fn init(&mut self, ops: &mut EventOps) {
        ops.add(Events::new(&self.event_fd, EventSet::IN))
            .expect("Cannot register event.");
    }
}

// A dummy subscriber that makes use of the optional data in `Events` when
// registering & processing events.
// Using 3 counters each having associated event data to showcase the implementation of
// EventSubscriber trait with this scenario.
pub struct CounterSubscriberWithData {
    counter_1: Counter,
    counter_2: Counter,
    counter_3: Counter,

    first_data: u32,
    toggle_registry: bool,
}

impl CounterSubscriberWithData {
    // `first_data` represents the first event data that can be used by this subscriber.
    pub fn new(first_data: u32) -> Self {
        Self {
            counter_1: Counter::new(),
            counter_2: Counter::new(),
            counter_3: Counter::new(),
            // Using consecutive numbers for the event data helps the compiler to optimize
            // match statements on counter_1_data, counter_2_data, counter_3_data using
            // a jump table.
            first_data,
            toggle_registry: false,
        }
    }

    pub fn trigger_all_counters(&mut self) {
        self.counter_1.trigger_event();
        self.counter_2.trigger_event();
        self.counter_3.trigger_event();
    }

    pub fn get_all_counter_values(&self) -> Vec<u64> {
        vec![
            self.counter_1.counter(),
            self.counter_2.counter(),
            self.counter_3.counter(),
        ]
    }

    pub fn set_toggle_registry(&mut self, toggle: bool) {
        self.toggle_registry = toggle;
    }
}

impl MutEventSubscriber for CounterSubscriberWithData {
    fn process(&mut self, events: Events, ops: &mut EventOps) {
        if self.toggle_registry {
            self.toggle_registry = false;

            ops.remove(Events::with_data(
                &self.counter_1.event_fd,
                self.first_data,
                EventSet::IN,
            ))
            .expect("Cannot remove event.");
            ops.remove(Events::with_data(
                &self.counter_2.event_fd,
                self.first_data + 1,
                EventSet::IN,
            ))
            .expect("Cannot remove event.");
            ops.remove(Events::with_data(
                &self.counter_3.event_fd,
                self.first_data + 2,
                EventSet::IN,
            ))
            .expect("Cannot remove event.");

            ops.add(Events::with_data(
                &self.counter_1.event_fd,
                self.first_data,
                EventSet::IN,
            ))
            .expect("Cannot register event.");
            ops.add(Events::with_data(
                &self.counter_2.event_fd,
                self.first_data + 1,
                EventSet::IN,
            ))
            .expect("Cannot register event.");
            ops.add(Events::with_data(
                &self.counter_3.event_fd,
                self.first_data + 2,
                EventSet::IN,
            ))
            .expect("Cannot register event.");
        }
        match events.event_set() {
            EventSet::IN => {
                let event_id = events.data() - self.first_data;
                match event_id {
                    0 => {
                        self.counter_1.counter += 1;
                    }
                    1 => {
                        self.counter_2.counter += 1;
                    }
                    2 => {
                        self.counter_3.counter += 1;
                    }
                    _ => {
                        eprintln!("Received spurious event.");
                    }
                };
            }
            EventSet::ERROR => {
                eprintln!("Got error on the monitored event.");
            }
            EventSet::HANG_UP => {
                ops.remove(events)
                    .unwrap_or(eprintln!("Encountered error during cleanup"));
                panic!("Cannot continue execution. Associated fd was closed.");
            }
            _ => {}
        }
    }

    fn init(&mut self, ops: &mut EventOps) {
        ops.add(Events::with_data(
            &self.counter_1.event_fd,
            self.first_data,
            EventSet::IN,
        ))
        .expect("Cannot register event.");
        ops.add(Events::with_data(
            &self.counter_2.event_fd,
            self.first_data + 1,
            EventSet::IN,
        ))
        .expect("Cannot register event.");
        ops.add(Events::with_data(
            &self.counter_3.event_fd,
            self.first_data + 2,
            EventSet::IN,
        ))
        .expect("Cannot register event.");
    }
}

pub struct CounterInnerMutSubscriber {
    event_fd: EventFd,
    counter: AtomicU64,
}

impl CounterInnerMutSubscriber {
    pub fn new() -> Self {
        Self {
            event_fd: EventFd::new(0).unwrap(),
            counter: AtomicU64::new(0),
        }
    }

    pub fn trigger_event(&self) {
        let _ = self.event_fd.write(1);
    }

    pub fn clear_event(&self) {
        let _ = self.event_fd.read();
    }

    pub fn counter(&self) -> u64 {
        self.counter.load(Ordering::Relaxed)
    }
}

impl EventSubscriber for CounterInnerMutSubscriber {
    fn process(&self, events: Events, ops: &mut EventOps) {
        match events.event_set() {
            EventSet::IN => {
                self.counter.fetch_add(1, Ordering::Relaxed);
            }
            EventSet::ERROR => {
                eprintln!("Got error on the monitored event.");
            }
            EventSet::HANG_UP => {
                ops.remove(events)
                    .unwrap_or(eprintln!("Encountered error during cleanup"));
                panic!("Cannot continue execution. Associated fd was closed.");
            }
            _ => {}
        }
    }

    fn init(&self, ops: &mut EventOps) {
        ops.add(Events::new(&self.event_fd, EventSet::IN))
            .expect("Cannot register event.");
    }
}
