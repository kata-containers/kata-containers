//! [`UInt`] addition modulus operations.

use crate::{AddMod, Limb, UInt};

impl<const LIMBS: usize> UInt<LIMBS> {
    /// Computes `self + rhs mod p` in constant time.
    ///
    /// Assumes `self` and `rhs` are `< p`.
    pub const fn add_mod(&self, rhs: &UInt<LIMBS>, p: &UInt<LIMBS>) -> UInt<LIMBS> {
        let (w, carry) = self.adc(rhs, Limb::ZERO);

        // Attempt to subtract the modulus, to ensure the result is in the field.
        let (w, borrow) = w.sbb(p, Limb::ZERO);
        let (_, borrow) = carry.sbb(Limb::ZERO, borrow);

        // If underflow occurred on the final limb, borrow = 0xfff...fff, otherwise
        // borrow = 0x000...000. Thus, we use it as a mask to conditionally add the
        // modulus.
        let mut i = 0;
        let mut res = Self::ZERO;
        let mut carry = Limb::ZERO;

        while i < LIMBS {
            let rhs = p.limbs[i].bitand(borrow);
            let (limb, c) = w.limbs[i].adc(rhs, carry);
            res.limbs[i] = limb;
            carry = c;
            i += 1;
        }

        res
    }
}

macro_rules! impl_add_mod {
    ($($size:expr),+) => {
        $(
            impl AddMod for UInt<$size> {
                type Output = Self;

                fn add_mod(&self, rhs: &Self, p: &Self) -> Self {
                    debug_assert!(self < p);
                    debug_assert!(rhs < p);
                    self.add_mod(rhs, p)
                }
            }
        )+
    };
}

impl_add_mod!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12);

#[cfg(test)]
mod tests {
    use crate::U256;

    // TODO(tarcieri): additional tests + proptests

    #[test]
    fn add_mod_nist_p256() {
        let a =
            U256::from_be_hex("44acf6b7e36c1342c2c5897204fe09504e1e2efb1a900377dbc4e7a6a133ec56");
        let b =
            U256::from_be_hex("d5777c45019673125ad240f83094d4252d829516fac8601ed01979ec1ec1a251");
        let n =
            U256::from_be_hex("ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551");

        let actual = a.add_mod(&b, &n);
        let expected =
            U256::from_be_hex("1a2472fde50286541d97ca6a3592dd75beb9c9646e40c511b82496cfc3926956");

        assert_eq!(expected, actual);
    }
}
