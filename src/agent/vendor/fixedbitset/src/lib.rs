//! `FixedBitSet` is a simple fixed size set of bits.
#![doc(html_root_url="https://docs.rs/fixedbitset/0.1/")]

mod range;

use std::ops::{BitAnd, BitOr, Index};
use std::cmp::{Ord, Ordering};
use std::iter::{Chain, FromIterator};
pub use range::IndexRange;

static TRUE: bool = true;
static FALSE: bool = false;

const BITS: usize = 32;
type Block = u32;

#[inline]
fn div_rem(x: usize, d: usize) -> (usize, usize)
{
    (x / d, x % d)
}

/// `FixedBitSet` is a simple fixed size set of bits that each can
/// be enabled (1 / **true**) or disabled (0 / **false**).
///
/// The bit set has a fixed capacity in terms of enabling bits (and the
/// capacity can grow using the `grow` method).
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FixedBitSet {
    data: Vec<Block>,
    /// length in bits
    length: usize,
}

impl FixedBitSet
{
    /// Create a new **FixedBitSet** with a specific number of bits,
    /// all initially clear.
    pub fn with_capacity(bits: usize) -> Self
    {
        let (mut blocks, rem) = div_rem(bits, BITS);
        blocks += (rem > 0) as usize;
        FixedBitSet {
            data: vec![0; blocks],
            length: bits,
        }
    }
    
    /// Grow capacity to **bits**, all new bits initialized to zero
    pub fn grow(&mut self, bits: usize) {
        let (mut blocks, rem) = div_rem(bits, BITS);
        blocks += (rem > 0) as usize;
        if bits > self.length {
            self.length = bits;
            self.data.resize(blocks, 0);
        }
    }

    /// Return the length of the `FixedBitSet` in bits.
    #[inline]
    pub fn len(&self) -> usize { self.length }

    /// Return **true** if the bit is enabled in the **FixedBitSet**,
    /// **false** otherwise.
    ///
    /// Note: bits outside the capacity are always disabled.
    ///
    /// Note: Also available with index syntax: `bitset[bit]`.
    #[inline]
    pub fn contains(&self, bit: usize) -> bool
    {
        let (block, i) = div_rem(bit, BITS);
        match self.data.get(block) {
            None => false,
            Some(b) => (b & (1 << i)) != 0,
        }
    }

    /// Clear all bits.
    #[inline]
    pub fn clear(&mut self)
    {
        for elt in &mut self.data[..] {
            *elt = 0
        }
    }

    /// Enable `bit`.
    ///
    /// **Panics** if **bit** is out of bounds.
    #[inline]
    pub fn insert(&mut self, bit: usize)
    {
        assert!(bit < self.length);
        let (block, i) = div_rem(bit, BITS);
        unsafe {
            *self.data.get_unchecked_mut(block) |= 1 << i;
        }
    }

    /// Enable `bit`, and return its previous value.
    ///
    /// **Panics** if **bit** is out of bounds.
    #[inline]
    pub fn put(&mut self, bit: usize) -> bool
    {
        assert!(bit < self.length);
        let (block, i) = div_rem(bit, BITS);
        unsafe {
            let word = self.data.get_unchecked_mut(block);
            let prev = *word & (1 << i) != 0;
            *word |= 1 << i;
            prev
        }
    }

    /// **Panics** if **bit** is out of bounds.
    #[inline]
    pub fn set(&mut self, bit: usize, enabled: bool)
    {
        assert!(bit < self.length);
        let (block, i) = div_rem(bit, BITS);
        unsafe {
            let elt = self.data.get_unchecked_mut(block);
            if enabled {
                *elt |= 1 << i;
            } else {
                *elt &= !(1 << i);
            }
        }
    }

