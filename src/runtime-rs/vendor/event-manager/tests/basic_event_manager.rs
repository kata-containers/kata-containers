// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause
//
// A basic, single threaded application that only needs one type of
// subscriber: `CounterSubscriber`.
//
// The application has an `EventManager` and can register multiple subscribers
// of type `CounterSubscriber`.

use std::ops::Drop;

use event_manager::utilities::subscribers::CounterSubscriber;
use event_manager::{EventManager, SubscriberId, SubscriberOps};

struct App {
    event_manager: EventManager<CounterSubscriber>,
    subscribers_id: Vec<SubscriberId>,
}

impl App {
    fn new() -> Self {
        Self {
            event_manager: EventManager::<CounterSubscriber>::new().unwrap(),
            subscribers_id: vec![],
        }
    }

    fn add_subscriber(&mut self) {
        let counter_subscriber = CounterSubscriber::new();
        let id = self.event_manager.add_subscriber(counter_subscriber);
        self.subscribers_id.push(id);
    }

    fn run(&mut self) {
        let _ = self.event_manager.run_with_timeout(100);
    }

    fn inject_events_for(&mut self, subscriber_index: &[usize]) {
        for i in subscriber_index {
            let subscriber = self
                .event_manager
                .subscriber_mut(self.subscribers_id[*i])
                .unwrap();
            subscriber.trigger_event();
        }
    }

    fn clear_events_for(&mut self, subscriber_indices: &[usize]) {
        for i in subscriber_indices {
            let subscriber = self
                .event_manager
                .subscriber_mut(self.subscribers_id[*i])
                .unwrap();
            subscriber.clear_event();
        }
    }

    fn get_counters(&mut self) -> Vec<u64> {
        let mut result = Vec::<u64>::new();
        for subscriber_id in &self.subscribers_id {
            let subscriber = self.event_manager.subscriber_mut(*subscriber_id).unwrap();
            result.push(subscriber.counter());
        }

        result
    }

    // Whenever the App does not need to monitor events anymore, it should explicitly call
    // the `remove_subscriber` function.
    fn cleanup(&mut self) {
        for id in &self.subscribers_id {
            let _ = self.event_manager.remove_subscriber(*id);
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[test]
fn test_single_threaded() {
    let mut app = App::new();
    for _ in 0..100 {
        app.add_subscriber();
    }

    // Random subscribers, in the sense that I randomly picked those numbers :)
    let triggered_subscribers: Vec<usize> = vec![1, 3, 50, 97];
    app.inject_events_for(&triggered_subscribers);
    app.run();

    let counters = app.get_counters();
    for i in 0..100 {
        assert_eq!(counters[i], 1 & (triggered_subscribers.contains(&i) as u64));
    }

    app.clear_events_for(&triggered_subscribers);
    app.run();
    let counters = app.get_counters();
    for i in 0..100 {
        assert_eq!(counters[i], 1 & (triggered_subscribers.contains(&i) as u64));
    }

    // Once the app does not need events anymore, the cleanup needs to be called.
    // This is particularly important when the app continues the execution, but event monitoring
    // is not necessary.
    app.cleanup();
}
