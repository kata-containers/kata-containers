// Copyright (c) 2017 Martijn Rijkeboer <mrr@sru-systems.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::block::Block;
use std::fmt;
use std::fmt::Debug;
use std::ops::{Index, IndexMut};

/// Structure representing the memory matrix.
pub struct Memory {
    /// The number of rows.
    rows: usize,

    /// The number of columns.
    cols: usize,

    /// The flat array of blocks representing the memory matrix.
    blocks: Box<[Block]>,
}

impl Memory {
    /// Creates a new memory matrix.
    pub fn new(lanes: u32, lane_length: u32) -> Memory {
        let rows = lanes as usize;
        let cols = lane_length as usize;
        let total = rows * cols;
        let blocks = vec![Block::zero(); total].into_boxed_slice();
        Memory { rows, cols, blocks }
    }

    #[cfg(feature = "crossbeam-utils")]
    /// Gets the mutable lanes representation of the memory matrix.
    pub fn as_lanes_mut(&mut self) -> Vec<&mut Memory> {
        let ptr: *mut Memory = self;
        let mut vec = Vec::with_capacity(self.rows);
        for _ in 0..self.rows {
            vec.push(unsafe { &mut (*ptr) });
        }
        vec
    }
}

impl Debug for Memory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Memory {{ rows: {}, cols: {} }}", self.rows, self.cols)
    }
}

impl Index<u32> for Memory {
    type Output = Block;
    fn index(&self, index: u32) -> &Block {
        &self.blocks[index as usize]
    }
}

impl Index<u64> for Memory {
    type Output = Block;
    fn index(&self, index: u64) -> &Block {
        &self.blocks[index as usize]
    }
}

impl Index<(u32, u32)> for Memory {
    type Output = Block;
    fn index(&self, index: (u32, u32)) -> &Block {
        let pos = ((index.0 as usize) * self.cols) + (index.1 as usize);
        &self.blocks[pos]
    }
}

impl IndexMut<u32> for Memory {
    fn index_mut(&mut self, index: u32) -> &mut Block {
        &mut self.blocks[index as usize]
    }
}

impl IndexMut<u64> for Memory {
    fn index_mut(&mut self, index: u64) -> &mut Block {
        &mut self.blocks[index as usize]
    }
}

impl IndexMut<(u32, u32)> for Memory {
    fn index_mut(&mut self, index: (u32, u32)) -> &mut Block {
        let pos = ((index.0 as usize) * self.cols) + (index.1 as usize);
        &mut self.blocks[pos]
    }
}

#[cfg(test)]
mod tests {

    use crate::memory::Memory;

    #[test]
    fn new_returns_correct_instance() {
        let lanes = 4;
        let lane_length = 128;
        let memory = Memory::new(lanes, lane_length);
        assert_eq!(memory.rows, lanes as usize);
        assert_eq!(memory.cols, lane_length as usize);
        assert_eq!(memory.blocks.len(), 512);
    }

    #[cfg(feature = "crossbeam-utils")]
    #[test]
    fn as_lanes_mut_returns_correct_vec() {
        let mut memory = Memory::new(4, 128);
        let lanes = memory.as_lanes_mut();
        assert_eq!(lanes.len(), 4);
    }
}
