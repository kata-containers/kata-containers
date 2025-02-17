// Copyright (C) 2019 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! An interval tree implementation specialized for VMM resource management.
//!
//! It's not designed as a generic interval tree, but specialized for VMM resource management.
//! In addition to the normal get()/insert()/delete()/update() tree operations, it also implements
//! allocate()/free() for resource allocation.
//!
//! # Examples
//! ```rust
//! extern crate dbs_allocator;
//! use dbs_allocator::{Constraint, IntervalTree, NodeState, Range};
//!
//! // Create an interval tree and add available resources.
//! let mut tree = IntervalTree::<u64>::new();
//! tree.insert(Range::new(0x100u32, 0x100u32), None);
//! tree.insert(Range::new(0x200u16, 0x2ffu16), None);
//!
//! // Allocate a range with constraints.
//! let mut constraint = Constraint::new(8u64);
//! constraint.min = 0x211;
//! constraint.max = 0x21f;
//! constraint.align = 0x8;
//!
//! let key = tree.allocate(&constraint);
//! assert_eq!(key, Some(Range::new(0x218u64, 0x21fu64)));
//! let val = tree.get(&Range::new(0x218u64, 0x21fu64));
//! assert_eq!(val, Some(NodeState::Allocated));
//!
//! // Associate data with the allocated range and mark the range as occupied.
//! // Note: caller needs to protect from concurrent access between allocate() and the first call
//! // to update() to mark range as occupied.
//! let old = tree.update(&Range::new(0x218u32, 0x21fu32), 2);
//! assert_eq!(old, None);
//! let old = tree.update(&Range::new(0x218u32, 0x21fu32), 3);
//! assert_eq!(old, Some(2));
//! let val = tree.get(&Range::new(0x218u32, 0x21fu32));
//! assert_eq!(val, Some(NodeState::Valued(&3)));
//!
//! // Free allocated resource.
//! let old = tree.free(key.as_ref().unwrap());
//! assert_eq!(old, Some(3));
//! ```

use std::cmp::{max, min, Ordering};

use crate::{AllocPolicy, Constraint};

/// Represent a closed range `[min, max]`.
#[allow(missing_docs)]
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Range {
    pub min: u64,
    pub max: u64,
}

impl std::fmt::Debug for Range {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[ {:016x}, {:016x} ]", self.min, self.max)
    }
}

impl Range {
    /// Create a instance of [`Range`] with given `min` and `max`.
    ///
    /// ## Panic
    /// - if min is bigger than max
    /// - if min == 0 && max == u64:MAX
    pub fn new<T>(min: T, max: T) -> Self
    where
        u64: From<T>,
    {
        let umin = u64::from(min);
        let umax = u64::from(max);
        if umin > umax || (umin == 0 && umax == u64::MAX) {
            panic!("interval_tree: Range({}, {}) is invalid", umin, umax);
        }
        Range {
            min: umin,
            max: umax,
        }
    }

    /// Create a instance of [`Range`] with given base and size.
    ///
    /// ## Panic
    /// - if base + size wraps around
    /// - if base == 0 && size == u64::MAX
    pub fn with_size<T>(base: T, size: T) -> Self
    where
        u64: From<T>,
    {
        let umin = u64::from(base);
        let umax = u64::from(size).checked_add(umin).unwrap();
        if umin > umax || (umin == 0 && umax == std::u64::MAX) {
            panic!("interval_tree: Range({}, {}) is invalid", umin, umax);
        }
        Range {
            min: umin,
            max: umax,
        }
    }

    /// Create a instance of [`Range`] containing only the point `value`.
    pub fn new_point<T>(value: T) -> Self
    where
        u64: From<T>,
    {
        let val = u64::from(value);
        Range { min: val, max: val }
    }

    /// Get size of the range.
    pub fn len(&self) -> u64 {
        self.max - self.min + 1
    }

    /// Check whether the range is empty.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Check whether two Range objects intersect with each other.
    pub fn intersect(&self, other: &Range) -> bool {
        max(self.min, other.min) <= min(self.max, other.max)
    }

    /// Check whether another [Range] object is fully covered by this range.
    pub fn contain(&self, other: &Range) -> bool {
        self.min <= other.min && self.max >= other.max
    }

    /// Create a new instance of [Range] with `min` aligned to `align`.
    ///
    /// # Examples
    /// ```rust
    /// extern crate dbs_allocator;
    /// use dbs_allocator::Range;
    ///
    /// let a = Range::new(2u32, 6u32);
    /// assert_eq!(a.align_to(0), Some(Range::new(2u32, 6u32)));
    /// assert_eq!(a.align_to(1), Some(Range::new(2u16, 6u16)));
    /// assert_eq!(a.align_to(2), Some(Range::new(2u64, 6u64)));
    /// assert_eq!(a.align_to(4), Some(Range::new(4u8, 6u8)));
    /// assert_eq!(a.align_to(8), None);
    /// assert_eq!(a.align_to(3), None);
    /// let b = Range::new(2u8, 2u8);
    /// assert_eq!(b.align_to(2), Some(Range::new(2u8, 2u8)));
    /// ```
    pub fn align_to(&self, align: u64) -> Option<Range> {
        match align {
            0 | 1 => Some(*self),
            _ => {
                if align & (align - 1) != 0 {
                    return None;
                }
                if let Some(min) = self.min.checked_add(align - 1).map(|v| v & !(align - 1)) {
                    if min <= self.max {
                        return Some(Range::new(min, self.max));
                    }
                }
                None
            }
        }
    }
}

impl Ord for Range {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.min.cmp(&other.min) {
            Ordering::Equal => self.max.cmp(&other.max),
            res => res,
        }
    }
}

