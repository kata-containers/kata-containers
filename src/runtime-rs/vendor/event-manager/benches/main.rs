// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

use criterion::{criterion_group, criterion_main, Criterion};

use event_manager::utilities::subscribers::{
    CounterInnerMutSubscriber, CounterSubscriber, CounterSubscriberWithData,
};
use event_manager::{EventManager, EventSubscriber, MutEventSubscriber, SubscriberOps};
use std::sync::{Arc, Mutex};

// Test the performance of event manager when it manages a single subscriber type.
// The performance is assessed under stress, all added subscribers have active events.
fn run_basic_subscriber(c: &mut Criterion) {
    let no_of_subscribers = 200;

    let mut event_manager = EventManager::<CounterSubscriber>::new().unwrap();
    for _ in 0..no_of_subscribers {
        let mut counter_subscriber = CounterSubscriber::new();
        counter_subscriber.trigger_event();
        event_manager.add_subscriber(counter_subscriber);
    }

    c.bench_function("process_basic", |b| {
        b.iter(|| {
            let ev_count = event_manager.run().unwrap();
            assert_eq!(ev_count, no_of_subscribers)
        })
    });
}

// Test the performance of event manager when the subscribers are wrapped in an Arc<Mutex>.
// The performance is assessed under stress, all added subscribers have active events.
fn run_arc_mutex_subscriber(c: &mut Criterion) {
    let no_of_subscribers = 200;

    let mut event_manager = EventManager::<Arc<Mutex<CounterSubscriber>>>::new().unwrap();
    for _ in 0..no_of_subscribers {
        let counter_subscriber = Arc::new(Mutex::new(CounterSubscriber::new()));
        counter_subscriber.lock().unwrap().trigger_event();
        event_manager.add_subscriber(counter_subscriber);
    }

    c.bench_function("process_with_arc_mutex", |b| {
        b.iter(|| {
            let ev_count = event_manager.run().unwrap();
            assert_eq!(ev_count, no_of_subscribers)
        })
    });
}

// Test the performance of event manager when the subscribers are wrapped in an Arc, and they
// leverage inner mutability to update their internal state.
// The performance is assessed under stress, all added subscribers have active events.
fn run_subscriber_with_inner_mut(c: &mut Criterion) {
    let no_of_subscribers = 200;

    let mut event_manager = EventManager::<Arc<dyn EventSubscriber + Send + Sync>>::new().unwrap();
    for _ in 0..no_of_subscribers {
        let counter_subscriber = CounterInnerMutSubscriber::new();
        counter_subscriber.trigger_event();
        event_manager.add_subscriber(Arc::new(counter_subscriber));
    }

    c.bench_function("process_with_inner_mut", |b| {
        b.iter(|| {
            let ev_count = event_manager.run().unwrap();
            assert_eq!(ev_count, no_of_subscribers)
        })
    });
}

// Test the performance of event manager when it manages subscribers of different types, that are
// wrapped in an Arc<Mutex>. Also, make use of `Events` with custom user data
// (using CounterSubscriberWithData).
// The performance is assessed under stress, all added subscribers have active events, and the
// CounterSubscriberWithData subscribers have multiple active events.
fn run_multiple_subscriber_types(c: &mut Criterion) {
    let no_of_subscribers = 100;

    let mut event_manager = EventManager::<Arc<Mutex<dyn MutEventSubscriber>>>::new()
        .expect("Cannot create event manager.");

    for i in 0..no_of_subscribers {
        // The `CounterSubscriberWithData` expects to receive a number as a parameter that
        // represents the number it can use as its inner Events data.
        let mut data_subscriber = CounterSubscriberWithData::new(i * no_of_subscribers);
        data_subscriber.trigger_all_counters();
        event_manager.add_subscriber(Arc::new(Mutex::new(data_subscriber)));

        let mut counter_subscriber = CounterSubscriber::new();
        counter_subscriber.trigger_event();
        event_manager.add_subscriber(Arc::new(Mutex::new(counter_subscriber)));
    }

    c.bench_function("process_dynamic_dispatch", |b| {
        b.iter(|| {
            let _ = event_manager.run().unwrap();
        })
    });
}

// Test the performance of event manager when it manages a single subscriber type.
// Just a few of the events are active in this test scenario.
fn run_with_few_active_events(c: &mut Criterion) {
    let no_of_subscribers = 200;

    let mut event_manager = EventManager::<CounterSubscriber>::new().unwrap();

    for i in 0..no_of_subscribers {
        let mut counter_subscriber = CounterSubscriber::new();
        // Let's activate the events for a few subscribers (i.e. only the ones that are
        // divisible by 23). 23 is a random number that I just happen to like.
        if i % 23 == 0 {
            counter_subscriber.trigger_event();
        }
        event_manager.add_subscriber(counter_subscriber);
    }

    c.bench_function("process_dispatch_few_events", |b| {
        b.iter(|| {
            let _ = event_manager.run().unwrap();
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(200)
        .measurement_time(std::time::Duration::from_secs(40));
    targets = run_basic_subscriber, run_arc_mutex_subscriber, run_subscriber_with_inner_mut,
              run_multiple_subscriber_types, run_with_few_active_events
}

criterion_main! {
    benches
}
