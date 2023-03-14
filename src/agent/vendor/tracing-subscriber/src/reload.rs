//! Wrapper for a `Layer` to allow it to be dynamically reloaded.
//!
//! This module provides a [`Layer` type] which wraps another type implementing
//! the [`Layer` trait], allowing the wrapped type to be replaced with another
//! instance of that type at runtime.
//!
//! This can be used in cases where a subset of `Subscriber` functionality
//! should be dynamically reconfigured, such as when filtering directives may
//! change at runtime. Note that this layer introduces a (relatively small)
//! amount of overhead, and should thus only be used as needed.
//!
//! [`Layer` type]: struct.Layer.html
//! [`Layer` trait]: ../layer/trait.Layer.html
use crate::layer;
use crate::sync::RwLock;

use std::{
    error, fmt,
    marker::PhantomData,
    sync::{Arc, Weak},
};
use tracing_core::{
    callsite, span,
    subscriber::{Interest, Subscriber},
    Event, Metadata,
};

/// Wraps a `Layer`, allowing it to be reloaded dynamically at runtime.
#[derive(Debug)]
pub struct Layer<L, S> {
    // TODO(eliza): this once used a `crossbeam_util::ShardedRwLock`. We may
    // eventually wish to replace it with a sharded lock implementation on top
    // of our internal `RwLock` wrapper type. If possible, we should profile
    // this first to determine if it's necessary.
    inner: Arc<RwLock<L>>,
    _s: PhantomData<fn(S)>,
}

/// Allows reloading the state of an associated `Layer`.
#[derive(Debug)]
pub struct Handle<L, S> {
    inner: Weak<RwLock<L>>,
    _s: PhantomData<fn(S)>,
}

/// Indicates that an error occurred when reloading a layer.
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
}

#[derive(Debug)]
enum ErrorKind {
    SubscriberGone,
    Poisoned,
}

// ===== impl Layer =====

impl<L, S> crate::Layer<S> for Layer<L, S>
where
    L: crate::Layer<S> + 'static,
    S: Subscriber,
{
    fn on_layer(&mut self, subscriber: &mut S) {
        try_lock!(self.inner.write(), else return).on_layer(subscriber);
    }

    #[inline]
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        try_lock!(self.inner.read(), else return Interest::sometimes()).register_callsite(metadata)
    }

    #[inline]
    fn enabled(&self, metadata: &Metadata<'_>, ctx: layer::Context<'_, S>) -> bool {
        try_lock!(self.inner.read(), else return false).enabled(metadata, ctx)
    }

    #[inline]
    fn new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).new_span(attrs, id, ctx)
    }

    #[inline]
    fn on_record(&self, span: &span::Id, values: &span::Record<'_>, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_record(span, values, ctx)
    }

    #[inline]
    fn on_follows_from(&self, span: &span::Id, follows: &span::Id, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_follows_from(span, follows, ctx)
    }

    #[inline]
    fn on_event(&self, event: &Event<'_>, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_event(event, ctx)
    }

    #[inline]
    fn on_enter(&self, id: &span::Id, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_enter(id, ctx)
    }

    #[inline]
    fn on_exit(&self, id: &span::Id, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_exit(id, ctx)
    }

    #[inline]
    fn on_close(&self, id: span::Id, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_close(id, ctx)
    }

    #[inline]
    fn on_id_change(&self, old: &span::Id, new: &span::Id, ctx: layer::Context<'_, S>) {
        try_lock!(self.inner.read()).on_id_change(old, new, ctx)
    }
}

impl<L, S> Layer<L, S>
where
    L: crate::Layer<S> + 'static,
    S: Subscriber,
{
    /// Wraps the given `Layer`, returning a `Layer` and a `Handle` that allows
    /// the inner type to be modified at runtime.
    pub fn new(inner: L) -> (Self, Handle<L, S>) {
        let this = Self {
            inner: Arc::new(RwLock::new(inner)),
            _s: PhantomData,
        };
        let handle = this.handle();
        (this, handle)
    }

    /// Returns a `Handle` that can be used to reload the wrapped `Layer`.
    pub fn handle(&self) -> Handle<L, S> {
        Handle {
            inner: Arc::downgrade(&self.inner),
            _s: PhantomData,
        }
    }
}

// ===== impl Handle =====

impl<L, S> Handle<L, S>
where
    L: crate::Layer<S> + 'static,
    S: Subscriber,
{
    /// Replace the current layer with the provided `new_layer`.
    pub fn reload(&self, new_layer: impl Into<L>) -> Result<(), Error> {
        self.modify(|layer| {
            *layer = new_layer.into();
        })
    }

    /// Invokes a closure with a mutable reference to the current layer,
    /// allowing it to be modified in place.
    pub fn modify(&self, f: impl FnOnce(&mut L)) -> Result<(), Error> {
        let inner = self.inner.upgrade().ok_or(Error {
            kind: ErrorKind::SubscriberGone,
        })?;

        let mut lock = try_lock!(inner.write(), else return Err(Error::poisoned()));
        f(&mut *lock);
        // Release the lock before rebuilding the interest cache, as that
        // function will lock the new layer.
        drop(lock);

        callsite::rebuild_interest_cache();
        Ok(())
    }

    /// Returns a clone of the layer's current value if it still exists.
    /// Otherwise, if the subscriber has been dropped, returns `None`.
    pub fn clone_current(&self) -> Option<L>
    where
        L: Clone,
    {
        self.with_current(L::clone).ok()
    }

    /// Invokes a closure with a borrowed reference to the current layer,
    /// returning the result (or an error if the subscriber no longer exists).
    pub fn with_current<T>(&self, f: impl FnOnce(&L) -> T) -> Result<T, Error> {
        let inner = self.inner.upgrade().ok_or(Error {
            kind: ErrorKind::SubscriberGone,
        })?;
        let inner = try_lock!(inner.read(), else return Err(Error::poisoned()));
        Ok(f(&*inner))
    }
}

impl<L, S> Clone for Handle<L, S> {
    fn clone(&self) -> Self {
        Handle {
            inner: self.inner.clone(),
            _s: PhantomData,
        }
    }
}

// ===== impl Error =====

impl Error {
    fn poisoned() -> Self {
        Self {
            kind: ErrorKind::Poisoned,
        }
    }

    /// Returns `true` if this error occurred because the layer was poisoned by
    /// a panic on another thread.
    pub fn is_poisoned(&self) -> bool {
        matches!(self.kind, ErrorKind::Poisoned)
    }

    /// Returns `true` if this error occurred because the `Subscriber`
    /// containing the reloadable layer was dropped.
    pub fn is_dropped(&self) -> bool {
        matches!(self.kind, ErrorKind::SubscriberGone)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self.kind {
            ErrorKind::SubscriberGone => "subscriber no longer exists",
            ErrorKind::Poisoned => "lock poisoned",
        };
        f.pad(msg)
    }
}

impl error::Error for Error {}