    /// Copies boolean value from specified bit to the specified bit.
    ///
    /// **Panics** if **to** is out of bounds.
    #[inline]
    pub fn copy_bit(&mut self, from: usize, to: usize)
    {
        assert!(to < self.length);
        let (to_block, t) = div_rem(to, BITS);
        let enabled = self.contains(from);
        unsafe {
            let to_elt = self.data.get_unchecked_mut(to_block);
            if enabled {
                *to_elt |= 1 << t;
            } else {
                *to_elt &= !(1 << t);
            }
        }
    }

    /// Count the number of set bits in the given bit range.
    ///
    /// Use `..` to count the whole content of the bitset.
    ///
    /// **Panics** if the range extends past the end of the bitset.
    #[inline]
    pub fn count_ones<T: IndexRange>(&self, range: T) -> usize
    {
        Masks::new(range, self.length)
            .map(|(block, mask)| unsafe {
                let value = *self.data.get_unchecked(block);
                (value & mask).count_ones() as usize
            })
            .sum()
    }

    /// Sets every bit in the given range to the given state (`enabled`)
    ///
    /// Use `..` to toggle the whole bitset.
    ///
    /// **Panics** if the range extends past the end of the bitset.
    #[inline]
    pub fn set_range<T: IndexRange>(&mut self, range: T, enabled: bool)
    {
        for (block, mask) in Masks::new(range, self.length) {
            unsafe {
                if enabled {
                    *self.data.get_unchecked_mut(block) |= mask;
                } else {
                    *self.data.get_unchecked_mut(block) &= !mask;
                }
            }
        }
    }

    /// Enables every bit in the given range.
    ///
    /// Use `..` to make the whole bitset ones.
    ///
    /// **Panics** if the range extends past the end of the bitset.
    #[inline]
    pub fn insert_range<T: IndexRange>(&mut self, range: T)
    {
        self.set_range(range, true);
    }

    /// View the bitset as a slice of `u32` blocks
    #[inline]
    pub fn as_slice(&self) -> &[u32]
    {
        &self.data
    }

    /// View the bitset as a mutable slice of `u32` blocks. Writing past the bitlength in the last
    /// will cause `contains` to return potentially incorrect results for bits past the bitlength.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u32]
    {
        &mut self.data
    }

    /// Iterates over all enabled bits.
    ///
    /// Iterator element is the index of the `1` bit, type `usize`.
    #[inline]
    pub fn ones(&self) -> Ones {
        match self.as_slice().split_first() {
            Some((&block, rem)) => {
                Ones {
                    current_bit_idx: 0,
                    current_block_idx: 0,
                    current_block: block,
                    remaining_blocks: rem
                }
            }
            None => {
                Ones {
                    current_bit_idx: 0,
                    current_block_idx: 0,
                    current_block: 0,
                    remaining_blocks: &[]
                }
            }
        }
    }

    /// Returns a lazy iterator over the intersection of two `FixedBitSet`s
    pub fn intersection<'a>(&'a self, other: &'a FixedBitSet) -> Intersection<'a>
    {
        Intersection {
            iter: self.ones(),
            other: other,
        }
    }

    /// Returns a lazy iterator over the union of two `FixedBitSet`s.
    pub fn union<'a>(&'a self, other: &'a FixedBitSet) -> Union<'a>
    {
        Union {
            iter: self.ones().chain(other.difference(self)),
        }
    }

    /// Returns a lazy iterator over the difference of two `FixedBitSet`s. The difference of `a`
    /// and `b` is the elements of `a` which are not in `b`.
    pub fn difference<'a>(&'a self, other: &'a FixedBitSet) -> Difference<'a>
    {
        Difference {
            iter: self.ones(),
            other: other,
        }
    }
}

/// An iterator producing elements in the difference of two sets.
///
/// This struct is created by the [`FixedBitSet::difference`] method.
pub struct Difference<'a> {
    iter: Ones<'a>,
    other: &'a FixedBitSet,
}

impl<'a> Iterator for Difference<'a> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(nxt) = self.iter.next() {
            if !self.other.contains(nxt) {
                return Some(nxt);
            }
        }
        None
    }
}


