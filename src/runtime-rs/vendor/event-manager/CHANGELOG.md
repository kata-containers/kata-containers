# v0.2.1

## Changed

- Updated the vmm-sys-util dependency to v0.8.0.

## Fixed

- Fixed `RemoteEndpoint` `Clone` implementation.
- Check the maximum capacity when calling `EventManager::new`.

# v0.2.0

## Fixed

- Fixed a race condition that might lead to wrongfully call the dispatch
  function for an inactive event
  ([[#41]](https://github.com/rust-vmm/event-manager/issues/41)).

## Added

- By default, the event manager can dispatch 256 events at one time. This limit
  can now be increased by using the `new_with_capacity` constructor
  ([[#37]](https://github.com/rust-vmm/event-manager/issues/37)).

# v0.1.0

This is the first release of event-manager.
The event-manager provides abstractions for implementing event based systems.
For now, this crate only works on Linux and uses the epoll API to provide a
mechanism for handling I/O notifications.
