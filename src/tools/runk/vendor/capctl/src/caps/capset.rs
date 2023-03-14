use core::fmt;
use core::iter::FromIterator;
use core::ops::{
    BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, Sub, SubAssign,
};

use super::{Cap, CAP_BITMASK, NUM_CAPS};

/// Represents a set of capabilities.
///
/// Internally, this stores the set of capabilities as a bitmask, which is much more efficient than
/// a `HashSet<Cap>`.
#[derive(Copy, Clone, Eq, Hash, PartialEq)]
pub struct CapSet {
    pub(super) bits: u64,
}

impl CapSet {
    /// Create an empty capability set.
    #[inline]
    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    /// Clear all capabilities from this set.
    ///
    /// After this call, `set.is_empty()` will return `true`.
    #[inline]
    pub fn clear(&mut self) {
        self.bits = 0;
    }

    /// Check if this capability set is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bits == 0
    }

    /// Returns the number of capabilities in this capability set.
    #[inline]
    pub fn size(&self) -> usize {
        self.bits.count_ones() as usize
    }

    /// Checks if a given capability is present in this set.
    #[inline]
    pub fn has(&self, cap: Cap) -> bool {
        self.bits & cap.to_single_bitfield() != 0
    }

    /// Adds the given capability to this set.
    #[inline]
    pub fn add(&mut self, cap: Cap) {
        self.bits |= cap.to_single_bitfield();
    }

    /// Removes the given capability from this set.
    #[inline]
    pub fn drop(&mut self, cap: Cap) {
        self.bits &= !cap.to_single_bitfield();
    }

    /// If `val` is `true` the given capability is added; otherwise it is removed.
    pub fn set_state(&mut self, cap: Cap, val: bool) {
        if val {
            self.add(cap);
        } else {
            self.drop(cap);
        }
    }

    /// Adds all of the capabilities yielded by the given iterator to this set.
    ///
    /// If you want to add all the capabilities in another `CapSet`, you should use
    /// `set1 = set1.union(set2)` or `set1 |= set2`, NOT `set1.add_all(set2)`.
    pub fn add_all<T: IntoIterator<Item = Cap>>(&mut self, t: T) {
        for cap in t.into_iter() {
            self.add(cap);
        }
    }

    /// Removes all of the capabilities yielded by the given iterator from this set.
    ///
    /// If you want to remove all the capabilities in another `CapSet`, you should use
    /// `set1 = set1.intersection(!set2)`, `set1 &= !set2`, or `set1 -= set2`, NOT
    /// `set1.drop_all(set2)`.
    pub fn drop_all<T: IntoIterator<Item = Cap>>(&mut self, t: T) {
        for cap in t.into_iter() {
            self.drop(cap);
        }
    }

    /// Returns an iterator over all of the capabilities in this set.
    #[inline]
    pub fn iter(&self) -> CapSetIterator {
        self.into_iter()
    }

    /// Returns the union of this set and another capability set (i.e. all the capabilities that
    /// are in either set).
    #[inline]
    pub const fn union(&self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    /// Returns the intersection of this set and another capability set (i.e. all the capabilities
    /// that are in both sets).
    #[inline]
    pub const fn intersection(&self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }

    /// WARNING: This is an internal method and its signature may change in the future. Use [the
    /// `capset!()` macro] instead.
    ///
    /// [the `capset!()` macro]: ../macro.capset.html
    #[doc(hidden)]
    #[inline]
    pub const fn from_bitmask_truncate(bitmask: u64) -> Self {
        Self {
            bits: bitmask & CAP_BITMASK,
        }
    }

    #[inline]
    pub(crate) fn from_bitmasks_u32(lower: u32, upper: u32) -> Self {
        Self::from_bitmask_truncate(((upper as u64) << 32) | (lower as u64))
    }
}

impl Default for CapSet {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

impl Not for CapSet {
    type Output = Self;