/// An iterator producing elements in the intersection of two sets.
///
/// This struct is created by the [`FixedBitSet::intersection`] method.
pub struct Intersection<'a> {
    iter: Ones<'a>,
    other: &'a FixedBitSet,
}

impl<'a> Iterator for Intersection<'a> {
    type Item = usize; // the bit position of the '1'

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(nxt) = self.iter.next() {
            if self.other.contains(nxt) {
                return Some(nxt);
            }
        }
        None
    }
}

/// An iterator producing elements in the union of two sets.
///
/// This struct is created by the [`FixedBitSet::union`] method.
pub struct Union<'a> {
    iter: Chain<Ones<'a>, Difference<'a>>,
}

impl<'a> Iterator for Union<'a> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}


struct Masks {
    first_block: usize,
    first_mask: Block,
    last_block: usize,
    last_mask: Block,
}

impl Masks {
    #[inline]
    fn new<T: IndexRange>(range: T, length: usize) -> Masks {
        let start = range.start().unwrap_or(0);
        let end = range.end().unwrap_or(length);
        assert!(start <= end && end <= length);

        let (first_block, first_rem) = div_rem(start, BITS);
        let (last_block, last_rem) = div_rem(end, BITS);

        Masks {
            first_block: first_block as usize,
            first_mask: Block::max_value() << first_rem,
            last_block: last_block as usize,
            last_mask: (Block::max_value() >> 1) >> (BITS - last_rem - 1),
            // this is equivalent to `MAX >> (BITS - x)` with correct semantics when x == 0.
        }
    }
}

impl Iterator for Masks {
    type Item = (usize, Block);
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.first_block.cmp(&self.last_block) {
            Ordering::Less => {
                let res = (self.first_block, self.first_mask);
                self.first_block += 1;
                self.first_mask = !0;
                Some(res)
            }
            Ordering::Equal => {
                let mask = self.first_mask & self.last_mask;
                let res = if mask == 0 {
                    None
                } else {
                    Some((self.first_block, mask))
                };
                self.first_block += 1;
                res
            }
            Ordering::Greater => None,
        }
    }
}


/// An  iterator producing the indices of the set bit in a set.
///
/// This struct is created by the [`FixedBitSet::ones`] method.
pub struct Ones<'a> {
    current_bit_idx: usize,
    current_block_idx: usize,
    remaining_blocks: &'a [Block],
    current_block: Block
}

impl<'a> Iterator for Ones<'a> {
    type Item = usize; // the bit position of the '1'

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let mut block = self.current_block;
        let mut idx = self.current_bit_idx;

        loop {
            loop {
                if (block & 1) == 1 {
                    self.current_block = block >> 1;
                    self.current_bit_idx = idx + 1;
                    return Some(idx);
                }
                // reordering the two lines below makes a huge (2x) difference in performance!
                block = block >> 1;
                idx += 1;
                if block == 0 {
                    break;
                }
            }

            // go to next block
            match self.remaining_blocks.split_first() {
                Some((&next_block, rest)) => {
                    self.remaining_blocks = rest;
                    self.current_block_idx += 1;
                    idx = self.current_block_idx * BITS;
                    block = next_block;
                }
                None => {
                    // last block => done
                    return None;
                }
            }
        }
    }
}

impl Clone for FixedBitSet
{
    #[inline]
    fn clone(&self) -> Self
    {
        FixedBitSet {
            data: self.data.clone(),
            length: self.length,
        }
    }
}

/// Return **true** if the bit is enabled in the bitset,
/// or **false** otherwise.
///
/// Note: bits outside the capacity are always disabled, and thus
/// indexing a FixedBitSet will not panic.
impl Index<usize> for FixedBitSet
{
    type Output = bool;

    #[inline]
    fn index(&self, bit: usize) -> &bool
    {
        if self.contains(bit) {
            &TRUE
        } else {
            &FALSE
        }
    }
}

