// Copyright (c) 2017 Martijn Rijkeboer <mrr@sru-systems.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::common;
use std::fmt;
use std::fmt::Debug;
use std::ops::{BitXorAssign, Index, IndexMut};

/// Structure for the (1KB) memory block implemented as 128 64-bit words.
pub struct Block([u64; common::QWORDS_IN_BLOCK]);

impl Block {
    /// Gets the byte slice representation of the block.
    pub fn as_u8(&self) -> &[u8] {
        let bytes: &[u8; common::BLOCK_SIZE] = unsafe {
            &*(&self.0 as *const [u64; common::QWORDS_IN_BLOCK] as *const [u8; common::BLOCK_SIZE])
        };
        bytes
    }

    /// Gets the mutable byte slice representation of the block.
    pub fn as_u8_mut(&mut self) -> &mut [u8] {
        let bytes: &mut [u8; common::BLOCK_SIZE] = unsafe {
            &mut *(&mut self.0 as *mut [u64; common::QWORDS_IN_BLOCK]
                as *mut [u8; common::BLOCK_SIZE])
        };
        bytes
    }

    /// Copies self to destination.
    pub fn copy_to(&self, dst: &mut Block) {
        for (d, s) in dst.0.iter_mut().zip(self.0.iter()) {
            *d = *s
        }
    }

    /// Creates a new block filled with zeros.
    pub fn zero() -> Block {
        Block([0u64; common::QWORDS_IN_BLOCK])
    }
}

impl<'a> BitXorAssign<&'a Block> for Block {
    fn bitxor_assign(&mut self, rhs: &Block) {
        for (s, r) in self.0.iter_mut().zip(rhs.0.iter()) {
            *s ^= *r
        }
    }
}

impl Clone for Block {
    fn clone(&self) -> Block {
        Block(self.0)
    }
}

impl Debug for Block {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_list().entries(self.0.iter()).finish()
    }
}

impl Eq for Block {}

impl Index<usize> for Block {
    type Output = u64;
    fn index(&self, index: usize) -> &u64 {
        &self.0[index]
    }
}

impl IndexMut<usize> for Block {
    fn index_mut(&mut self, index: usize) -> &mut u64 {
        &mut self.0[index]
    }
}

impl PartialEq for Block {
    fn eq(&self, other: &Block) -> bool {
        let mut equal = true;
        for (s, o) in self.0.iter().zip(other.0.iter()) {
            if s != o {
                equal = false;
            }
        }
        equal
    }
}

#[cfg(test)]
mod tests {

    use crate::block::Block;
    use crate::common;

    #[test]
    fn as_u8_returns_correct_slice() {
        let block = Block::zero();
        let expected = vec![0u8; 1024];
        let actual = block.as_u8();
        assert_eq!(actual, expected.as_slice());
    }

    #[test]
    fn as_u8_mut_returns_correct_slice() {
        let mut block = Block::zero();
        let mut expected = vec![0u8; 1024];
        let actual = block.as_u8_mut();
        assert_eq!(actual, expected.as_mut_slice());
    }

    #[test]
    fn bitxor_assign_updates_lhs() {
        let mut lhs = Block([0u64; common::QWORDS_IN_BLOCK]);
        let rhs = Block([1u64; common::QWORDS_IN_BLOCK]);
        lhs ^= &rhs;
        assert_eq!(lhs, rhs);
    }

    #[test]
    fn copy_to_copies_block() {
        let src = Block([1u64; common::QWORDS_IN_BLOCK]);
        let mut dst = Block([0u64; common::QWORDS_IN_BLOCK]);
        src.copy_to(&mut dst);
        assert_eq!(dst, src);
    }

    #[test]
    fn clone_clones_block() {
        let orig = Block([1u64; common::QWORDS_IN_BLOCK]);
        let copy = orig.clone();
        assert_eq!(copy, orig);
    }

    #[test]
    fn zero_creates_block_will_all_zeros() {
        let expected = Block([0u64; common::QWORDS_IN_BLOCK]);
        let actual = Block::zero();
        assert_eq!(actual, expected);
    }
}
