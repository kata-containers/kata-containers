//! An atomically managed intrusive linked list of `Arc` nodes

use std::marker;
use std::ops::Deref;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::Arc;

pub struct ArcList<T> {
    list: AtomicUsize,
    _marker: marker::PhantomData<T>,
}

impl<T> ArcList<T> {
    pub fn new() -> ArcList<T> {
        ArcList {
            list: AtomicUsize::new(0),
            _marker: marker::PhantomData,
        }
    }

    /// Pushes the `data` provided onto this list if it's not already enqueued
    /// in this list.
    ///
    /// If `data` is already enqueued in this list then this is a noop,
    /// otherwise, the `data` here is pushed on the end of the list.
    pub fn push(&self, data: &Arc<Node<T>>) -> Result<(), ()> {
        if data.enqueued.swap(true, SeqCst) {
            // note that even if our list is sealed off then the other end is
            // still guaranteed to see us because we were previously enqueued.
            return Ok(());
        }
        let mut head = self.list.load(SeqCst);
        let node = Arc::into_raw(data.clone()) as usize;
        loop {
            // If we've been sealed off, abort and return an error
            if head == 1 {
                unsafe {
                    drop(Arc::from_raw(node as *mut Node<T>));
                }
                return Err(());
            }

            // Otherwise attempt to push this node
            data.next.store(head, SeqCst);
            match self.list.compare_exchange(head, node, SeqCst, SeqCst) {
                Ok(_) => break Ok(()),
                Err(new_head) => head = new_head,
            }
        }
    }

    /// Atomically empties this list, returning a new owned copy which can be
    /// used to iterate over the entries.
    pub fn take(&self) -> ArcList<T> {
        let mut list = self.list.load(SeqCst);
        loop {
            if list == 1 {
                break;
            }
            match self.list.compare_exchange(list, 0, SeqCst, SeqCst) {
                Ok(_) => break,
                Err(l) => list = l,
            }
        }
        ArcList {
            list: AtomicUsize::new(list),
            _marker: marker::PhantomData,
        }
    }

    /// Atomically empties this list and prevents further successful calls to
    /// `push`.
    pub fn take_and_seal(&self) -> ArcList<T> {
        ArcList {
            list: AtomicUsize::new(self.list.swap(1, SeqCst)),
            _marker: marker::PhantomData,
        }
    }

    /// Removes the head of the list of nodes, returning `None` if this is an
    /// empty list.
    pub fn pop(&mut self) -> Option<Arc<Node<T>>> {
        let head = *self.list.get_mut();
        if head == 0 || head == 1 {
            return None;
        }
        let head = unsafe { Arc::from_raw(head as *const Node<T>) };
        *self.list.get_mut() = head.next.load(SeqCst);
        // At this point, the node is out of the list, so store `false` so we
        // can enqueue it again and see further changes.
        assert!(head.enqueued.swap(false, SeqCst));
        Some(head)
    }
}

impl<T> Drop for ArcList<T> {
    fn drop(&mut self) {
        while let Some(_) = self.pop() {
            // ...
        }
    }
}

pub struct Node<T> {
    next: AtomicUsize,
    enqueued: AtomicBool,
    data: T,
}

impl<T> Node<T> {
    pub fn new(data: T) -> Node<T> {
        Node {
            next: AtomicUsize::new(0),
            enqueued: AtomicBool::new(false),
            data,
        }
    }
}

impl<T> Deref for Node<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        let a = ArcList::new();
        let n = Arc::new(Node::new(1));
        assert!(a.push(&n).is_ok());

        let mut l = a.take();
        assert_eq!(**l.pop().unwrap(), 1);
        assert!(l.pop().is_none());
    }

    #[test]
    fn seal() {
        let a = ArcList::new();
        let n = Arc::new(Node::new(1));
        let mut l = a.take_and_seal();
        assert!(l.pop().is_none());
        assert!(a.push(&n).is_err());

        assert!(a.take().pop().is_none());
        assert!(a.take_and_seal().pop().is_none());
    }
}
