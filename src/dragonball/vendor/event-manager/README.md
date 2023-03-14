# event-manager

The `event-manager` provides abstractions for implementing event based
systems. For now, this crate only works on Linux and uses the
[epoll](http://man7.org/linux/man-pages/man7/epoll.7.html) API to provide a
mechanism for handling I/O notifications.

## Design

This crate is build around two abstractions:
- Event Manager
- Event Subscriber

The subscriber defines and registers an interest list with the event manager.
The interest list represents the events that the subscriber wants to monitor.

The Event Manager allows adding and removing subscribers, and provides
APIs through which the subscribers can be updated in terms of events in their
interest list. These actions are abstracted through the `SubscriberOps` trait.

To interface with the Event Manager, the Event Subscribers need to provide an
initialization function, and a callback for when events in the
interest list become ready. The subscribers can update their interest list
when handling ready events. These actions are abstracted through the
`EventSubscriber` and `MutEventSubscriber` traits. They contain the same
methods, but the former only requires immutable `self` borrows, whereas the
latter requires mutable borrows. Any type implementing `EventSubscriber`
automatically implements `MutEventSubscriber` as well.

A typical event-based application creates the event manager, registers
subscribers, and then calls into the event manager's `run` function in a loop.
Behind the scenes, the event manager calls into `epoll::wait` and maps the file
descriptors in the ready list to the subscribers it manages. The event manager
calls the subscriber's `process` function (its registered callback). When
dispatching the events, the event manager creates a specialized object and
passes it in the callback function so that the subscribers can use it to alter
their interest list.

![](docs/event-manager.png)

Read more in the [design document](docs/DESIGN.md).

## Implementing an Event Subscriber

The event subscriber has full control over the events that it monitors.
The events need to be added to the event manager's loop as part of the
`init` function. Adding events to the loop can return errors, and it is
the responsibility of the subscriber to handle them.

Similarly, the event subscriber is in full control of the ready events.
When an event becomes ready, the event manager will call into the subscriber
`process` function. The subscriber SHOULD handle the following events which
are always returned when they occur (they don't need to be registered):
- `EventSet::ERROR` - an error occurred on the monitor file descriptor.
- `EventSet::HANG_UP` - hang up happened on the associated fd.
- `EventSet::READ_HANG_UP` - hang up when the registered event is edge
   triggered.

For more details about the error cases, you can check the
[`epoll_ctl documentation`](https://www.man7.org/linux/man-pages/man2/epoll_ctl.2.html).


## Initializing the Event Manager

The `EventManager` uses a generic type parameter which represents the
subscriber type. The crate provides automatic implementations of
`EventSubscriber` for `Arc<T>` and `Rc<T>` (for any `T: EventSubscriber +?Sized`),
together with automatic implementations of `MutEventSubscriber` for `Mutex<T>`
and `RefCell<T>` (for any `T: MutEventSubscriber + ?Sized`). The generic type
parameter enables either static or dynamic dispatch.

This crate has no default features. The optional `remote_endpoint`
feature enables interactions with the `EventManager` from different threads
without the need of more intrusive synchronization.

## Examples

For closer to real life use cases, please check the examples in
[tests](tests).

### Basic Single Thread Subscriber

#### Implementing a Basic Subscriber

```rust
use event_manager::{EventOps, Events, MutEventSubscriber};
use vmm_sys_util::{eventfd::EventFd, epoll::EventSet};

use std::os::unix::io::AsRawFd;
use std::fmt::{Display, Formatter, Result};

pub struct CounterSubscriber {
    event_fd: EventFd,
    counter: u64,
}

impl CounterSubscriber {
    pub fn new() -> Self {
        Self {
            event_fd: EventFd::new(0).unwrap(),
            counter: 0,
        }
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
                event_ops.remove(events).unwrap_or(
                    eprintln!("Encountered error during cleanup")
                );
                panic!("Cannot continue execution. Associated fd was closed.");
            }
            _ => {}
        }
    }

    fn init(&mut self, ops: &mut EventOps) {
        ops.add(Events::new(&self.event_fd, EventSet::IN)).expect("Cannot register event.");
    }
}
```

#### Adding Subscribers to the Event Manager

```rust
struct App {
    event_manager: EventManager<CounterSubscriber>,
    subscribers_id: Vec<SubscriberId>,
}

impl App {
    fn new() -> Self {
        Self {
            event_manager: EventManager::<CounterSubscriber>::new().unwrap(),
            subscribers_id: vec![]
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
}
```

## Development and Testing

The `event-manager` is tested using unit tests, Rust integration tests and
performance benchmarks. It leverages
[`rust-vmm-ci`](https://github.com/rust-vmm/rust-vmm-ci) for continuous
testing. All tests are run in the `rustvmm/dev` container.

More details on running the tests can be found in the
[development](docs/DEVELOPMENT.md) document.

## License

This project is licensed under either of:

- [Apache License](LICENSE-APACHE), Version 2.0
- [BSD-3-Clause License](LICENSE-BSD-3-CLAUSE)
