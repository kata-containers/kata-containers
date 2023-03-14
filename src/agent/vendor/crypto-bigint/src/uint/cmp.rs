//! [`UInt`] comparisons.
//!
//! By default these are all constant-time and use the `subtle` crate.

use super::UInt;
use crate::{Limb, LimbInt, LimbUInt, WideLimbInt, Zero};
use core::cmp::Ordering;
use subtle::{Choice, ConstantTimeEq, ConstantTimeGreater, ConstantTimeLess};

impl<const LIMBS: usize> UInt<LIMBS> {
    /// Return `a` if `c`!=0 or `b` if `c`==0.
    ///
    /// Const-friendly: we can't yet use `subtle` in `const fn` contexts.
    #[inline]
    pub(crate) const fn ct_select(a: UInt<LIMBS>, b: UInt<LIMBS>, c: LimbUInt) -> Self {
        let mut limbs = [Limb::ZERO; LIMBS];

        let mut i = 0;
        while i < LIMBS {
            limbs[i] = Limb::ct_select(a.limbs[i], b.limbs[i], c);
            i += 1;
        }

        UInt { limbs }
    }

    /// Returns all 1's if `self`!=0 or 0 if `self`==0.
    ///
    /// Const-friendly: we can't yet use `subtle` in `const fn` contexts.
    #[inline]
    pub(crate) const fn ct_is_nonzero(&self) -> LimbUInt {
        let mut b = 0;
        let mut i = 0;
        while i < LIMBS {
            b |= self.limbs[i].0;
            i += 1;
        }
        Limb::is_nonzero(Limb(b))
    }

    /// Returns -1 if self < rhs
    ///          0 if self == rhs
    ///          1 if self > rhs
    ///
    /// Const-friendly: we can't yet use `subtle` in `const fn` contexts.
    #[inline]
    pub(crate) const fn ct_cmp(&self, rhs: &Self) -> LimbInt {
        let mut gt = 0;
        let mut lt = 0;
        let mut i = LIMBS;

        while i > 0 {
            let a = self.limbs[i - 1].0 as WideLimbInt;
            let b = rhs.limbs[i - 1].0 as WideLimbInt;
            gt |= ((b - a) >> Limb::BIT_SIZE) & 1 & !lt;
            lt |= ((a - b) >> Limb::BIT_SIZE) & 1 & !gt;
            i -= 1;
        }
        (gt as LimbInt) - (lt as LimbInt)
    }
}

impl<const LIMBS: usize> ConstantTimeEq for UInt<LIMBS> {
    fn ct_eq(&self, other: &Self) -> Choice {
        self.limbs
            .iter()
            .zip(other.limbs.iter())
            .fold(Choice::from(1), |acc, (a, b)| acc & a.ct_eq(b))
    }
}

impl<const LIMBS: usize> ConstantTimeGreater for UInt<LIMBS> {
    fn ct_gt(&self, other: &Self) -> Choice {
        let underflow = other.sbb(self, Limb::ZERO).1;
        !underflow.is_zero()
    }
}

impl<const LIMBS: usize> ConstantTimeLess for UInt<LIMBS> {
    fn ct_lt(&self, other: &Self) -> Choice {
        let underflow = self.sbb(other, Limb::ZERO).1;
        !underflow.is_zero()
    }
}

impl<const LIMBS: usize> Eq for UInt<LIMBS> {}

impl<const LIMBS: usize> Ord for UInt<LIMBS> {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut n = 0i8;
        n -= self.ct_lt(other).unwrap_u8() as i8;
        n += self.ct_gt(other).unwrap_u8() as i8;

        match n {
            -1 => Ordering::Less,
            1 => Ordering::Greater,
            _ => {
                debug_assert_eq!(n, 0);
                debug_assert!(bool::from(self.ct_eq(other)));
                Ordering::Equal
            }
        }
    }
}

impl<const LIMBS: usize> PartialOrd for UInt<LIMBS> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<const LIMBS: usize> PartialEq for UInt<LIMBS> {
    fn eq(&self, other: &Self) -> bool {
        self.ct_eq(other).into()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Integer, Zero, U128};
    use subtle::{ConstantTimeEq, ConstantTimeGreater, ConstantTimeLess};

    #[test]
    fn is_zero() {
        assert!(bool::from(U128::ZERO.is_zero()));
        assert!(!bool::from(U128::ONE.is_zero()));
        assert!(!bool::from(U128::MAX.is_zero()));
    }

    #[test]
    fn is_odd() {
        assert!(!bool::from(U128::ZERO.is_odd()));
        assert!(bool::from(U128::ONE.is_odd()));
        assert!(bool::from(U128::MAX.is_odd()));
    }

    #[test]
    fn ct_eq() {
        let a = U128::ZERO;
        let b = U128::MAX;

        assert!(bool::from(a.ct_eq(&a)));
        assert!(!bool::from(a.ct_eq(&b)));
        assert!(!bool::from(b.ct_eq(&a)));
        assert!(bool::from(b.ct_eq(&b)));
    }

    #[test]
    fn ct_gt() {
        let a = U128::ZERO;
        let b = U128::ONE;
        let c = U128::MAX;

        assert!(bool::from(b.ct_gt(&a)));
        assert!(bool::from(c.ct_gt(&a)));
        assert!(bool::from(c.ct_gt(&b)));

        assert!(!bool::from(a.ct_gt(&a)));
        assert!(!bool::from(b.ct_gt(&b)));
        assert!(!bool::from(c.ct_gt(&c)));

        assert!(!bool::from(a.ct_gt(&b)));
        assert!(!bool::from(a.ct_gt(&c)));
        assert!(!bool::from(b.ct_gt(&c)));
    }

    #[test]
    fn ct_lt() {
        let a = U128::ZERO;
        let b = U128::ONE;
        let c = U128::MAX;

        assert!(bool::from(a.ct_lt(&b)));
        assert!(bool::from(a.ct_lt(&c)));
        assert!(bool::from(b.ct_lt(&c)));

        assert!(!bool::from(a.ct_lt(&a)));
        assert!(!bool::from(b.ct_lt(&b)));
        assert!(!bool::from(c.ct_lt(&c)));

        assert!(!bool::from(b.ct_lt(&a)));
        assert!(!bool::from(c.ct_lt(&a)));
        assert!(!bool::from(c.ct_lt(&b)));
    }
}