impl PartialOrd for Range {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// State of interval tree node.
///
/// Valid state transitions:
/// - None -> Free: [IntervalTree::insert()]
/// - None -> Valued: [IntervalTree::insert()]
/// - Free -> Allocated: [IntervalTree::allocate()]
/// - Allocated -> Valued(T): [IntervalTree::update()]
/// - Valued -> Valued(T): [IntervalTree::update()]
/// - Allocated -> Free: [IntervalTree::free()]
/// - Valued(T) -> Free: [IntervalTree::free()]
/// - * -> None: [IntervalTree::delete()]
#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub enum NodeState<T> {
    /// Node is free
    Free,
    /// Node is allocated but without associated data
    Allocated,
    /// Node is allocated with associated data.
    Valued(T),
}

impl<T> NodeState<T> {
    fn take(&mut self) -> Self {
        std::mem::replace(self, NodeState::<T>::Free)
    }

    fn replace(&mut self, value: NodeState<T>) -> Self {
        std::mem::replace(self, value)
    }

    fn as_ref(&self) -> NodeState<&T> {
        match self {
            NodeState::<T>::Valued(ref x) => NodeState::<&T>::Valued(x),
            NodeState::<T>::Allocated => NodeState::<&T>::Allocated,
            NodeState::<T>::Free => NodeState::<&T>::Free,
        }
    }

    fn as_mut(&mut self) -> NodeState<&mut T> {
        match self {
            NodeState::<T>::Valued(ref mut x) => NodeState::<&mut T>::Valued(x),
            NodeState::<T>::Allocated => NodeState::<&mut T>::Allocated,
            NodeState::<T>::Free => NodeState::<&mut T>::Free,
        }
    }

    fn is_free(&self) -> bool {
        matches!(self, NodeState::<T>::Free)
    }
}

impl<T> From<NodeState<T>> for Option<T> {
    fn from(n: NodeState<T>) -> Option<T> {
        match n {
            NodeState::<T>::Free | NodeState::<T>::Allocated => None,
            NodeState::<T>::Valued(data) => Some(data),
        }
    }
}

/// Internal tree node to implement interval tree.
#[derive(Debug, PartialEq, Eq)]
struct InnerNode<T> {
    /// Interval handled by this node.
    key: Range,
    /// Optional contained data, None if the node is free.
    data: NodeState<T>,
    /// Optional left child of current node.
    left: Option<Node<T>>,
    /// Optional right child of current node.
    right: Option<Node<T>>,
    /// Cached height of the node.
    height: u32,
    /// Cached maximum valued covered by this node.
    max_key: u64,
}

impl<T> InnerNode<T> {
    fn new(key: Range, data: NodeState<T>) -> Self {
        InnerNode {
            key,
            data,
            left: None,
            right: None,
            height: 1,
            max_key: key.max,
        }
    }
}

/// Newtype for interval tree nodes.
#[derive(Debug, PartialEq, Eq)]
struct Node<T>(Box<InnerNode<T>>);

impl<T> Node<T> {
    fn new(key: Range, data: Option<T>) -> Self {
        let value = if let Some(t) = data {
            NodeState::Valued(t)
        } else {
            NodeState::Free
        };
        Node(Box::new(InnerNode::new(key, value)))
    }

    /// Returns a readonly reference to the node associated with the `key` or None if not found.
    fn search(&self, key: &Range) -> Option<&Self> {
        match self.0.key.cmp(key) {
            Ordering::Equal => Some(self),
            Ordering::Less => self.0.right.as_ref().and_then(|node| node.search(key)),
            Ordering::Greater => self.0.left.as_ref().and_then(|node| node.search(key)),
        }
    }

    /// Returns a shared reference to the node covers full range of the `key`.
    fn search_superset(&self, key: &Range) -> Option<&Self> {
        if self.0.key.contain(key) {
            Some(self)
        } else if key.max < self.0.key.min && self.0.left.is_some() {
            // Safe to unwrap() because we have just checked it.
            self.0.left.as_ref().unwrap().search_superset(key)
        } else if key.min > self.0.key.max && self.0.right.is_some() {
            // Safe to unwrap() because we have just checked it.
            self.0.right.as_ref().unwrap().search_superset(key)
        } else {
            None
        }
    }

    /// Returns a mutable reference to the node covers full range of the `key`.
    fn search_superset_mut(&mut self, key: &Range) -> Option<&mut Self> {
        if self.0.key.contain(key) {
            Some(self)
        } else if key.max < self.0.key.min && self.0.left.is_some() {
            // Safe to unwrap() because we have just checked it.
            self.0.left.as_mut().unwrap().search_superset_mut(key)
        } else if key.min > self.0.key.max && self.0.right.is_some() {
            // Safe to unwrap() because we have just checked it.
            self.0.right.as_mut().unwrap().search_superset_mut(key)
        } else {
            None
        }
    }

    /// Insert a new (key, data) pair into the subtree.
    ///
    /// Note: it will panic if the new key intersects with existing nodes.
    fn insert(mut self, key: Range, data: Option<T>) -> Self {
        match self.0.key.cmp(&key) {
            Ordering::Equal => {
                panic!("interval_tree: key {:?} exists", key);
            }
            Ordering::Less => {
                if self.0.key.intersect(&key) {
                    panic!(
                        "interval_tree: key {:?} intersects with existing {:?}",
                        key, self.0.key
                    );
                }
                match self.0.right {
                    None => self.0.right = Some(Node::new(key, data)),
                    Some(_) => self.0.right = self.0.right.take().map(|n| n.insert(key, data)),
                }
            }
            Ordering::Greater => {
                if self.0.key.intersect(&key) {
                    panic!(
                        "interval_tree: key {:?} intersects with existing {:?}",
                        key, self.0.key
                    );
                }
                match self.0.left {
                    None => self.0.left = Some(Node::new(key, data)),
                    Some(_) => self.0.left = self.0.left.take().map(|n| n.insert(key, data)),
                }
            }
        }
        self.updated_node()
    }