/// Sets the bit at index **i** to **true** for each item **i** in the input **src**.
impl Extend<usize> for FixedBitSet
{
    fn extend<I: IntoIterator<Item=usize>>(&mut self, src: I) {
        let iter = src.into_iter();
        for i in iter {
            if i >= self.len() {
                self.grow(i + 1);
            }
            self.put(i);
        }
    }
}

/// Return a FixedBitSet containing bits set to **true** for every bit index in
/// the iterator, other bits are set to **false**.
impl FromIterator<usize> for FixedBitSet
{
    fn from_iter<I: IntoIterator<Item=usize>>(src: I) -> Self {
        let mut fbs = FixedBitSet::with_capacity(0);
        fbs.extend(src);
        fbs
    }
}

impl <'a> BitAnd for &'a FixedBitSet
{
    type Output = FixedBitSet;
    fn bitand(self, other: &FixedBitSet) -> FixedBitSet {
        let (short, long) = {
            if self.len() <= other.len() {
                (&self.data, &other.data)
            } else {
                (&other.data, &self.data)
            }
        };
        let mut data = short.clone();
        for (i, &block) in long.iter().take(short.len()).enumerate() {
            data[i] &= block;
        }
        let len = std::cmp::min(self.len(), other.len());
        FixedBitSet{data: data, length: len}
    }
}

impl <'a> BitOr for &'a FixedBitSet
{
    type Output = FixedBitSet;
    fn bitor(self, other: &FixedBitSet) -> FixedBitSet {
        let (short, long) = {
            if self.len() <= other.len() {
                (&self.data, &other.data)
            } else {
                (&other.data, &self.data)
            }
        };
        let mut data = long.clone();
        for (i, &block) in short.iter().enumerate() {
            data[i] |= block;
        }
        let len = std::cmp::max(self.len(), other.len());
        FixedBitSet{data: data, length: len}
    }
}

#[test]
fn it_works() {
    const N: usize = 50;
    let mut fb = FixedBitSet::with_capacity(N);
    println!("{:?}", fb);

    for i in 0..(N + 10) {
        assert_eq!(fb.contains(i), false);
    }

    fb.insert(10);
    fb.set(11, false);
    fb.set(12, false);
    fb.set(12, true);
    fb.set(N-1, true);
    println!("{:?}", fb);
    assert!(fb.contains(10));
    assert!(!fb.contains(11));
    assert!(fb.contains(12));
    assert!(fb.contains(N-1));
    for i in 0..N {
        let contain = i == 10 || i == 12 || i == N - 1;
        assert_eq!(contain, fb[i]);
    }

    fb.clear();
}

#[test]
fn grow() {
    let mut fb = FixedBitSet::with_capacity(48);
    for i in 0..fb.len() {
        fb.set(i, true);
    }

    let old_len = fb.len();
    fb.grow(72);
    for j in 0..fb.len() {
        assert_eq!(fb.contains(j), j < old_len);
    }
    fb.set(64, true);
    assert!(fb.contains(64));
}

#[test]
fn copy_bit() {
    let mut fb = FixedBitSet::with_capacity(48);
    for i in 0..fb.len() {
        fb.set(i, true);
    }
    fb.set(42, false);
    fb.copy_bit(42, 2);
    assert!(!fb.contains(42));
    assert!(!fb.contains(2));
    assert!(fb.contains(1));
    fb.copy_bit(1, 42);
    assert!(fb.contains(42));
    fb.copy_bit(1024, 42);
    assert!(!fb[42]);
}

