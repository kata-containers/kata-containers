// Copyright 2020 Ant Group. All rights reserved.
// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;
use std::fmt::{Debug, Display, Formatter};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

#[derive(Default)]
pub struct InodeBitmap {
    map: RwLock<BTreeMap<u64, AtomicU64>>,
}

impl Debug for InodeBitmap {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_string().as_str())
    }
}

impl Display for InodeBitmap {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            serde_json::json!({"inode_range": self.bitmap_to_array()})
                .to_string()
                .as_str(),
        )
    }
}

impl InodeBitmap {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline(always)]
    fn get_index_and_mask(ino: u64) -> (u64, u64) {
        (ino >> 6, 1_u64 << (ino & 0x3f_u64))
    }

    #[inline(always)]
    fn range_to_vec(start: u64, end: u64) -> Vec<u64> {
        if start == end {
            vec![start]
        } else {
            vec![start, end]
        }
    }

    pub fn set(&self, ino: u64) {
        let (index, mask) = Self::get_index_and_mask(ino);

        let m = self.map.read().unwrap();
        if let Some(v) = m.get(&index) {
            v.fetch_or(mask, Ordering::Relaxed);
            return;
        }
        drop(m);

        let mut m = self.map.write().unwrap();
        m.entry(index)
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_or(mask, Ordering::Relaxed);
    }

    pub fn is_set(&self, ino: u64) -> bool {
        let (index, mask) = InodeBitmap::get_index_and_mask(ino);
        self.map
            .read()
            .unwrap()
            .get(&index)
            .map_or(false, |v| v.load(Ordering::Relaxed) & mask != 0)
    }

    pub fn clear(&self, ino: u64) {
        let (index, mask) = InodeBitmap::get_index_and_mask(ino);
        let m = self.map.read().unwrap();

        if let Some(v) = m.get(&index) {
            v.fetch_and(!mask, Ordering::Relaxed);
        }
    }

    pub fn clear_all(&self) {
        let m = self.map.read().unwrap();

        for it in m.values() {
            it.store(0_u64, Ordering::Relaxed);
        }
    }

    /// "[[1,5],[8],[10],[100,199],...]"
    fn bitmap_to_vec(&self, load: fn(&AtomicU64) -> u64) -> Vec<Vec<u64>> {
        let m = self.map.read().unwrap();
        let mut ret: Vec<Vec<u64>> = Vec::new();
        let mut start: Option<u64> = None;
        // 0 is an invalid inode number
        let mut last: u64 = 0;

        for it in m.iter() {
            let base = it.0 << 6;
            let mut v = load(it.1);

            while v != 0 {
                // trailing_zeros need rustup version >= 1.46
                let ino = base + v.trailing_zeros() as u64;
                v &= v - 1;
                start = match start {
                    None => Some(ino),
                    Some(s) => {
                        if ino != last + 1 {
                            ret.push(InodeBitmap::range_to_vec(s, last));
                            Some(ino)
                        } else {
                            Some(s)
                        }
                    }
                };
                last = ino;
            }
        }
        if let Some(s) = start {
            ret.push(InodeBitmap::range_to_vec(s, last));
        }

        ret
    }

    pub fn bitmap_to_array(&self) -> Vec<Vec<u64>> {
        self.bitmap_to_vec(|v| v.load(Ordering::Relaxed))
    }

    pub fn bitmap_to_array_and_clear(&self) -> Vec<Vec<u64>> {
        self.bitmap_to_vec(|v| v.fetch_and(0_u64, Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inode_bitmap() {
        let empty: Vec<Vec<u64>> = Vec::new();
        let m = InodeBitmap::new();
        m.set(1);
        m.set(2);
        m.set(5);
        assert_eq!(m.bitmap_to_array(), [vec![1, 2], vec![5]]);

        assert!(m.is_set(2));
        m.clear(2);
        assert!(!m.is_set(2));
        assert_eq!(m.bitmap_to_array(), [[1], [5]]);

        m.set(65);
        m.set(66);
        m.set(4000);
        m.set(40001);
        m.set(40002);
        m.set(40003);
        assert_eq!(
            m.bitmap_to_array(),
            [
                vec![1],
                vec![5],
                vec![65, 66],
                vec![4000],
                vec![40001, 40003]
            ]
        );

        m.clear_all();
        assert_eq!(m.bitmap_to_array(), empty);

        m.set(65);
        m.set(40001);
        assert_eq!(m.bitmap_to_array(), [vec![65], vec![40001]]);

        for i in 0..100000 {
            m.set(i);
        }
        m.set(100002);
        assert_eq!(
            m.bitmap_to_array_and_clear(),
            [vec![0, 99999], vec![100002]]
        );
        assert!(!m.is_set(9000));
        assert_eq!(m.bitmap_to_array(), empty);
    }
}