    /// Update an existing entry and return the old value.
    fn update(&mut self, key: &Range, data: NodeState<T>) -> Option<T> {
        match self.0.key.cmp(key) {
            Ordering::Equal => {
                match (self.0.data.as_ref(), data.as_ref()) {
                    (NodeState::<&T>::Free, NodeState::<&T>::Free)
                    | (NodeState::<&T>::Free, NodeState::<&T>::Valued(_))
                    | (NodeState::<&T>::Allocated, NodeState::<&T>::Free)
                    | (NodeState::<&T>::Allocated, NodeState::<&T>::Allocated)
                    | (NodeState::<&T>::Valued(_), NodeState::<&T>::Free)
                    | (NodeState::<&T>::Valued(_), NodeState::<&T>::Allocated) => {
                        panic!("try to update unallocated interval tree node");
                    }
                    _ => {}
                }
                self.0.data.replace(data).into()
            }
            Ordering::Less => match self.0.right.as_mut() {
                None => None,
                Some(node) => node.update(key, data),
            },
            Ordering::Greater => match self.0.left.as_mut() {
                None => None,
                Some(node) => node.update(key, data),
            },
        }
    }

    /// Delete `key` from the subtree.
    ///
    /// Note: it doesn't return whether the key exists in the subtree, so caller need to ensure the
    /// logic.
    fn delete(mut self, key: &Range) -> (Option<T>, Option<Self>) {
        match self.0.key.cmp(key) {
            Ordering::Equal => {
                let data = self.0.data.take();
                return (data.into(), self.delete_root());
            }
            Ordering::Less => {
                if let Some(node) = self.0.right.take() {
                    let (data, right) = node.delete(key);
                    self.0.right = right;
                    return (data, Some(self.updated_node()));
                }
            }
            Ordering::Greater => {
                if let Some(node) = self.0.left.take() {
                    let (data, left) = node.delete(key);
                    self.0.left = left;
                    return (data, Some(self.updated_node()));
                }
            }
        }
        (None, Some(self))
    }

    /// Rotate the node if necessary to keep balance.
    fn rotate(self) -> Self {
        let l = height(&self.0.left);
        let r = height(&self.0.right);
        match (l as i32) - (r as i32) {
            -1..=1 => self,
            2 => self.rotate_left_successor(),
            -2 => self.rotate_right_successor(),
            _ => unreachable!(),
        }
    }

    /// Perform a single left rotation on this node.
    fn rotate_left(mut self) -> Self {
        let mut new_root = self.0.right.take().expect("Node is broken");
        self.0.right = new_root.0.left.take();
        self.update_cached_info();
        new_root.0.left = Some(self);
        new_root.update_cached_info();
        new_root
    }

    /// Perform a single right rotation on this node.
    fn rotate_right(mut self) -> Self {
        let mut new_root = self.0.left.take().expect("Node is broken");
        self.0.left = new_root.0.right.take();
        self.update_cached_info();
        new_root.0.right = Some(self);
        new_root.update_cached_info();
        new_root
    }

    /// Performs a rotation when the left successor is too high.
    fn rotate_left_successor(mut self) -> Self {
        let left = self.0.left.take().expect("Node is broken");
        if height(&left.0.left) < height(&left.0.right) {
            let rotated = left.rotate_left();
            self.0.left = Some(rotated);
            self.update_cached_info();
        } else {
            self.0.left = Some(left);
        }
        self.rotate_right()
    }

    /// Performs a rotation when the right successor is too high.
    fn rotate_right_successor(mut self) -> Self {
        let right = self.0.right.take().expect("Node is broken");
        if height(&right.0.left) > height(&right.0.right) {
            let rotated = right.rotate_right();
            self.0.right = Some(rotated);
            self.update_cached_info();
        } else {
            self.0.right = Some(right);
        }
        self.rotate_left()
    }

    fn delete_root(mut self) -> Option<Self> {
        match (self.0.left.take(), self.0.right.take()) {
            (None, None) => None,
            (Some(l), None) => Some(l),
            (None, Some(r)) => Some(r),
            (Some(l), Some(r)) => Some(Self::combine_subtrees(l, r)),
        }
    }

    /// Find the minimal key below the tree and returns a new optional tree where the minimal
    /// value has been removed and the (optional) minimal node as tuple (min_node, remaining)
    fn get_new_root(mut self) -> (Self, Option<Self>) {
        match self.0.left.take() {
            None => {
                let remaining = self.0.right.take();
                (self, remaining)
            }
            Some(left) => {
                let (min_node, left) = left.get_new_root();
                self.0.left = left;
                (min_node, Some(self.updated_node()))
            }
        }
    }

    fn combine_subtrees(l: Self, r: Self) -> Self {
        let (mut new_root, remaining) = r.get_new_root();
        new_root.0.left = Some(l);
        new_root.0.right = remaining;
        new_root.updated_node()
    }

    fn find_candidate(&self, constraint: &Constraint) -> Option<&Self> {
        match constraint.policy {
            AllocPolicy::FirstMatch => self.first_match(constraint),
            AllocPolicy::Default => self.first_match(constraint),
        }
    }

    fn first_match(&self, constraint: &Constraint) -> Option<&Self> {
        let mut candidate = if self.0.left.is_some() {
            self.0.left.as_ref().unwrap().first_match(constraint)
        } else {
            None
        };

        if candidate.is_none() && self.check_constraint(constraint) {
            candidate = Some(self);
        }
        if candidate.is_none() && self.0.right.is_some() {
            candidate = self.0.right.as_ref().unwrap().first_match(constraint);
        }
        candidate
    }

    fn check_constraint(&self, constraint: &Constraint) -> bool {
        if self.0.data.is_free() {
            let min = std::cmp::max(self.0.key.min, constraint.min);
            let max = std::cmp::min(self.0.key.max, constraint.max);
            if min <= max {
                let key = Range::new(min, max);
                if constraint.align == 0 || constraint.align == 1 {
                    return key.len() >= constraint.size;
                }
                return match key.align_to(constraint.align) {
                    None => false,
                    Some(aligned_key) => aligned_key.len() >= constraint.size,
                };
            }
        }
        false
    }

