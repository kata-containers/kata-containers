# Event Manager Design

## Interest List Updates

Subscribers can update their interest list when the `EventManager` calls
their `process` function. The EventManager crates a specialized `EventOps`
object. `EventOps` limits the operations that the subscribers may call to the
ones that are related to the interest list as follows:
- Adding a new event that the subscriber is interested in.
- Modifying an existing event (for example: update an event to be
  edge-triggered instead of being level-triggered or update the user data
  associated with an event).
- Remove an existing event.

The subscriber is responsible for handling the errors returned from calling
`add`, `modify` or `remove`.

The `EventManager` knows how to associate these actions to a registered
subscriber because it adds the corresponding `SubscriberId` when it creates the
`EventOps` object.

## Events

By default, `Events` wrap a file descriptor, and a bit mask of events
(for example `EPOLLIN | EPOLLOUT`). The `Events` can optionally contain user
defined data.

The `Events` are used in `add`, `remove` and `modify` functions
in [`EventOps`](../src/events.rs). While their semantic is very similar to that
of `libc::epoll_event`, they come with an additional requirement. When
creating `Events` objects, the subscribers must specify the file descriptor
associated with the event mask. There are a few reasons behind this choice:
- Reducing the number of parameters on the `EventOps` functions. Instead of
  always passing the file descriptor along with an `epoll_event` object, the
  user only needs to pass `Events`.
- Backing the file descriptor in `Events` provides a simple mapping from a file
  descriptor to the subscriber that is watching events on that particular file
  descriptor.

Storing the file descriptor in all `Events` means that there are 32 bits left
for custom user data.
A file descriptor can be registered only once (it can be associated with only
one subscriber).

### Using Events With Custom Data

The 32-bits in custom data can be used to map events to internal callbacks
based on user-defined numeric values instead of file descriptors. In the
below example, the user defined values are consecutive so that the match
statement can be optimized to a jump table.

```rust
    struct Painter {}
    const PROCESS_GREEN:u32 = 0;
    const PROCESS_RED: u32 = 1;
    const PROCESS_BLUE: u32 = 2;

    impl Painter {
        fn process_green(&self, event: Events) {}
        fn process_red(&self, event: Events) {}
        fn process_blue(&self, events: Events) {}
    }

    impl MutEventSubscriber for Painter {
        fn init(&mut self, ops: &mut EventOps) {
            let green_eventfd = EventFd::new(0).unwrap();
            let ev_for_green = Events::with_data(&green_eventfd, PROCESS_GREEN, EventSet::IN);
            ops.add(ev_for_green).unwrap();

            let red_eventfd = EventFd::new(0).unwrap();
            let ev_for_red = Events::with_data(&red_eventfd, PROCESS_RED, EventSet::IN);
            ops.add(ev_for_red).unwrap();

            let blue_eventfd = EventFd::new(0).unwrap();
            let ev_for_blue = Events::with_data(&blue_eventfd, PROCESS_BLUE, EventSet::IN);
            ops.add(ev_for_blue).unwrap();
        }

        fn process(&mut self, events: Events, ops: &mut EventOps) {
            match events.data() {
                PROCESS_GREEN => self.process_green(events),
                PROCESS_RED => self.process_red(events),
                PROCESS_BLUE => self.process_blue(events),
                _ => error!("spurious event"),
            };
        }
    }
```

## Remote Endpoint

A manager remote endpoint allows users to interact with the `EventManger`
(as a `SubscriberOps` trait object) from a different thread of execution.
This is particularly useful when the `EventManager` owns the subscriber object
the user wants to interact with, and the communication happens from a separate
thread. This functionality is gated behind the `remote_endpoint` feature.

The current implementation relies on passing boxed closures to the manager and
getting back a boxed result. The manager is notified about incoming invocation
requests via an [`EventFd`](https://docs.rs/vmm-sys-util/latest/vmm_sys_util/eventfd/struct.EventFd.html)
which is added by the manager to its internal run loop. The manager runs each
closure to completion, and then returns the boxed result using a sender object
that is part of the initial message that also included the closure. The
following example uses the previously defined `Painter` subscriber type.

```rust
fn main() {
    // Create an event manager object.
    let mut event_manager = EventManager::<Painter>::new().unwrap();

    // Obtain a remote endpoint object.
    let endpoint = event_manager.remote_endpoint();

    // Move the event manager to a new thread and start running the event loop there.
    let thread_handle = thread::spawn(move || loop {
        event_manager.run().unwrap();            
    });

    let subscriber = Painter {};

    // Add the subscriber using the remote endpoint. The subscriber is moved to the event
    // manager thread, and is now owned by the manager. In return, we get the subscriber id,
    // which can be used to identify the subscriber for subsequent operations.
    let id = endpoint
        .call_blocking(move |sub_ops| -> Result<SubscriberId> {
            Ok(sub_ops.add_subscriber(subscriber))
        })
        .unwrap();
    // ...

    // Add a new event to the subscriber, using fd 1 as an example.
    let events = Events::new_raw(1, EventSet::OUT);
    endpoint
        .call_blocking(move |sub_ops| -> Result<()> { sub_ops.event_ops(id)?.add(events) })
        .unwrap();

    // ...

    thread_handle.join();
}
```

The `call_blocking` invocation sends a message over a channel to the event manager on the
other thread, and then blocks until a response is received. The event manager detects the
presence of such messages as with any other event, and handles them as part of the event
loop. This can lead to deadlocks if, for example, `call_blocking` is invoked in the `process`
implmentation of a subscriber to the same event manager.