#[test]
fn count_ones() {
    let mut fb = FixedBitSet::with_capacity(100);
    fb.set(11, true);
    fb.set(12, true);
    fb.set(7, true);
    fb.set(35, true);
    fb.set(40, true);
    fb.set(77, true);
    fb.set(95, true);
    fb.set(50, true);
    fb.set(99, true);
    assert_eq!(fb.count_ones(..7), 0);
    assert_eq!(fb.count_ones(..8), 1);
    assert_eq!(fb.count_ones(..11), 1);
    assert_eq!(fb.count_ones(..12), 2);
    assert_eq!(fb.count_ones(..13), 3);
    assert_eq!(fb.count_ones(..35), 3);
    assert_eq!(fb.count_ones(..36), 4);
    assert_eq!(fb.count_ones(..40), 4);
    assert_eq!(fb.count_ones(..41), 5);
    assert_eq!(fb.count_ones(50..), 4);
    assert_eq!(fb.count_ones(70..95), 1);
    assert_eq!(fb.count_ones(70..96), 2);
    assert_eq!(fb.count_ones(70..99), 2);
    assert_eq!(fb.count_ones(..), 9);
    assert_eq!(fb.count_ones(0..100), 9);
    assert_eq!(fb.count_ones(0..0), 0);
    assert_eq!(fb.count_ones(100..100), 0);
    assert_eq!(fb.count_ones(7..), 9);
    assert_eq!(fb.count_ones(8..), 8);
}

#[test]
fn ones() {
    let mut fb = FixedBitSet::with_capacity(100);
    fb.set(11, true);
    fb.set(12, true);
    fb.set(7, true);
    fb.set(35, true);
    fb.set(40, true);
    fb.set(77, true);
    fb.set(95, true);
    fb.set(50, true);
    fb.set(99, true);

    let ones: Vec<_> = fb.ones().collect();

    assert_eq!(vec![7, 11, 12, 35, 40, 50, 77, 95, 99], ones);
}

#[test]
fn iter_ones_range() {
    fn test_range(from: usize, to: usize, capa: usize) {
        assert!(to <= capa);
        let mut fb = FixedBitSet::with_capacity(capa);
        for i in from..to {
            fb.insert(i);
        }
        let ones: Vec<_> = fb.ones().collect();
        let expected: Vec<_> = (from..to).collect();
        assert_eq!(expected, ones);
    }

    for i in 0..100 {
      test_range(i, 100, 100);
      test_range(0, i, 100);
    }
}

#[should_panic]
#[test]
fn count_ones_oob() {
    let fb = FixedBitSet::with_capacity(100);
    fb.count_ones(90..101);
}

#[should_panic]
#[test]
fn count_ones_negative_range() {
    let fb = FixedBitSet::with_capacity(100);
    fb.count_ones(90..80);
}

#[test]
fn count_ones_panic() {
    for i in 1..128 {
        let fb = FixedBitSet::with_capacity(i);
        for j in 0..fb.len() + 1 {
            for k in j..fb.len() + 1 {
                assert_eq!(fb.count_ones(j..k), 0);
            }
        }
    }
}


#[test]
fn default() {
    let fb = FixedBitSet::default();
    assert_eq!(fb.len(), 0);
}

#[test]
fn insert_range() {
    let mut fb = FixedBitSet::with_capacity(97);
    fb.insert_range(..3);
    fb.insert_range(9..32);
    fb.insert_range(37..81);
    fb.insert_range(90..);
    for i in 0..97 {
        assert_eq!(fb.contains(i), i<3 || 9<=i&&i<32 || 37<=i&&i<81 || 90<=i);
    }
    assert!(!fb.contains(97));
    assert!(!fb.contains(127));
    assert!(!fb.contains(128));
}

#[test]
fn set_range() {
    let mut fb = FixedBitSet::with_capacity(48);
    fb.insert_range(..);

    fb.set_range(..32, false);
    fb.set_range(37.., false);
    fb.set_range(5..9, true);
    fb.set_range(40..40, true);

    for i in 0..48 {
        assert_eq!(fb.contains(i), 5<=i&&i<9 || 32<=i&&i<37);
    }
    assert!(!fb.contains(48));
    assert!(!fb.contains(64));
}