    /// Update cached information of the node.
    /// Please make sure that the cached values of both children are up to date.
    fn update_cached_info(&mut self) {
        self.0.height = max(height(&self.0.left), height(&self.0.right)) + 1;
        self.0.max_key = max(
            max_key(&self.0.left),
            max(max_key(&self.0.right), self.0.key.max),
        );
    }

    /// Update the sub-tree to keep balance.
    fn updated_node(mut self) -> Self {
        self.update_cached_info();
        self.rotate()
    }
}

/// Compute height of the optional sub-tree.
fn height<T>(node: &Option<Node<T>>) -> u32 {
    node.as_ref().map_or(0, |n| n.0.height)
}

/// Compute maximum key value covered by the optional sub-tree.
fn max_key<T>(node: &Option<Node<T>>) -> u64 {
    node.as_ref().map_or(0, |n| n.0.max_key)
}

/// An interval tree implementation specialized for VMM resource management.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct IntervalTree<T> {
    root: Option<Node<T>>,
}

impl<T> IntervalTree<T> {
    /// Construct a default empty [IntervalTree] object.
    ///
    /// # Examples
    /// ```rust
    /// extern crate dbs_allocator;
    ///
    /// let tree = dbs_allocator::IntervalTree::<u64>::new();
    /// ```
    pub fn new() -> Self {
        IntervalTree { root: None }
    }

    /// Check whether the interval tree is empty.
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    /// Get the data item associated with the key, or return None if no match found.
    ///
    /// # Examples
    /// ```rust
    /// extern crate dbs_allocator;
    /// use dbs_allocator::{IntervalTree, NodeState, Range};
    ///
    /// let mut tree = dbs_allocator::IntervalTree::<u64>::new();
    /// assert!(tree.is_empty());
    /// assert_eq!(tree.get(&Range::new(0x101u64, 0x101u64)), None);
    /// tree.insert(Range::new(0x100u64, 0x100u64), Some(1));
    /// tree.insert(Range::new(0x200u64, 0x2ffu64), None);
    /// assert!(!tree.is_empty());
    /// assert_eq!(
    ///     tree.get(&Range::new(0x100u64, 0x100u64)),
    ///     Some(NodeState::Valued(&1))
    /// );
    /// assert_eq!(
    ///     tree.get(&Range::new(0x200u64, 0x2ffu64)),
    ///     Some(NodeState::Free)
    /// );
    /// assert_eq!(tree.get(&Range::new(0x101u64, 0x101u64)), None);
    /// assert_eq!(tree.get(&Range::new(0x100u64, 0x101u64)), None);
    /// ```
    pub fn get(&self, key: &Range) -> Option<NodeState<&T>> {
        match self.root {
            None => None,
            Some(ref node) => node.search(key).map(|n| n.0.data.as_ref()),
        }
    }

    /// Get a shared reference to the node fully covering the entire key range.
    ///
    /// # Examples
    /// ```rust
    /// extern crate dbs_allocator;
    /// use dbs_allocator::{IntervalTree, NodeState, Range};
    ///
    /// let mut tree = IntervalTree::<u64>::new();
    /// tree.insert(Range::new(0x100u32, 0x100u32), Some(1));
    /// tree.insert(Range::new(0x200u32, 0x2ffu32), None);
    /// assert_eq!(
    ///     tree.get_superset(&Range::new(0x100u32, 0x100u32)),
    ///     Some((&Range::new(0x100u32, 0x100u32), NodeState::Valued(&1)))
    /// );
    /// assert_eq!(
    ///     tree.get_superset(&Range::new(0x210u32, 0x210u32)),
    ///     Some((&Range::new(0x200u32, 0x2ffu32), NodeState::Free))
    /// );
    /// assert_eq!(
    ///     tree.get_superset(&Range::new(0x2ffu32, 0x2ffu32)),
    ///     Some((&Range::new(0x200u32, 0x2ffu32), NodeState::Free))
    /// );
    /// ```
    pub fn get_superset(&self, key: &Range) -> Option<(&Range, NodeState<&T>)> {
        match self.root {
            None => None,
            Some(ref node) => node
                .search_superset(key)
                .map(|n| (&n.0.key, n.0.data.as_ref())),
        }
    }

    /// Get a mutable reference to the node fully covering the entire key range.
    ///
    /// # Examples
    /// ```rust
    /// extern crate dbs_allocator;
    /// use dbs_allocator::{IntervalTree, NodeState, Range};
    ///
    /// let mut tree = IntervalTree::<u64>::new();
    /// tree.insert(Range::new(0x100u32, 0x100u32), Some(1));
    /// tree.insert(Range::new(0x200u32, 0x2ffu32), None);
    /// assert_eq!(
    ///     tree.get_superset_mut(&Range::new(0x100u32, 0x100u32)),
    ///     Some((&Range::new(0x100u32, 0x100u32), NodeState::Valued(&mut 1)))
    /// );
    /// assert_eq!(
    ///     tree.get_superset_mut(&Range::new(0x210u32, 0x210u32)),
    ///     Some((&Range::new(0x200u32, 0x2ffu32), NodeState::Free))
    /// );
    /// assert_eq!(
    ///     tree.get_superset_mut(&Range::new(0x2ffu32, 0x2ffu32)),
    ///     Some((&Range::new(0x200u32, 0x2ffu32), NodeState::Free))
    /// );
    /// ```
    pub fn get_superset_mut(&mut self, key: &Range) -> Option<(&Range, NodeState<&mut T>)> {
        match self.root {
            None => None,
            Some(ref mut node) => node
                .search_superset_mut(key)
                .map(|n| (&n.0.key, n.0.data.as_mut())),
        }
    }

