use crate::{Limb, LimbUInt, UInt};

impl<const LIMBS: usize> UInt<LIMBS> {
    /// Calculate the number of bits needed to represent this number.
    #[allow(trivial_numeric_casts)]
    pub const fn bits(self) -> usize {
        let mut i = LIMBS - 1;
        while i > 0 && self.limbs[i].0 == 0 {
            i -= 1;
        }

        let limb = self.limbs[i].0;
        let bits = (Limb::BIT_SIZE * (i + 1)) as LimbUInt - limb.leading_zeros() as LimbUInt;

        Limb::ct_select(
            Limb(bits),
            Limb::ZERO,
            !self.limbs[0].is_nonzero() & !Limb(i as LimbUInt).is_nonzero(),
        )
        .0 as usize
    }
}
