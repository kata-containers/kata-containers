// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

use std::sync::{Arc, Mutex};
use std::thread;

use event_manager::utilities::subscribers::{
    CounterInnerMutSubscriber, CounterSubscriber, CounterSubscriberWithData,
};
use event_manager::{EventManager, EventSubscriber, MutEventSubscriber, SubscriberOps};

// Showcase how you can manage subscribers of different types using the same event manager
// in a multithreaded context.
#[test]
fn test_diff_type_subscribers() {
    let mut event_manager = EventManager::<Arc<Mutex<dyn MutEventSubscriber + Send>>>::new()
        .expect("Cannot create event manager.");

    // The `CounterSubscriberWithData` expects to receive a number as a parameter that represents
    // the number it can use as its inner Events data.
    let data_subscriber = Arc::new(Mutex::new(CounterSubscriberWithData::new(1)));
    data_subscriber.lock().unwrap().trigger_all_counters();

    let counter_subscriber = Arc::new(Mutex::new(CounterSubscriber::new()));
    counter_subscriber.lock().unwrap().trigger_event();

    // Saving the ids allows to modify the subscribers in the event manager context (i.e. removing
    // the subscribers from the event manager loop).
    let ds_id = event_manager.add_subscriber(data_subscriber);
    let cs_id = event_manager.add_subscriber(counter_subscriber);

    let thread_handle = thread::spawn(move || {
        // In a typical application this would be an infinite loop with some break condition.
        // For tests, 100 iterations should suffice.
        for _ in 0..100 {
            // When the event manager loop exits, it will return the number of events it
            // handled.
            let ev_count = event_manager.run().unwrap();
            // We are expecting the ev_count to be:
            // 4 = 3 (events triggered from data_subscriber) + 1 (event from counter_subscriber).
            assert_eq!(ev_count, 4);
        }

        // One really important thing is to always remove the subscribers once you don't need them
        // any more.
        let _ = event_manager.remove_subscriber(ds_id).unwrap();
        let _ = event_manager.remove_subscriber(cs_id).unwrap();
    });
    thread_handle.join().unwrap();
}

// Showcase how you can manage a single type of subscriber using the same event manager
// in a multithreaded context.
#[test]
fn test_one_type_subscriber() {
    let mut event_manager =
        EventManager::<CounterSubscriberWithData>::new().expect("Cannot create event manager");

    // The `CounterSubscriberWithData` expects to receive the number it can use as its inner
    // `Events` data as a parameter.
    // Let's make sure that the inner data of the two subscribers will not overlap by keeping some
    //  numbers between the first_data of each subscriber.
    let data_subscriber_1 = CounterSubscriberWithData::new(1);
    let data_subscriber_2 = CounterSubscriberWithData::new(1999);

    // Saving the ids allows to modify the subscribers in the event manager context (i.e. removing
    // the subscribers from the event manager loop).
    let ds_id_1 = event_manager.add_subscriber(data_subscriber_1);
    let ds_id_2 = event_manager.add_subscriber(data_subscriber_2);

    // Since we moved the ownership of the subscriber to the event manager, now we need to get
    // a mutable reference from it so that we can modify them.
    // Enclosing this in a code block so that the references are dropped when they're no longer
    // needed.
    {
        let data_subscriber_1 = event_manager.subscriber_mut(ds_id_1).unwrap();
        data_subscriber_1.trigger_all_counters();

        let data_subscriber_2 = event_manager.subscriber_mut(ds_id_2).unwrap();
        data_subscriber_2.trigger_all_counters();
    }

    let thread_handle = thread::spawn(move || {
        // In a typical application this would be an infinite loop with some break condition.
        // For tests, 100 iterations should suffice.
        for _ in 0..100 {
            // When the event manager loop exits, it will return the number of events it
            // handled.
            let ev_count = event_manager.run().unwrap();
            // Since we triggered all counters, we're expecting the number of events to always be
            // 6 (3 from one subscriber and 3 from the other).
            assert_eq!(ev_count, 6);
        }

        // One really important thing is to always remove the subscribers once you don't need them
        // any more.
        // When calling remove_subscriber, you receive ownership of the subscriber object.
        let data_subscriber_1 = event_manager.remove_subscriber(ds_id_1).unwrap();
        let data_subscriber_2 = event_manager.remove_subscriber(ds_id_2).unwrap();

        // Let's check how the subscribers look after 100 iterations.
        // Each subscriber's counter start from 0. When an event is triggered, each counter
        // is incremented. So after 100 iterations when they're all triggered, we expect them
        // to be 100.
        let expected_subscriber_counters = vec![100, 100, 100];
        assert_eq!(
            data_subscriber_1.get_all_counter_values(),
            expected_subscriber_counters
        );
        assert_eq!(
            data_subscriber_2.get_all_counter_values(),
            expected_subscriber_counters
        );
    });

    thread_handle.join().unwrap();
}

// Showcase how you can manage subscribers that make use of inner mutability using the event manager
// in a multithreaded context.
#[test]
fn test_subscriber_inner_mut() {
    // We are using just the `CounterInnerMutSubscriber` subscriber, so we could've initialize
    // the event manager as: EventManager::<CounterInnerMutSubscriber>::new().
    // The typical application will have more than one subscriber type, so let's just pretend
    // this is the case in this example as well.
    let mut event_manager = EventManager::<Arc<dyn EventSubscriber + Send + Sync>>::new()
        .expect("Cannot create event manager");

    let subscriber = Arc::new(CounterInnerMutSubscriber::new());
    subscriber.trigger_event();

    // Let's just clone the subscriber before adding it to the event manager. This will allow us
    // to use it from this thread without the need to call into EventManager::subscriber_mut().
    let subscriber_id = event_manager.add_subscriber(subscriber.clone());

    let thread_handle = thread::spawn(move || {
        for _ in 0..100 {
            // When the event manager loop exits, it will return the number of events it
            // handled.
            let ev_count = event_manager.run().unwrap();
            assert_eq!(ev_count, 1);
        }

        assert!(event_manager.remove_subscriber(subscriber_id).is_ok());
    });
    thread_handle.join().unwrap();

    assert_eq!(subscriber.counter(), 100);
}