    /// Get a shared reference to the value associated with the id.
    ///
    /// # Examples
    /// ```rust
    /// extern crate dbs_allocator;
    /// use dbs_allocator::{IntervalTree, NodeState, Range};
    ///
    /// let mut tree = IntervalTree::<u32>::new();
    /// tree.insert(Range::new(0x100u16, 0x100u16), Some(1));
    /// tree.insert(Range::new(0x200u16, 0x2ffu16), None);
    /// assert_eq!(tree.get_by_id(0x100u16), Some(&1));
    /// assert_eq!(tree.get_by_id(0x210u32), None);
    /// assert_eq!(tree.get_by_id(0x2ffu64), None);
    /// ```
    pub fn get_by_id<U>(&self, id: U) -> Option<&T>
    where
        u64: From<U>,
    {
        match self.root {
            None => None,
            Some(ref node) => {
                let key = Range::new_point(id);
                match node.search_superset(&key) {
                    Some(node) => node.0.data.as_ref().into(),
                    None => None,
                }
            }
        }
    }

    /// Get a mutable reference to the value associated with the id.
    ///
    /// # Examples
    /// ```rust
    /// extern crate dbs_allocator;
    /// use dbs_allocator::{IntervalTree, NodeState, Range};
    ///
    /// let mut tree = IntervalTree::<u32>::new();
    /// tree.insert(Range::new(0x100u16, 0x100u16), Some(1));
    /// tree.insert(Range::new(0x200u16, 0x2ffu16), None);
    /// assert_eq!(tree.get_by_id_mut(0x100u16), Some(&mut 1));
    /// assert_eq!(tree.get_by_id_mut(0x210u32), None);
    /// assert_eq!(tree.get_by_id_mut(0x2ffu64), None);
    /// ```
    pub fn get_by_id_mut<U>(&mut self, id: U) -> Option<&mut T>
    where
        u64: From<U>,
    {
        match self.root {
            None => None,
            Some(ref mut node) => {
                let key = Range::new_point(id);
                match node.search_superset_mut(&key) {
                    Some(node) => node.0.data.as_mut().into(),
                    None => None,
                }
            }
        }
    }

    /// Insert the (key, data) pair into the interval tree, panic if intersects with existing nodes.
    ///
    /// # Examples
    /// ```rust
    /// extern crate dbs_allocator;
    /// use dbs_allocator::{IntervalTree, NodeState, Range};
    ///
    /// let mut tree = IntervalTree::<u64>::new();
    /// tree.insert(Range::new(0x100u32, 0x100u32), Some(1));
    /// tree.insert(Range::new(0x200u32, 0x2ffu32), None);
    /// assert_eq!(
    ///     tree.get(&Range::new(0x100u64, 0x100u64)),
    ///     Some(NodeState::Valued(&1))
    /// );
    /// assert_eq!(
    ///     tree.get(&Range::new(0x200u64, 0x2ffu64)),
    ///     Some(NodeState::Free)
    /// );
    /// ```
    pub fn insert(&mut self, key: Range, data: Option<T>) {
        match self.root.take() {
            None => self.root = Some(Node::new(key, data)),
            Some(node) => self.root = Some(node.insert(key, data)),
        }
    }

    /// Update an existing entry and return the old value.
    ///
    /// # Examples
    /// ```rust
    /// extern crate dbs_allocator;
    /// use dbs_allocator::{Constraint, IntervalTree, Range};
    ///
    /// let mut tree = IntervalTree::<u64>::new();
    /// tree.insert(Range::new(0x100u64, 0x100u64), None);
    /// tree.insert(Range::new(0x200u64, 0x2ffu64), None);
    ///
    /// let constraint = Constraint::new(2u32);
    /// let key = tree.allocate(&constraint);
    /// assert_eq!(key, Some(Range::new(0x200u64, 0x201u64)));
    /// let old = tree.update(&Range::new(0x200u64, 0x201u64), 2);
    /// assert_eq!(old, None);
    /// let old = tree.update(&Range::new(0x200u64, 0x201u64), 3);
    /// assert_eq!(old, Some(2));
    /// ```
    pub fn update(&mut self, key: &Range, data: T) -> Option<T> {
        match self.root.as_mut() {
            None => None,
            Some(node) => node.update(key, NodeState::<T>::Valued(data)),
        }
    }

    /// Remove the `key` from the tree and return the associated data.
    ///
    /// # Examples
    /// ```rust
    /// extern crate dbs_allocator;
    /// use dbs_allocator::{IntervalTree, Range};
    ///
    /// let mut tree = IntervalTree::<u64>::new();
    /// tree.insert(Range::new(0x100u64, 0x100u64), Some(1));
    /// tree.insert(Range::new(0x200u64, 0x2ffu64), None);
    /// let old = tree.delete(&Range::new(0x100u64, 0x100u64));
    /// assert_eq!(old, Some(1));
    /// let old = tree.delete(&Range::new(0x200u64, 0x2ffu64));
    /// assert_eq!(old, None);
    /// ```
    pub fn delete(&mut self, key: &Range) -> Option<T> {
        match self.root.take() {
            Some(node) => {
                let (data, root) = node.delete(key);
                self.root = root;
                data
            }
            None => None,
        }
    }

