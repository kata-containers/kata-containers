// Copyright (C) 2020 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

use event_manager::utilities::subscribers::CounterSubscriberWithData;
use event_manager::{EventManager, SubscriberOps};

// Test for the race condition reported by: Karthik.n <karthik.n@zohocorp.com>
// FYI: https://github.com/rust-vmm/event-manager/issues/41
#[test]
fn test_reuse_file_descriptor() {
    let mut event_manager = EventManager::<CounterSubscriberWithData>::new().unwrap();
    let mut counter_subscriber = CounterSubscriberWithData::new(0);

    // Set flag to toggle the registration of all three fds on the first epoll event, so the final
    // event counter should be 1.
    counter_subscriber.set_toggle_registry(true);
    counter_subscriber.trigger_all_counters();
    let id = event_manager.add_subscriber(counter_subscriber);

    event_manager.run().unwrap();
    let c_ref = event_manager.subscriber_mut(id).unwrap();
    let counters = c_ref.get_all_counter_values();
    assert_eq!(counters[0] + counters[1] + counters[2], 1);
}