    #[inline]
    fn not(self) -> Self::Output {
        Self {
            bits: (!self.bits) & CAP_BITMASK,
        }
    }
}

impl BitAnd for CapSet {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        self.intersection(rhs)
    }
}

impl BitOr for CapSet {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl BitXor for CapSet {
    type Output = Self;

    #[inline]
    fn bitxor(self, rhs: Self) -> Self::Output {
        Self {
            bits: self.bits ^ rhs.bits,
        }
    }
}

impl Sub for CapSet {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            bits: self.bits & (!rhs.bits),
        }
    }
}

impl BitAndAssign for CapSet {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs;
    }
}

impl BitOrAssign for CapSet {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl BitXorAssign for CapSet {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        *self = *self ^ rhs;
    }
}

impl SubAssign for CapSet {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Extend<Cap> for CapSet {
    #[inline]
    fn extend<I: IntoIterator<Item = Cap>>(&mut self, it: I) {
        self.add_all(it);
    }
}

impl FromIterator<Cap> for CapSet {
    #[inline]
    fn from_iter<I: IntoIterator<Item = Cap>>(it: I) -> Self {
        let mut res = Self::empty();
        res.extend(it);
        res
    }
}

impl IntoIterator for CapSet {
    type Item = Cap;
    type IntoIter = CapSetIterator;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        CapSetIterator { set: self, i: 0 }
    }
}

impl fmt::Debug for CapSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

/// A helper macro to statically construct a `CapSet` from a list of capabilities.
///
/// Examples:
/// ```
/// # use core::iter::FromIterator;
/// # use capctl::capset;
/// # use capctl::caps::{Cap, CapSet};
/// assert_eq!(capset!(), CapSet::empty());
/// assert_eq!(capset!(Cap::CHOWN), [Cap::CHOWN].iter().cloned().collect());
/// assert_eq!(capset!(Cap::CHOWN, Cap::SYSLOG), [Cap::CHOWN, Cap::SYSLOG].iter().cloned().collect());
/// ```
///
/// Note that you cannot use raw integers, only `Cap` variants. For example, this is not allowed:
///
/// ```compile_fail
/// # use capctl::capset;
/// # use capctl::caps::{Cap, CapSet};
/// assert_eq!(capset!(0), capset!(Cap::CHOWN));
/// ```
#[macro_export]
macro_rules! capset {
    () => {
        $crate::caps::CapSet::empty()
    };

    ($($caps:expr),+ $(,)?) => {
        $crate::caps::CapSet::from_bitmask_truncate(
            0 $(| (1u64 << ($caps as $crate::caps::Cap as u8)))*
        )
    };
}

/// An iterator over all the capabilities in a `CapSet`.
///
/// This is constructed by [`CapSet::iter()`].
///
/// [`CapSet::iter()`]: ./struct.CapSet.html#method.iter
#[derive(Clone)]
pub struct CapSetIterator {
    set: CapSet,
    i: u8,
}

impl Iterator for CapSetIterator {
    type Item = Cap;

    fn next(&mut self) -> Option<Cap> {
        while let Some(cap) = Cap::from_u8(self.i) {
            self.i += 1;
            if self.set.has(cap) {
                return Some(cap);
            }
        }

        None
    }