    /// Allocate a resource range according the allocation constraints.
    ///
    /// # Examples
    /// ```rust
    /// extern crate dbs_allocator;
    /// use dbs_allocator::{Constraint, IntervalTree, Range};
    ///
    /// let mut tree = IntervalTree::<u64>::new();
    /// tree.insert(Range::new(0x100u64, 0x100u64), None);
    /// tree.insert(Range::new(0x200u64, 0x2ffu64), None);
    ///
    /// let constraint = Constraint::new(2u8);
    /// let key = tree.allocate(&constraint);
    /// assert_eq!(key, Some(Range::new(0x200u64, 0x201u64)));
    /// tree.update(&Range::new(0x200u64, 0x201u64), 2);
    /// ```
    pub fn allocate(&mut self, constraint: &Constraint) -> Option<Range> {
        if constraint.size == 0 {
            return None;
        }
        let candidate = match self.root.as_mut() {
            None => None,
            Some(node) => node.find_candidate(constraint),
        };

        match candidate {
            None => None,
            Some(node) => {
                let node_key = node.0.key;
                let range = Range::new(
                    max(node_key.min, constraint.min),
                    min(node_key.max, constraint.max),
                );
                // Safe to unwrap because candidate satisfy the constraints.
                let aligned_key = range.align_to(constraint.align).unwrap();
                let result = Range::new(aligned_key.min, aligned_key.min + constraint.size - 1);

                // Allocate a resource from the node, no need to split the candidate node.
                if node_key.min == aligned_key.min && node_key.len() == constraint.size {
                    self.root
                        .as_mut()
                        .unwrap()
                        .update(&node_key, NodeState::<T>::Allocated);
                    return Some(node_key);
                }

                // Split the candidate node.
                // TODO: following algorithm is not optimal in preference of simplicity.
                self.delete(&node_key);
                if aligned_key.min > node_key.min {
                    self.insert(Range::new(node_key.min, aligned_key.min - 1), None);
                }
                self.insert(result, None);
                if result.max < node_key.max {
                    self.insert(Range::new(result.max + 1, node_key.max), None);
                }

                self.root
                    .as_mut()
                    .unwrap()
                    .update(&result, NodeState::<T>::Allocated);
                Some(result)
            }
        }
    }