#[test]
fn bitand_equal_lengths() {
    let len = 109;
    let a_end = 59;
    let b_start = 23;
    let mut a = FixedBitSet::with_capacity(len);
    let mut b = FixedBitSet::with_capacity(len);
    a.set_range(..a_end, true);
    b.set_range(b_start.., true);
    let ab = &a & &b;
    for i in 0..b_start {
        assert!(!ab.contains(i));
    }
    for i in b_start..a_end {
        assert!(ab.contains(i));
    }
    for i in a_end..len {
        assert!(!ab.contains(i));
    }
    assert_eq!(a.len(), ab.len());
}

#[test]
fn bitand_first_smaller() {
    let a_len = 113;
    let b_len = 137;
    let len = std::cmp::min(a_len, b_len);
    let a_end = 97;
    let b_start = 89;
    let mut a = FixedBitSet::with_capacity(a_len);
    let mut b = FixedBitSet::with_capacity(b_len);
    a.set_range(..a_end, true);
    b.set_range(b_start.., true);
    let ab = &a & &b;
    for i in 0..b_start {
        assert!(!ab.contains(i));
    }
    for i in b_start..a_end {
        assert!(ab.contains(i));
    }
    for i in a_end..len {
        assert!(!ab.contains(i));
    }
    assert_eq!(a.len(), ab.len());
}

#[test]
fn bitand_first_larger() {
    let a_len = 173;
    let b_len = 137;
    let len = std::cmp::min(a_len, b_len);
    let a_end = 107;
    let b_start = 43;
    let mut a = FixedBitSet::with_capacity(a_len);
    let mut b = FixedBitSet::with_capacity(b_len);
    a.set_range(..a_end, true);
    b.set_range(b_start.., true);
    let ab = &a & &b;
    for i in 0..b_start {
        assert!(!ab.contains(i));
    }
    for i in b_start..a_end {
        assert!(ab.contains(i));
    }
    for i in a_end..len {
        assert!(!ab.contains(i));
    }
    assert_eq!(b.len(), ab.len());
}

#[test]
fn intersection() {
    let len = 109;
    let a_end = 59;
    let b_start = 23;
    let mut a = FixedBitSet::with_capacity(len);
    let mut b = FixedBitSet::with_capacity(len);
    a.set_range(..a_end, true);
    b.set_range(b_start.., true);

    let ab = a.intersection(&b).collect::<FixedBitSet>();

    for i in 0..b_start {
        assert!(!ab.contains(i));
    }
    for i in b_start..a_end {
        assert!(ab.contains(i));
    }
    for i in a_end..len {
        assert!(!ab.contains(i));
    }
}

#[test]
fn union() {
    let a_len = 173;
    let b_len = 137;
    let a_start = 139;
    let b_end = 107;
    let mut a = FixedBitSet::with_capacity(a_len);
    let mut b = FixedBitSet::with_capacity(b_len);
    a.set_range(a_start.., true);
    b.set_range(..b_end, true);
    let ab = a.union(&b).collect::<FixedBitSet>();
    for i in a_start..a_len {
        assert!(ab.contains(i));
    }
    for i in 0..b_end {
        assert!(ab.contains(i));
    }
    for i in b_end..a_start {
        assert!(!ab.contains(i));
    }
}

#[test]
fn difference() {
    let a_len = 83;
    let b_len = 151;
    let a_start = 0;
    let a_end = 79;
    let b_start = 53;
    let mut a = FixedBitSet::with_capacity(a_len);
    let mut b = FixedBitSet::with_capacity(b_len);
    a.set_range(a_start..a_end, true);
    b.set_range(b_start..b_len, true);
    let a_diff_b = a.difference(&b).collect::<FixedBitSet>();
    for i in a_start..b_start {
        assert!(a_diff_b.contains(i));
    }
    for i in b_start..b_len {
        assert!(!a_diff_b.contains(i));
    }
}

