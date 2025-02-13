// Copyright (C) 2019, 2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Data structures and algorithms to support resource allocation and management.
//!
//! The `dbs-allocator` crate provides data structures and algorithms to manage and allocate
//! integer identifiable resources. The resource manager in virtual machine monitor (VMM) may
//! manage and allocate resources for virtual machines by using:
//! - [Constraint]: Struct to declare constraints for resource allocation.
//! - [IntervalTree]: An interval tree implementation specialized for VMM resource management.

#![deny(missing_docs)]

pub mod interval_tree;
pub use interval_tree::{IntervalTree, NodeState, Range};

/// Error codes for resource allocation operations.
#[derive(thiserror::Error, Debug, Eq, PartialEq)]
pub enum Error {
    /// Invalid boundary for resource allocation.
    #[error("invalid boundary constraint: min ({0}), max ({1})")]
    InvalidBoundary(u64, u64),
}

/// Specialized version of [`std::result::Result`] for resource allocation operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Resource allocation policies.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AllocPolicy {
    /// Default resource allocation policy.
    Default,
    /// Return the first available resource matching the allocation constraints.
    FirstMatch,
}

/// Struct to declare resource allocation constraints.
#[derive(Copy, Clone, Debug)]
pub struct Constraint {
    /// Size of resource to allocate.
    pub size: u64,
    /// Lower boundary for resource allocation.
    pub min: u64,
    /// Upper boundary for resource allocation.
    pub max: u64,
    /// Alignment for allocated resource.
    pub align: u64,
    /// Policy for resource allocation.
    pub policy: AllocPolicy,
}

impl Constraint {
    /// Create a new instance of [`Constraint`] with default settings.
    pub fn new<T>(size: T) -> Self
    where
        u64: From<T>,
    {
        Constraint {
            size: u64::from(size),
            min: 0,
            max: u64::MAX,
            align: 1,
            policy: AllocPolicy::Default,
        }
    }

    /// Set the lower boundary constraint for resource allocation.
    pub fn min<T>(mut self, min: T) -> Self
    where
        u64: From<T>,
    {
        self.min = u64::from(min);
        self
    }

    /// Set the upper boundary constraint for resource allocation.
    pub fn max<T>(mut self, max: T) -> Self
    where
        u64: From<T>,
    {
        self.max = u64::from(max);
        self
    }

    /// Set the alignment constraint for allocated resource.
    pub fn align<T>(mut self, align: T) -> Self
    where
        u64: From<T>,
    {
        self.align = u64::from(align);
        self
    }

    /// Set the resource allocation policy.
    pub fn policy(mut self, policy: AllocPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Validate the resource allocation constraints.
    pub fn validate(&self) -> Result<()> {
        if self.max < self.min {
            return Err(Error::InvalidBoundary(self.min, self.max));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_set_min() {
        let constraint = Constraint::new(2_u64).min(1_u64);
        assert_eq!(constraint.min, 1_u64);
    }

    #[test]
    fn test_set_max() {
        let constraint = Constraint::new(2_u64).max(100_u64);
        assert_eq!(constraint.max, 100_u64);
    }

    #[test]
    fn test_set_align() {
        let constraint = Constraint::new(2_u64).align(8_u64);
        assert_eq!(constraint.align, 8_u64);
    }

    #[test]
    fn test_set_policy() {
        let mut constraint = Constraint::new(2_u64).policy(AllocPolicy::FirstMatch);
        assert_eq!(constraint.policy, AllocPolicy::FirstMatch);
        constraint = constraint.policy(AllocPolicy::Default);
        assert_eq!(constraint.policy, AllocPolicy::Default);
    }

    #[test]
    fn test_consistently_change_constraint() {
        let constraint = Constraint::new(2_u64)
            .min(1_u64)
            .max(100_u64)
            .align(8_u64)
            .policy(AllocPolicy::FirstMatch);
        assert_eq!(constraint.min, 1_u64);
        assert_eq!(constraint.max, 100_u64);
        assert_eq!(constraint.align, 8_u64);
        assert_eq!(constraint.policy, AllocPolicy::FirstMatch);
    }

    #[test]
    fn test_set_invalid_boundary() {
        // Normal case.
        let constraint = Constraint::new(2_u64).max(1000_u64).min(999_u64);
        assert!(constraint.validate().is_ok());

        // Error case.
        let constraint = Constraint::new(2_u64).max(999_u64).min(1000_u64);
        assert_eq!(
            constraint.validate(),
            Err(Error::InvalidBoundary(1000u64, 999u64))
        );
    }
}