    /// Free an allocated range and return the associated data.
    pub fn free(&mut self, key: &Range) -> Option<T> {
        let result = self.delete(key);
        let mut range = *key;

        // Try to merge with adjacent free nodes.
        if range.min > 0 {
            if let Some((r, v)) = self.get_superset(&Range::new(range.min - 1, range.min - 1)) {
                if v.is_free() {
                    range.min = r.min;
                }
            }
        }
        if range.max < std::u64::MAX {
            if let Some((r, v)) = self.get_superset(&Range::new(range.max + 1, range.max + 1)) {
                if v.is_free() {
                    range.max = r.max;
                }
            }
        }

        if range.min < key.min {
            self.delete(&Range::new(range.min, key.min - 1));
        }
        if range.max > key.max {
            self.delete(&Range::new(key.max + 1, range.max));
        }
        self.insert(range, None);

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn test_new_range() {
        let _ = Range::new(2u8, 1u8);
    }

    #[test]
    #[should_panic]
    fn test_new_range_overflow() {
        let _ = Range::new(0u64, std::u64::MAX);
    }

    #[test]
    fn test_range_intersect() {
        let range_a = Range::new(1u8, 4u8);
        let range_b = Range::new(4u16, 6u16);
        let range_c = Range::new(2u32, 3u32);
        let range_d = Range::new(4u64, 4u64);
        let range_e = Range::new(5u32, 6u32);

        assert!(range_a.intersect(&range_b));
        assert!(range_b.intersect(&range_a));
        assert!(range_a.intersect(&range_c));
        assert!(range_c.intersect(&range_a));
        assert!(range_a.intersect(&range_d));
        assert!(range_d.intersect(&range_a));
        assert!(!range_a.intersect(&range_e));
        assert!(!range_e.intersect(&range_a));

        assert_eq!(range_a.len(), 4);
        assert_eq!(range_d.len(), 1);
    }

    #[test]
    fn test_range_contain() {
        let range_a = Range::new(2u8, 6u8);
        assert!(range_a.contain(&Range::new(2u8, 3u8)));
        assert!(range_a.contain(&Range::new(3u8, 4u8)));
        assert!(range_a.contain(&Range::new(5u8, 5u8)));
        assert!(range_a.contain(&Range::new(5u8, 6u8)));
        assert!(range_a.contain(&Range::new(6u8, 6u8)));
        assert!(!range_a.contain(&Range::new(1u8, 1u8)));
        assert!(!range_a.contain(&Range::new(1u8, 2u8)));
        assert!(!range_a.contain(&Range::new(1u8, 3u8)));
        assert!(!range_a.contain(&Range::new(1u8, 7u8)));
        assert!(!range_a.contain(&Range::new(7u8, 8u8)));
        assert!(!range_a.contain(&Range::new(6u8, 7u8)));
        assert!(!range_a.contain(&Range::new(7u8, 8u8)));
    }

    #[test]
    fn test_range_align_to() {
        let range_a = Range::new(2u32, 6);
        assert_eq!(range_a.align_to(0), Some(Range::new(2u64, 6u64)));
        assert_eq!(range_a.align_to(1), Some(Range::new(2u8, 6u8)));
        assert_eq!(range_a.align_to(2), Some(Range::new(2u16, 6u16)));
        assert_eq!(range_a.align_to(4), Some(Range::new(4u32, 6u32)));
        assert_eq!(range_a.align_to(8), None);
        assert_eq!(range_a.align_to(3), None);

        let range_b = Range::new(0xFFFF_FFFF_FFFF_FFFDu64, 0xFFFF_FFFF_FFFF_FFFFu64);
        assert_eq!(
            range_b.align_to(2),
            Some(Range::new(0xFFFF_FFFF_FFFF_FFFEu64, 0xFFFF_FFFF_FFFF_FFFF))
        );
        assert_eq!(range_b.align_to(4), None);
    }

    #[test]
    fn test_range_ord() {
        let range_a = Range::new(1u32, 4u32);
        let range_b = Range::new(1u32, 4u32);
        let range_c = Range::new(1u32, 3u32);
        let range_d = Range::new(1u32, 5u32);
        let range_e = Range::new(2u32, 2u32);

        assert_eq!(range_a, range_b);
        assert_eq!(range_b, range_a);
        assert!(range_a > range_c);
        assert!(range_c < range_a);
        assert!(range_a < range_d);
        assert!(range_d > range_a);
        assert!(range_a < range_e);
        assert!(range_e > range_a);
    }

    #[should_panic]
    #[test]
    fn test_tree_insert_equal() {
        let mut tree = IntervalTree::<u64>::new();
        tree.insert(Range::new(0x100u16, 0x200), Some(1));
        tree.insert(Range::new(0x100u32, 0x200), None);
    }

    #[should_panic]
    #[test]
    fn test_tree_insert_intersect_on_right() {
        let mut tree = IntervalTree::<u64>::new();
        tree.insert(Range::new(0x100, 0x200u32), Some(1));
        tree.insert(Range::new(0x200, 0x2ffu64), None);
    }

    #[should_panic]
    #[test]
    fn test_tree_insert_intersect_on_left() {
        let mut tree = IntervalTree::<u64>::new();
        tree.insert(Range::new(0x100, 0x200u32), Some(1));
        tree.insert(Range::new(0x000, 0x100u64), None);
    }

    #[test]
    fn test_tree_get_superset() {
        let mut tree = IntervalTree::<u64>::new();
        tree.insert(Range::new(0x100u32, 0x100u32), Some(1));
        tree.insert(Range::new(0x001u16, 0x008u16), None);
        tree.insert(Range::new(0x009u16, 0x00fu16), None);
        tree.insert(Range::new(0x200u16, 0x2ffu16), None);
        let mut constraint = Constraint::new(8u64);
        constraint.min = 0x211;
        constraint.max = 0x21f;
        constraint.align = 0x8;
        tree.allocate(&constraint);

        // Valued case.
        assert_eq!(
            tree.get_superset(&Range::new(0x100u32, 0x100)),
            Some((&Range::new(0x100, 0x100u32), NodeState::Valued(&1)))
        );

        // Free case.
        assert_eq!(
            tree.get_superset(&Range::new(0x200u16, 0x200)),
            Some((&Range::new(0x200, 0x217u64), NodeState::Free))
        );
        assert_eq!(
            tree.get_superset(&Range::new(0x2ffu32, 0x2ff)),
            Some((&Range::new(0x220, 0x2ffu32), NodeState::Free))
        );

        // Allocated case.
        assert_eq!(
            tree.get_superset(&Range::new(0x218u16, 0x21f)),
            Some((&Range::new(0x218, 0x21fu16), NodeState::Allocated))
        );

        // None case.
        assert_eq!(tree.get_superset(&Range::new(0x2ffu32, 0x300)), None);
        assert_eq!(tree.get_superset(&Range::new(0x300u32, 0x300)), None);
        assert_eq!(tree.get_superset(&Range::new(0x1ffu32, 0x300)), None);
    }

    #[test]
    fn test_tree_get_superset_mut() {
        let mut tree = IntervalTree::<u64>::new();
        tree.insert(Range::new(0x100u32, 0x100u32), Some(1));
        tree.insert(Range::new(0x200u16, 0x2ffu16), None);
        let mut constraint = Constraint::new(8u64);
        constraint.min = 0x211;
        constraint.max = 0x21f;
        constraint.align = 0x8;
        tree.allocate(&constraint);

        // Valued case.
        assert_eq!(
            tree.get_superset_mut(&Range::new(0x100u32, 0x100u32)),
            Some((&Range::new(0x100u32, 0x100u32), NodeState::Valued(&mut 1)))
        );

        // Allocated case.
        assert_eq!(
            tree.get_superset_mut(&Range::new(0x218u64, 0x21fu64)),
            Some((&Range::new(0x218u64, 0x21fu64), NodeState::Allocated))
        );

        // Free case.
        assert_eq!(
            tree.get_superset_mut(&Range::new(0x2ffu32, 0x2ffu32)),
            Some((&Range::new(0x220u32, 0x2ffu32), NodeState::Free))
        );

        // None case.
        assert_eq!(tree.get_superset(&Range::new(0x2ffu32, 0x300)), None);
        assert_eq!(tree.get_superset(&Range::new(0x300u32, 0x300)), None);
        assert_eq!(tree.get_superset(&Range::new(0x1ffu32, 0x300)), None);
    }

    #[test]
    fn test_tree_update() {
        let mut tree = IntervalTree::<u64>::new();
        tree.insert(Range::new(0x100u32, 0x100u32), None);
        tree.insert(Range::new(0x200u32, 0x2ffu32), None);

        let constraint = Constraint::new(2u32);
        let key = tree.allocate(&constraint);
        assert_eq!(key, Some(Range::new(0x200u32, 0x201u32)));
        let old = tree.update(&Range::new(0x200u32, 0x201u32), 2);
        assert_eq!(old, None);
        let old = tree.update(&Range::new(0x200u32, 0x201u32), 3);
        assert_eq!(old, Some(2));
        let old = tree.update(&Range::new(0x200u32, 0x200u32), 4);
        assert_eq!(old, None);
        let old = tree.update(&Range::new(0x200u32, 0x203u32), 5);
        assert_eq!(old, None);

        tree.delete(&Range::new(0x200u32, 0x201u32));
        let old = tree.update(&Range::new(0x200u32, 0x201u32), 2);
        assert_eq!(old, None);
    }

    #[test]
    fn test_tree_delete() {
        let mut tree = IntervalTree::<u64>::new();
        assert_eq!(tree.get(&Range::new(0x101u32, 0x101u32)), None);
        assert!(tree.is_empty());
        tree.insert(Range::new(0x100u32, 0x100u32), Some(1));
        tree.insert(Range::new(0x001u16, 0x00fu16), None);
        tree.insert(Range::new(0x200u32, 0x2ffu32), None);
        assert!(!tree.is_empty());
        assert_eq!(
            tree.get(&Range::new(0x100u32, 0x100u32)),
            Some(NodeState::Valued(&1))
        );
        assert_eq!(
            tree.get(&Range::new(0x200u32, 0x2ffu32)),
            Some(NodeState::Free)
        );
        assert_eq!(tree.get(&Range::new(0x101u32, 0x101u32)), None);

        let old = tree.delete(&Range::new(0x001u16, 0x00fu16));
        assert_eq!(old, None);
        let old = tree.delete(&Range::new(0x100u32, 0x100u32));
        assert_eq!(old, Some(1));
        let old = tree.delete(&Range::new(0x200u32, 0x2ffu32));
        assert_eq!(old, None);

        assert!(tree.is_empty());
        assert_eq!(tree.get(&Range::new(0x100u32, 0x100u32)), None);
        assert_eq!(tree.get(&Range::new(0x200u32, 0x2ffu32)), None);
    }

    #[test]
    fn test_allocate_free() {
        let mut tree = IntervalTree::<u64>::new();
        let mut constraint = Constraint::new(1u8);

        assert_eq!(tree.allocate(&constraint), None);
        tree.insert(Range::new(0x100u16, 0x100u16), None);
        tree.insert(Range::new(0x200u16, 0x2ffu16), None);

        let key = tree.allocate(&constraint);
        assert_eq!(key, Some(Range::new(0x100u16, 0x100u16)));
        let old = tree.update(&Range::new(0x100u16, 0x100u16), 2);
        assert_eq!(old, None);
        let val = tree.get(&Range::new(0x100u16, 0x100u16));
        assert_eq!(val, Some(NodeState::Valued(&2)));

        constraint.min = 0x100;
        constraint.max = 0x100;
        assert_eq!(tree.allocate(&constraint), None);

        constraint.min = 0x201;
        constraint.max = 0x300;
        constraint.align = 0x8;
        constraint.size = 0x10;
        assert_eq!(
            tree.allocate(&constraint),
            Some(Range::new(0x208u16, 0x217u16))
        );

        // Free the node when it's still in 'Allocated' state.
        let old = tree.free(&Range::new(0x208u16, 0x217u16));
        assert_eq!(old, None);

        // Reallocate the freed resource.
        assert_eq!(
            tree.allocate(&constraint),
            Some(Range::new(0x208u16, 0x217u16))
        );

        constraint.size = 0x100;
        assert_eq!(tree.allocate(&constraint), None);

        // Verify that allocating a bigger range with smaller allocated range fails.
        constraint.min = 0x200;
        constraint.max = 0x2ff;
        constraint.align = 0x8;
        constraint.size = 0x100;
        assert_eq!(tree.allocate(&constraint), None);

        // Free the node when it's in 'Valued' state.
        tree.update(&Range::new(0x208u16, 0x217u16), 0x10);
        assert_eq!(tree.allocate(&constraint), None);
        let old = tree.free(&Range::new(0x208u16, 0x217u16));
        assert_eq!(old, Some(0x10));

        // Reallocate the freed resource, verify that adjacent free nodes have been merged.
        assert_eq!(
            tree.allocate(&constraint),
            Some(Range::new(0x200u32, 0x2ffu32))
        );
    }

    #[test]
    fn test_with_size() {
        let range_a = Range::with_size(1u8, 3u8);
        let range_b = Range::with_size(4u16, 2u16);
        let range_c = Range::with_size(2u32, 1u32);
        let range_d = Range::with_size(4u64, 0u64);
        let range_e = Range::with_size(5u32, 1u32);

        assert_eq!(range_a, Range::new(1u8, 4u8));
        assert_eq!(range_b, Range::new(4u16, 6u16));
        assert_eq!(range_c, Range::new(2u32, 3u32));
        assert_eq!(range_d, Range::new(4u64, 4u64));
        assert_eq!(range_e, Range::new(5u32, 6u32));
    }

    #[test]
    fn test_new_point() {
        let range_a = Range::new_point(1u8);
        let range_b = Range::new_point(2u16);
        let range_c = Range::new_point(3u32);
        let range_d = Range::new_point(4u64);
        let range_e = Range::new_point(5u32);

        assert_eq!(range_a, Range::with_size(1u8, 0u8));
        assert_eq!(range_b, Range::with_size(2u16, 0u16));
        assert_eq!(range_c, Range::with_size(3u32, 0u32));
        assert_eq!(range_d, Range::with_size(4u64, 0u64));
        assert_eq!(range_e, Range::with_size(5u32, 0u32));
    }

    #[test]
    fn test_get_by_id() {
        let mut tree = IntervalTree::<u32>::new();
        tree.insert(Range::new(0x100u16, 0x100u16), Some(1));
        tree.insert(Range::new(0x001u32, 0x005u32), Some(2));
        tree.insert(Range::new(0x200u16, 0x2ffu16), None);

        assert_eq!(tree.get_by_id(0x100u16), Some(&1));
        assert_eq!(tree.get_by_id(0x002u32), Some(&2));
        assert_eq!(tree.get_by_id(0x210u32), None);
        assert_eq!(tree.get_by_id(0x2ffu64), None);
    }

    #[test]
    fn test_get_by_id_mut() {
        let mut tree = IntervalTree::<u32>::new();
        tree.insert(Range::new(0x100u16, 0x100u16), Some(1));
        tree.insert(Range::new(0x001u32, 0x005u32), Some(2));
        tree.insert(Range::new(0x200u16, 0x2ffu16), None);

        assert_eq!(tree.get_by_id_mut(0x100u16), Some(&mut 1));
        assert_eq!(tree.get_by_id_mut(0x002u32), Some(&mut 2));
        assert_eq!(tree.get_by_id_mut(0x210u32), None);
        assert_eq!(tree.get_by_id_mut(0x2ffu64), None);
    }
}