    #[inline]
    fn last(self) -> Option<Cap> {
        // This calculates the position of the largest bit that is set.
        // For example, if the bitmask is 0b10101, n=5.
        let n = core::mem::size_of::<u64>() as u8 * 8 - self.set.bits.leading_zeros() as u8;

        if self.i < n {
            // We haven't yet passed the largest bit.
            // This uses `<` instead of `<=` because `self.i` and `n` are off by 1 (so we also have
            // to subtract 1 below).

            let res = Cap::from_u8(n - 1);
            debug_assert!(res.is_some());
            res
        } else {
            None
        }
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl ExactSizeIterator for CapSetIterator {
    #[inline]
    fn len(&self) -> usize {
        // It should be literally impossible for i to be out of this range
        debug_assert!(self.i <= NUM_CAPS);

        (self.set.bits >> self.i).count_ones() as usize
    }
}

impl core::iter::FusedIterator for CapSetIterator {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capset_empty() {
        let mut set = Cap::iter().collect::<CapSet>();
        for cap in Cap::iter() {
            set.drop(cap);
        }
        assert_eq!(set.bits, 0);
        assert!(set.is_empty());

        set = CapSet::empty();
        assert_eq!(set.bits, 0);
        assert!(set.is_empty());
        assert_eq!(set, CapSet::default());

        set = Cap::iter().collect::<CapSet>();
        set.clear();
        assert_eq!(set.bits, 0);
        assert!(set.is_empty());

        assert!(!Cap::iter().any(|c| set.has(c)));
    }

    #[test]
    fn test_capset_full() {
        let mut set = CapSet::empty();
        for cap in Cap::iter() {
            set.add(cap);
        }
        assert_eq!(set.bits, CAP_BITMASK);
        assert!(!set.is_empty());

        set = CapSet::empty();
        set.extend(Cap::iter());
        assert_eq!(set.bits, CAP_BITMASK);
        assert!(!set.is_empty());

        assert!(Cap::iter().all(|c| set.has(c)));
    }

    #[test]
    fn test_capset_add_drop() {
        let mut set = CapSet::empty();
        set.add(Cap::CHOWN);
        assert!(set.has(Cap::CHOWN));
        assert!(!set.is_empty());

        set.drop(Cap::CHOWN);
        assert!(!set.has(Cap::CHOWN));
        assert!(set.is_empty());

        set.set_state(Cap::CHOWN, true);
        assert!(set.has(Cap::CHOWN));
        assert!(!set.is_empty());

        set.set_state(Cap::CHOWN, false);
        assert!(!set.has(Cap::CHOWN));
        assert!(set.is_empty());
    }

    #[test]
    fn test_capset_add_drop_all() {
        let mut set = CapSet::empty();
        set.add_all([Cap::FOWNER, Cap::CHOWN, Cap::KILL].iter().cloned());

        // Iteration order is not preserved, but it should be consistent.
        assert!(set
            .into_iter()
            .eq([Cap::CHOWN, Cap::FOWNER, Cap::KILL].iter().cloned()));
        assert!(set
            .into_iter()
            .eq([Cap::CHOWN, Cap::FOWNER, Cap::KILL].iter().cloned()));

        set.drop_all([Cap::FOWNER, Cap::CHOWN].iter().cloned());
        assert!(set.iter().eq([Cap::KILL].iter().cloned()));

        set.drop_all([Cap::KILL].iter().cloned());
        assert!(set.iter().eq([].iter().cloned()));
    }

    #[test]
    fn test_capset_from_iter() {
        let set = [Cap::CHOWN, Cap::FOWNER]
            .iter()
            .cloned()
            .collect::<CapSet>();
        assert!(set.iter().eq([Cap::CHOWN, Cap::FOWNER].iter().cloned()));
    }

    #[test]
    fn test_capset_iter_full() {
        assert!(Cap::iter().eq(CapSet { bits: CAP_BITMASK }.iter()));
        assert!(Cap::iter().eq(Cap::iter().collect::<CapSet>().iter()));
    }

    #[test]
    fn test_capset_iter_count() {
        for set in [
            [].iter().cloned().collect(),
            [Cap::CHOWN, Cap::FOWNER].iter().cloned().collect(),
            Cap::iter().collect::<CapSet>(),
        ]
        .iter()
        {
            let mut count = set.size();

            let mut it = set.iter();
            assert_eq!(it.len(), count);
            assert_eq!(it.clone().count(), count);
            assert_eq!(it.size_hint(), (count, Some(count)));

            while let Some(_cap) = it.next() {
                count -= 1;
                assert_eq!(it.len(), count);
                assert_eq!(it.clone().count(), count);
                assert_eq!(it.size_hint(), (count, Some(count)));
            }

            assert_eq!(count, 0);

            assert_eq!(it.len(), 0);
            assert_eq!(it.clone().count(), 0);
            assert_eq!(it.size_hint(), (0, Some(0)));
        }
    }

    #[test]
    fn test_capset_iter_last() {
        let last_cap = Cap::iter().last().unwrap();

        assert_eq!(
            Cap::iter().collect::<CapSet>().iter().last(),
            Some(last_cap)
        );
        assert_eq!(CapSet::empty().iter().last(), None);

        let mut it = Cap::iter().collect::<CapSet>().iter();
        assert_eq!(it.clone().last(), Some(last_cap));
        while it.next().is_some() {
            if it.clone().next().is_some() {
                assert_eq!(it.clone().last(), Some(last_cap));
            } else {
                assert_eq!(it.clone().last(), None);
            }
        }
        assert_eq!(it.len(), 0);
        assert_eq!(it.last(), None);

        it = capset!(Cap::FOWNER).iter();
        assert_eq!(it.clone().last(), Some(Cap::FOWNER));
        assert_eq!(it.next(), Some(Cap::FOWNER));
        assert_eq!(it.last(), None);

        it = capset!(Cap::CHOWN).iter();
        assert_eq!(it.clone().last(), Some(Cap::CHOWN));
        assert_eq!(it.next(), Some(Cap::CHOWN));
        assert_eq!(it.last(), None);

        it = capset!(Cap::CHOWN, Cap::FOWNER).iter();
        assert_eq!(it.clone().last(), Some(Cap::FOWNER));
        assert_eq!(it.next(), Some(Cap::CHOWN));
        assert_eq!(it.clone().last(), Some(Cap::FOWNER));
        assert_eq!(it.next(), Some(Cap::FOWNER));
        assert_eq!(it.clone().last(), None);
        assert_eq!(it.next(), None);
        assert_eq!(it.last(), None);
    }

    #[test]
    fn test_capset_union() {
        let a = [Cap::CHOWN, Cap::FOWNER]
            .iter()
            .cloned()
            .collect::<CapSet>();
        let b = [Cap::FOWNER, Cap::KILL].iter().cloned().collect::<CapSet>();
        let c = [Cap::CHOWN, Cap::FOWNER, Cap::KILL]
            .iter()
            .cloned()
            .collect::<CapSet>();
        assert_eq!(a.union(b), c);
    }

    #[test]
    fn test_capset_intersection() {
        let a = [Cap::CHOWN, Cap::FOWNER]
            .iter()
            .cloned()
            .collect::<CapSet>();
        let b = [Cap::FOWNER, Cap::KILL].iter().cloned().collect::<CapSet>();
        let c = [Cap::FOWNER].iter().cloned().collect::<CapSet>();
        assert_eq!(a.intersection(b), c);
    }

    #[test]
    fn test_capset_not() {
        assert_eq!(!Cap::iter().collect::<CapSet>(), CapSet::empty());
        assert_eq!(Cap::iter().collect::<CapSet>(), !CapSet::empty());

        let mut a = Cap::iter().collect::<CapSet>();
        let mut b = CapSet::empty();
        a.add(Cap::CHOWN);
        b.drop(Cap::CHOWN);
        assert_eq!(!a, b);
    }

    #[test]
    fn test_capset_bitor() {
        let a = [Cap::CHOWN, Cap::FOWNER]
            .iter()
            .cloned()
            .collect::<CapSet>();
        let b = [Cap::FOWNER, Cap::KILL].iter().cloned().collect::<CapSet>();
        let c = [Cap::CHOWN, Cap::FOWNER, Cap::KILL]
            .iter()
            .cloned()
            .collect::<CapSet>();
        assert_eq!(a | b, c);

        let mut d = a;
        d |= b;
        assert_eq!(d, c);
    }

    #[test]
    fn test_capset_bitand() {
        let a = [Cap::CHOWN, Cap::FOWNER]
            .iter()
            .cloned()
            .collect::<CapSet>();
        let b = [Cap::FOWNER, Cap::KILL].iter().cloned().collect::<CapSet>();
        let c = [Cap::FOWNER].iter().cloned().collect::<CapSet>();
        assert_eq!(a & b, c);

        let mut d = a;
        d &= b;
        assert_eq!(d, c);
    }

    #[test]
    fn test_capset_bitxor() {
        let a = [Cap::CHOWN, Cap::FOWNER]
            .iter()
            .cloned()
            .collect::<CapSet>();
        let b = [Cap::FOWNER, Cap::KILL].iter().cloned().collect::<CapSet>();
        let c = [Cap::CHOWN, Cap::KILL].iter().cloned().collect::<CapSet>();
        assert_eq!(a ^ b, c);

        let mut d = a;
        d ^= b;
        assert_eq!(d, c);
    }

    #[test]
    fn test_capset_sub() {
        let a = [Cap::CHOWN, Cap::FOWNER]
            .iter()
            .cloned()
            .collect::<CapSet>();
        let b = [Cap::FOWNER, Cap::KILL].iter().cloned().collect::<CapSet>();
        let c = [Cap::CHOWN].iter().cloned().collect::<CapSet>();
        assert_eq!(a - b, c);

        let mut d = a;
        d -= b;
        assert_eq!(d, c);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_capset_fmt() {
        assert_eq!(format!("{:?}", CapSet::empty()), "{}");
        assert_eq!(
            format!("{:?}", [Cap::CHOWN].iter().cloned().collect::<CapSet>()),
            "{CHOWN}"
        );
        assert_eq!(
            format!(
                "{:?}",
                [Cap::CHOWN, Cap::FOWNER]
                    .iter()
                    .cloned()
                    .collect::<CapSet>()
            ),
            "{CHOWN, FOWNER}"
        );
    }

    #[test]
    fn test_capset_macro() {
        assert_eq!(capset!(), CapSet::empty());

        assert_eq!(capset!(Cap::CHOWN), [Cap::CHOWN].iter().cloned().collect());
        assert_eq!(capset!(Cap::CHOWN,), [Cap::CHOWN].iter().cloned().collect());

        for cap in Cap::iter() {
            assert_eq!(capset!(cap), [cap].iter().cloned().collect());
            assert_eq!(capset!(cap,), [cap].iter().cloned().collect());
        }

        assert_eq!(
            capset!(Cap::CHOWN, Cap::SYSLOG),
            [Cap::CHOWN, Cap::SYSLOG].iter().cloned().collect()
        );
        assert_eq!(
            capset!(Cap::CHOWN, Cap::SYSLOG,),
            [Cap::CHOWN, Cap::SYSLOG].iter().cloned().collect()
        );

        assert_eq!(
            capset!(Cap::CHOWN, Cap::SYSLOG, Cap::FOWNER),
            [Cap::CHOWN, Cap::SYSLOG, Cap::FOWNER]
                .iter()
                .cloned()
                .collect()
        );
        assert_eq!(
            capset!(Cap::CHOWN, Cap::SYSLOG, Cap::FOWNER,),
            [Cap::CHOWN, Cap::SYSLOG, Cap::FOWNER]
                .iter()
                .cloned()
                .collect()
        );

        const EMPTY_SET: CapSet = capset!();
        assert_eq!(EMPTY_SET, CapSet::empty());

        const CHOWN_SET: CapSet = capset!(Cap::CHOWN);
        assert_eq!(CHOWN_SET, [Cap::CHOWN].iter().cloned().collect());

        const CHOWN_SYSLOG_SET: CapSet = capset!(Cap::CHOWN, Cap::SYSLOG,);
        assert_eq!(
            CHOWN_SYSLOG_SET,
            [Cap::CHOWN, Cap::SYSLOG].iter().cloned().collect()
        );
    }
}
