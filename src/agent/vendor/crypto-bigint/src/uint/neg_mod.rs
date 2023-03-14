//! [`UInt`] subtraction modulus operations.

use crate::{Limb, NegMod, UInt};

impl<const LIMBS: usize> UInt<LIMBS> {
    /// Computes `-a mod p` in constant time.
    pub const fn neg_mod(&self, p: &Self) -> Self {
        let mut tmp = [Limb::ZERO; LIMBS];

        // Subtract `a` from `p` to negate. Ignore the final
        // borrow because it cannot underflow; a is guaranteed to
        // be in the field.
        let mut borrow = Limb::ZERO;
        let mut i = 0;

        while i < LIMBS {
            let (l, b) = p.limbs[i].sbb(self.limbs[i], borrow);
            tmp[i] = l;
            borrow = b;

            i += 1;
        }

        // `tmp` could be `p` if `a` was zero. Create a mask that is
        // zero if `a` was zero, and `Limb::MAX` if self was nonzero.
        // FIXME: constant time comparison
        let mut self_or = self.limbs[0];
        let mut i = 1;

        while i < LIMBS {
            self_or = self_or.bitor(self.limbs[i]);
            i += 1;
        }

        let v = if self_or.eq_vartime(&Limb::ZERO) {
            Limb::ONE
        } else {
            Limb::ZERO
        };

        let mask = v.wrapping_sub(Limb::ONE);

        let mut i = 0;

        while i < LIMBS {
            tmp[i] = tmp[i].bitand(mask);
            i += 1;
        }

        UInt::new(tmp)
    }
}

macro_rules! impl_neg_mod {
    ($($size:expr),+) => {
        $(
            impl NegMod for UInt<$size> {
                type Output = Self;

                fn neg_mod(&self, p: &Self) -> Self {
                    debug_assert!(self < p);
                    self.neg_mod(p)
                }
            }
        )+
    };
}

impl_neg_mod!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12);