#[test]
fn bitor_equal_lengths() {
    let len = 109;
    let a_start = 17;
    let a_end = 23;
    let b_start = 19;
    let b_end = 59;
    let mut a = FixedBitSet::with_capacity(len);
    let mut b = FixedBitSet::with_capacity(len);
    a.set_range(a_start..a_end, true);
    b.set_range(b_start..b_end, true);
    let ab = &a | &b;
    for i in 0..a_start {
        assert!(!ab.contains(i));
    }
    for i in a_start..b_end {
        assert!(ab.contains(i));
    }
    for i in b_end..len {
        assert!(!ab.contains(i));
    }
    assert_eq!(ab.len(), len);
}

#[test]
fn bitor_first_smaller() {
    let a_len = 113;
    let b_len = 137;
    let a_end = 89;
    let b_start = 97;
    let mut a = FixedBitSet::with_capacity(a_len);
    let mut b = FixedBitSet::with_capacity(b_len);
    a.set_range(..a_end, true);
    b.set_range(b_start.., true);
    let ab = &a | &b;
    for i in 0..a_end {
        assert!(ab.contains(i));
    }
    for i in a_end..b_start {
        assert!(!ab.contains(i));
    }
    for i in b_start..b_len {
        assert!(ab.contains(i));
    }
    assert_eq!(b_len, ab.len());
}

#[test]
fn bitor_first_larger() {
    let a_len = 173;
    let b_len = 137;
    let a_start = 139;
    let b_end = 107;
    let mut a = FixedBitSet::with_capacity(a_len);
    let mut b = FixedBitSet::with_capacity(b_len);
    a.set_range(a_start.., true);
    b.set_range(..b_end, true);
    let ab = &a | &b;
    for i in a_start..a_len {
        assert!(ab.contains(i));
    }
    for i in 0..b_end {
        assert!(ab.contains(i));
    }
    for i in b_end..a_start {
        assert!(!ab.contains(i));
    }
    assert_eq!(a_len, ab.len());
}

#[test]
fn extend_on_empty() {
    let items: Vec<usize> = vec![2, 3, 5, 7, 11, 13, 17, 19, 23, 27, 29, 31, 37, 167];
    let mut fbs = FixedBitSet::with_capacity(0);
    fbs.extend(items.iter().cloned());
    let ones = fbs.ones().collect::<Vec<usize>>();
    assert!(ones == items);
}

#[test]
fn extend() {
    let items: Vec<usize> = vec![2, 3, 5, 7, 11, 13, 17, 19, 23, 27, 29, 31, 37, 167];
    let mut fbs = FixedBitSet::with_capacity(168);
    let new: Vec<usize> = vec![7, 37, 67, 137];
    for i in &new {
        fbs.put(*i);
    }

    fbs.extend(items.iter().cloned());

    let ones = fbs.ones().collect::<Vec<usize>>();
    let expected = {
        let mut tmp = items.clone();
        tmp.extend(new);
        tmp.sort();
        tmp.dedup();
        tmp
    };

    assert!(ones == expected);
}

#[test]
fn from_iterator() {
    let items: Vec<usize> = vec![0, 2, 4, 6, 8];
    let fb = items.iter().cloned().collect::<FixedBitSet>();
    for i in items {
        assert!(fb.contains(i));
    }
    for i in vec![1, 3, 5, 7] {
        assert!(!fb.contains(i));
    }
    assert_eq!(fb.len(), 9);
}

#[test]
fn from_iterator_ones() {
    let len = 257;
    let mut fb = FixedBitSet::with_capacity(len);
    for i in (0..len).filter(|i| i % 7 == 0) {
        fb.put(i);
    }
    fb.put(len - 1);
    let dup = fb.ones().collect::<FixedBitSet>();
    println!("{0:?}\n{1:?}", fb, dup);
    println!("{0:?}\n{1:?}", fb.ones().collect::<Vec<usize>>(), dup.ones().collect::<Vec<usize>>());
    assert_eq!(fb.len(), dup.len());
    assert_eq!(fb.ones().collect::<Vec<usize>>(), dup.ones().collect::<Vec<usize>>());
}
