// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! A manager remote endpoint allows user to interact with the `EventManger` (as a `SubscriberOps`
//! trait object) from a different thread of execution.
//!
//! This is particularly useful when the `EventManager` owns (via `EventManager::add_subscriber`)
//! a subscriber object the user needs to work with (via `EventManager::subscriber_mut`), but the
//! `EventManager` being on a different thread requires synchronized handles.
//!
//! Until more sophisticated methods are explored (for example making the `EventManager` offer
//! interior mutability using something like an RCU mechanism), the current approach relies on
//! passing boxed closures to the manager and getting back a boxed result. The manager is notified
//! about incoming invocation requests via an `EventFd` which is added to the epoll event set.
//! The signature of the closures as they are received is the `FnOnceBox` type alias defined
//! below. The actual return type is opaque to the manager, but known to the initiator. The manager
//! runs each closure to completion, and then returns the boxed result using a sender object that
//! is part of the initial message that also included the closure.

use std::any::Any;
use std::os::unix::io::{AsRawFd, RawFd};
use std::result;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use vmm_sys_util::eventfd::{EventFd, EFD_NONBLOCK};

use super::{Errno, Error, MutEventSubscriber, Result, SubscriberOps};

// The return type of the closure received by the manager is erased (by placing it into a
// `Box<dyn Any + Send>` in order to have a single concrete type definition for the messages that
// contain closures to be executed (the `FnMsg` defined below). The actual return type is
// recovered by the initiator of the remote call (more details in the implementation of
// `RemoteEndpoint::call_blocking` below). The `Send` bound is required to send back the boxed
// result over the return channel.
type ErasedResult = Box<dyn Any + Send>;

// Type alias for the boxed closures received by the manager. The `Send` bound at the end applies
// to the type of the closure, and is required to send the box over the channel.
type FnOnceBox<S> = Box<dyn FnOnce(&mut dyn SubscriberOps<Subscriber = S>) -> ErasedResult + Send>;

// The type of the messages received by the manager over its receive mpsc channel.
pub(crate) struct FnMsg<S> {
    // The closure to execute.
    pub(crate) fnbox: FnOnceBox<S>,
    // The sending endpoint of the channel used by the remote called to wait for the result.
    pub(crate) sender: Option<Sender<ErasedResult>>,
}

// Used by the `EventManager` to keep state associated with the channel.
pub(crate) struct EventManagerChannel<S> {
    // A clone of this is given to every `RemoteEndpoint` and used to signal the presence of
    // an new message on the channel.
    pub(crate) event_fd: Arc<EventFd>,
    // A clone of this sender is given to every `RemoteEndpoint` and used to send `FnMsg` objects
    // to the `EventManager` over the channel.
    pub(crate) sender: Sender<FnMsg<S>>,
    // The receiving half of the channel, used to receive incoming `FnMsg` objects.
    pub(crate) receiver: Receiver<FnMsg<S>>,
}

impl<S> EventManagerChannel<S> {
    pub(crate) fn new() -> Result<Self> {
        let (sender, receiver) = channel();
        Ok(EventManagerChannel {
            event_fd: Arc::new(
                EventFd::new(EFD_NONBLOCK).map_err(|e| Error::EventFd(Errno::from(e)))?,
            ),
            sender,
            receiver,
        })
    }

    pub(crate) fn fd(&self) -> RawFd {
        self.event_fd.as_raw_fd()
    }

    pub(crate) fn remote_endpoint(&self) -> RemoteEndpoint<S> {
        RemoteEndpoint {
            msg_sender: self.sender.clone(),
            event_fd: self.event_fd.clone(),
        }
    }
}

/// Enables interactions with an `EventManager` that runs on a different thread of execution.
pub struct RemoteEndpoint<S> {
    // A sender associated with `EventManager` channel requests are sent over.
    msg_sender: Sender<FnMsg<S>>,
    // Used to notify the `EventManager` about the arrival of a new request.
    event_fd: Arc<EventFd>,
}

impl<S> Clone for RemoteEndpoint<S> {
    fn clone(&self) -> Self {
        RemoteEndpoint {
            msg_sender: self.msg_sender.clone(),
            event_fd: self.event_fd.clone(),
        }
    }
}

impl<S: MutEventSubscriber> RemoteEndpoint<S> {
    // Send a message to the remote EventManger and raise a notification.
    fn send(&self, msg: FnMsg<S>) -> Result<()> {
        self.msg_sender.send(msg).map_err(|_| Error::ChannelSend)?;
        self.event_fd
            .write(1)
            .map_err(|e| Error::EventFd(Errno::from(e)))?;
        Ok(())
    }

    /// Call the specified closure on the associated remote `EventManager` (provided as a
    /// `SubscriberOps` trait object), and return the result. This method blocks until the result
    /// is received, and calling it from the same thread where the event loop runs leads to
    /// a deadlock.
    pub fn call_blocking<F, O, E>(&self, f: F) -> result::Result<O, E>
    where
        F: FnOnce(&mut dyn SubscriberOps<Subscriber = S>) -> result::Result<O, E> + Send + 'static,
        O: Send + 'static,
        E: From<Error> + Send + 'static,
    {
        // Create a temporary channel used to get back the result. We keep the receiving end,
        // and put the sending end into the message we pass to the remote `EventManager`.
        let (sender, receiver) = channel();

        // We erase the return type of `f` by moving and calling it inside another closure which
        // hides the result as an `ErasedResult`. This allows using the same channel to send
        // closures with different signatures (and thus different types) to the remote
        // `EventManager`.
        let fnbox = Box::new(
            move |ops: &mut dyn SubscriberOps<Subscriber = S>| -> ErasedResult { Box::new(f(ops)) },
        );

        // Send the message requesting the closure invocation.
        self.send(FnMsg {
            fnbox,
            sender: Some(sender),
        })?;

        // Block until a response is received. We can use unwrap because the downcast cannot fail,
        // since the signature of F (more specifically, the return value) constrains the concrete
        // type that's in the box.
        let result_box = receiver
            .recv()
            .map_err(|_| Error::ChannelRecv)?
            .downcast()
            .unwrap();

        // Turns out the dereference operator has a special behaviour for boxed objects; if we
        // own a `b: Box<T>` and call `*b`, the box goes away and we get the `T` inside.
        *result_box
    }

    /// Call the specified closure on the associated local/remote `EventManager` (provided as a
    /// `SubscriberOps` trait object), and discard the result. This method only fires
    /// the request but does not wait for result, so it may be called from the same thread where
    /// the event loop runs.
    pub fn fire<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce(&mut dyn SubscriberOps<Subscriber = S>) + Send + 'static,
    {
        // We erase the return type of `f` by moving and calling it inside another closure which
        // hides the result as an `ErasedResult`. This allows using the same channel send closures
        // with different signatures (and thus different types) to the remote `EventManager`.
        let fnbox = Box::new(
            move |ops: &mut dyn SubscriberOps<Subscriber = S>| -> ErasedResult {
                f(ops);
                Box::new(())
            },
        );

        // Send the message requesting the closure invocation.
        self.send(FnMsg {
            fnbox,
            sender: None,
        })
    }

    /// Kick the worker thread to wake up from the epoll event loop.
    pub fn kick(&self) -> Result<()> {
        self.event_fd
            .write(1)
            .map(|_| ())
            .map_err(|e| Error::EventFd(Errno::from(e)))
    }
}
