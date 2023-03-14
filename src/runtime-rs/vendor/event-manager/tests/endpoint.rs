// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

#![cfg(feature = "remote_endpoint")]

use std::any::Any;
use std::result;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use event_manager::utilities::subscribers::CounterSubscriber;
use event_manager::{self, EventManager, MutEventSubscriber, SubscriberId};

trait GenericSubscriber: MutEventSubscriber {
    fn as_mut_any(&mut self) -> &mut dyn Any;
}

impl GenericSubscriber for CounterSubscriber {
    fn as_mut_any(&mut self) -> &mut dyn Any {
        self
    }
}

// This test showcases the use of the three `RemoteEndpoint` methods: `call_blocking`, `fire`,
// and `kick`. The setting is we're moving an `EventManager` to a separate thread, and we want
// to register subscribers that remain fully owned by the manager (so, for example, it's not
// necessary to wrap them in a `Mutex` if event handling needs mutable access to their inner
// state). We also use the `GenericSubscriber` trait defined above to show how we can still
// obtain and interact with references to the actual types when event subscribers are registered
// as trait objects.
#[test]
fn test_endpoint() {
    let mut event_manager = EventManager::<Box<dyn GenericSubscriber + Send>>::new().unwrap();
    let endpoint = event_manager.remote_endpoint();
    let sub = Box::new(CounterSubscriber::new());

    let run_count = Arc::new(AtomicU64::new(0));
    let keep_running = Arc::new(AtomicBool::new(true));

    // These get moved into the following closure.
    let run_count_clone = run_count.clone();
    let keep_running_clone = keep_running.clone();

    // We spawn the thread that runs the event loop.
    let thread_handle = thread::spawn(move || {
        loop {
            // The manager gets moved to the new thread here.
            event_manager.run().unwrap();
            // Increment this counter every time the `run` method call above returns. Ordering
            // is not that important here because the other thread only reads the value.
            run_count_clone.fetch_add(1, Ordering::Relaxed);

            // When `keep_running` is no longer `true`, we break the loop and the
            // thread will exit.
            if !keep_running_clone.load(Ordering::Acquire) {
                break;
            }
        }
    });

    // We're back to the main program thread from now on.

    // `run_count` should not change for now as there's no possible activity for the manager.
    thread::sleep(Duration::from_millis(100));
    assert_eq!(run_count.load(Ordering::Relaxed), 0);

    // Use `call_blocking` to register a subscriber (and move it to the event loop thread under
    // the ownership of the manager). `call_block` also allows us to inspect the result of the
    // operation, but it blocks waiting for it and cannot be invoked from the same thread as
    // the event loop.
    let sub_id = endpoint
        .call_blocking(
            |sub_ops| -> result::Result<SubscriberId, event_manager::Error> {
                Ok(sub_ops.add_subscriber(sub))
            },
        )
        .unwrap();

    // We've added a subscriber. No subscriber events are fired yet, but the manager run
    // loop went through one iteration when the endpoint message was received, so `run_count`
    // has benn incremented.
    thread::sleep(Duration::from_millis(100));
    assert_eq!(run_count.load(Ordering::Relaxed), 1);

    // Now let's activate the subscriber event. It's going to generate continuous activity until
    // we explicitly clear it. We use `endpoint` to interact with the subscriber, because the
    // latter is fully owned by the manager. Also, we make use of the `as_mut_any` method
    // from our `GenericSubscriber` trait to get a reference to the actual subscriber type
    // (which has been erased as a trait object from the manager's perspective). We use `fire`
    // here, so we don't get a result.
    //
    // `fire` can also be used from the same thread as the `event_manager` runs on without causing
    // a deadlock, because it doesn't get a result from the closure. For example, we can pass an
    // endpoint to a subscriber, and use `fire` as part of `process` if it's helpful for that
    // particular use case.
    //
    // Not getting a result from the closure means we have to deal with error conditions within.
    // We use `unwrap` here, but that's ok because if the subscriber associated with `sub_id` is
    // not present, then we have a serious error in our program logic.
    endpoint
        .fire(move |sub_ops| {
            let sub = sub_ops.subscriber_mut(sub_id).unwrap();
            // The following `unwrap` cannot fail because we know the type is `CounterSubscriber`.
            sub.as_mut_any()
                .downcast_mut::<CounterSubscriber>()
                .unwrap()
                .trigger_event()
        })
        .unwrap();

    // The event will start triggering at this point, so `run_count` will increase.
    thread::sleep(Duration::from_millis(100));
    assert!(run_count.load(Ordering::Relaxed) > 1);

    // Let's clear the subscriber event. Using `fire` again.
    endpoint
        .fire(move |sub_ops| {
            let sub = sub_ops.subscriber_mut(sub_id).unwrap();
            // The following `unwrap` cannot fail because we know the actual type
            // is `CounterSubscriber`.
            sub.as_mut_any()
                .downcast_mut::<CounterSubscriber>()
                .unwrap()
                .clear_event()
        })
        .unwrap();

    // We wait a bit more. The manager will be once more become blocked waiting for events.
    thread::sleep(Duration::from_millis(100));

    keep_running.store(false, Ordering::Release);

    // Trying to `join` the manager here would lead to a deadlock, because it will never read
    // the value of `keep_running` to break the loop while stuck waiting for events. We use the
    // `kick` endpoint method to force `EventManager::run()` to return.

    endpoint.kick().unwrap();

    // We can `join` the manager thread and finalize now.
    thread_handle.join().unwrap();
}
