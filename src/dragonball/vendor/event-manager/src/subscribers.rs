// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

use super::SubscriberId;
use std::collections::HashMap;

// Internal structure used to keep the set of subscribers registered with an EventManger.
// This structure is a thin wrapper over a `HashMap` in which the keys are uniquely
// generated when calling `add`.
pub(crate) struct Subscribers<T> {
    // The key is the unique id of the subscriber and the entry is the `Subscriber`.
    subscribers: HashMap<SubscriberId, T>,
    // We are generating the unique ids by incrementing this counter for each added subscriber,
    // and rely on the large value range of u64 to ensure each value is effectively
    // unique over the runtime of any VMM.
    next_id: u64,
}

impl<T> Subscribers<T> {
    pub(crate) fn new() -> Self {
        Subscribers {
            subscribers: HashMap::new(),
            next_id: 1,
        }
    }

    // Adds a subscriber and generates an unique id to represent it.
    pub(crate) fn add(&mut self, subscriber: T) -> SubscriberId {
        let id = SubscriberId(self.next_id);
        self.next_id += 1;

        self.subscribers.insert(id, subscriber);

        id
    }

    // Remove and return the subscriber associated with the given id, if it exists.
    pub(crate) fn remove(&mut self, subscriber_id: SubscriberId) -> Option<T> {
        self.subscribers.remove(&subscriber_id)
    }

    // Checks whether a subscriber with `subscriber_id` is registered.
    pub(crate) fn contains(&mut self, subscriber_id: SubscriberId) -> bool {
        self.subscribers.contains_key(&subscriber_id)
    }

    // Return a mutable reference to the subriber represented by `subscriber_id`.
    //
    // This method should only be called for indices that are known to be valid, otherwise
    // panics can occur.
    pub(crate) fn get_mut_unchecked(&mut self, subscriber_id: SubscriberId) -> &mut T {
        self.subscribers.get_mut(&subscriber_id).unwrap()
    }
}
