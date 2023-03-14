# v0.4.0

## Fixed
- [[#173]](https://github.com/rust-vmm/vm-virtio/pull/173) Fix potential division by zero in
  iterator when the queue size is 0.

## Changed
- [[#162]](https://github.com/rust-vmm/vm-virtio/pull/162) Added error handling in the mock
  interface and the ability to create multiple descriptor chains for testing in order to
  support running fuzzing.
- [[#174]](https://github.com/rust-vmm/vm-virtio/pull/174) Updated the `avail_idx` and `used_idx`
  documentation to specify when these functions panic.


# v0.3.0

## Added
- [[#148]](https://github.com/rust-vmm/vm-virtio/pull/148): `QueueStateOwnedT` trait that stands
  for queue objects which are exclusively owned and accessed by a single thread of execution.
- [[#148]](https://github.com/rust-vmm/vm-virtio/pull/148): Added the `pop_descriptor_chain`
  method, which can be used to consume descriptor chains from the available ring without
  using an iterator, to `QueueStateT` and `QueueGuard`. Also added `go_to_previous_position()`
  to `QueueGuard`, which enables decrementing the next available index by one position, which
  effectively undoes the consumption of a descriptor chain in some use cases.
- [[#151]](https://github.com/rust-vmm/vm-virtio/pull/151): Added `MockSplitQueue::add_desc_chain()`,
  which places a descriptor chain at the specified offset in the descriptor table.  
- [[#153]](https://github.com/rust-vmm/vm-virtio/pull/153): Added `QueueStateT::size()` to return
  the size of the queue.

## Changed
- The minimum version of the `vm-memory` dependency is now `v0.8.0`
- [[#161]](https://github.com/rust-vmm/vm-virtio/pull/161): Improve the efficiency of `needs_notification`

## Removed
- [[#153]](https://github.com/rust-vmm/vm-virtio/pull/153): `#[derive(Clone)]` for `QueueState`

# v0.2.0

## Added

- *Testing Interface*: Added the possibility to initialize a mock descriptor
  chain from a list of descriptors.
- Added setters and getters for the queue fields required for extending the
  `Queue` in VMMs.

## Fixed

- Apply the appropriate endianness conversion on `used_idx`.

# v0.1.0

This is the first release of the crate.
