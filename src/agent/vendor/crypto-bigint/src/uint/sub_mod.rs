//! [`UInt`] subtraction modulus operations.

use crate::{Limb, SubMod, UInt};

impl<const LIMBS: usize> UInt<LIMBS> {
    /// Computes `self - rhs mod p` in constant time.
    ///
    /// Assumes `self` and `rhs` are `< p`.
    pub const fn sub_mod(&self, rhs: &UInt<LIMBS>, p: &UInt<LIMBS>) -> UInt<LIMBS> {
        let (mut out, borrow) = self.sbb(rhs, Limb::ZERO);

        // If underflow occurred on the final limb, borrow = 0xfff...fff, otherwise
        // borrow = 0x000...000. Thus, we use it as a mask to conditionally add the modulus.
        let mut carry = Limb::ZERO;
        let mut i = 0;

        while i < LIMBS {
            let (l, c) = out.limbs[i].adc(p.limbs[i].bitand(borrow), carry);
            out.limbs[i] = l;
            carry = c;
            i += 1;
        }

        out
    }
}

macro_rules! impl_sub_mod {
    ($($size:expr),+) => {
        $(
            impl SubMod for UInt<$size> {
                type Output = Self;

                fn sub_mod(&self, rhs: &Self, p: &Self) -> Self {
                    debug_assert!(self < p);
                    debug_assert!(rhs < p);
                    self.sub_mod(rhs, p)
                }
            }
        )+
    };
}

impl_sub_mod!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12);

#[cfg(all(test, feature = "rand"))]
mod tests {
    use crate::{Limb, NonZero, Random, RandomMod, UInt};
    use rand_core::SeedableRng;

    macro_rules! test_sub_mod {
        ($size:expr, $test_name:ident) => {
            #[test]
            fn $test_name() {
                let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(1);
                let moduli = [
                    NonZero::<UInt<$size>>::random(&mut rng),
                    NonZero::<UInt<$size>>::random(&mut rng),
                ];

                for p in &moduli {
                    let base_cases = [
                        (1u64, 0u64, 1u64.into()),
                        (0, 1, p.wrapping_sub(&1u64.into())),
                        (0, 0, 0u64.into()),
                    ];
                    for (a, b, c) in &base_cases {
                        let a: UInt<$size> = (*a).into();
                        let b: UInt<$size> = (*b).into();

                        let x = a.sub_mod(&b, p);
                        assert_eq!(*c, x, "{} - {} mod {} = {} != {}", a, b, p, x, c);
                    }

                    if $size > 1 {
                        for _i in 0..100 {
                            let a: UInt<$size> = Limb::random(&mut rng).into();
                            let b: UInt<$size> = Limb::random(&mut rng).into();
                            let (a, b) = if a < b { (b, a) } else { (a, b) };

                            let c = a.sub_mod(&b, p);
                            assert!(c < **p, "not reduced");
                            assert_eq!(c, a.wrapping_sub(&b), "result incorrect");
                        }
                    }

                    for _i in 0..100 {
                        let a = UInt::<$size>::random_mod(&mut rng, p);
                        let b = UInt::<$size>::random_mod(&mut rng, p);

                        let c = a.sub_mod(&b, p);
                        assert!(c < **p, "not reduced: {} >= {} ", c, p);

                        let x = a.wrapping_sub(&b);
                        if a >= b && x < **p {
                            assert_eq!(c, x, "incorrect result");
                        }
                    }
                }
            }
        };
    }

    // Test requires 1-limb is capable of representing a 64-bit integer
    #[cfg(target_pointer_width = "64")]
    test_sub_mod!(1, sub1);

    test_sub_mod!(2, sub2);
    test_sub_mod!(3, sub3);
    test_sub_mod!(4, sub4);
    test_sub_mod!(5, sub5);
    test_sub_mod!(6, sub6);
    test_sub_mod!(7, sub7);
    test_sub_mod!(8, sub8);
    test_sub_mod!(9, sub9);
    test_sub_mod!(10, sub10);
    test_sub_mod!(11, sub11);
    test_sub_mod!(12, sub12);
}
