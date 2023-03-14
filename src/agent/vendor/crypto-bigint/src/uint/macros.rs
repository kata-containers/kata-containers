//! Macros for defining aliases and relationships between `UInt` types.
// TODO(tarcieri): replace these with `const_evaluatable_checked` exprs when stable

// TODO(tarcieri): use `const_evaluatable_checked` when stable to make generic around bits.
macro_rules! impl_uint_aliases {
    ($(($name:ident, $bits:expr, $doc:expr)),+) => {
        $(
            #[doc = $doc]
            #[doc="unsigned big integer"]
            pub type $name = UInt<{nlimbs!($bits)}>;

            impl Encoding for $name {
                const BIT_SIZE: usize = $bits;
                const BYTE_SIZE: usize = $bits / 8;

                type Repr = [u8; $bits / 8];

                fn from_be_bytes(bytes: Self::Repr) -> Self {
                    Self::from_be_slice(&bytes)
                }

                fn from_le_bytes(bytes: Self::Repr) -> Self {
                    Self::from_be_slice(&bytes)
                }

                #[inline]
                fn to_be_bytes(&self) -> Self::Repr {
                    let mut result = [0u8; $bits / 8];
                    self.write_be_bytes(&mut result);
                    result
                }

                #[inline]
                fn to_le_bytes(&self) -> Self::Repr {
                    let mut result = [0u8; $bits / 8];
                    self.write_le_bytes(&mut result);
                    result
                }
            }
        )+
     };
}

// TODO(tarcieri): use `const_evaluatable_checked` when stable to make generic around bits.
macro_rules! impl_concat {
    ($(($name:ident, $bits:expr)),+) => {
        $(
            impl Concat for $name {
                type Output = UInt<{nlimbs!($bits) * 2}>;

                fn concat(&self, rhs: &Self) -> Self::Output {
                    let mut output = Self::Output::default();
                    let (lo, hi) = output.limbs.split_at_mut(self.limbs.len());
                    lo.copy_from_slice(&rhs.limbs);
                    hi.copy_from_slice(&self.limbs);
                    output
                }
            }

            impl From<($name, $name)> for UInt<{nlimbs!($bits) * 2}> {
                fn from(nums: ($name, $name)) -> UInt<{nlimbs!($bits) * 2}> {
                    nums.0.concat(&nums.1)
                }
            }
        )+
     };
}

// TODO(tarcieri): use `const_evaluatable_checked` when stable to make generic around bits.
macro_rules! impl_split {
    ($(($name:ident, $bits:expr)),+) => {
        $(
            impl Split for $name {
                type Output = UInt<{nlimbs!($bits) / 2}>;

                fn split(&self) -> (Self::Output, Self::Output) {
                    let mut hi_out = Self::Output::default();
                    let mut lo_out = Self::Output::default();
                    let (lo_in, hi_in) = self.limbs.split_at(self.limbs.len() / 2);
                    hi_out.limbs.copy_from_slice(&hi_in);
                    lo_out.limbs.copy_from_slice(&lo_in);
                    (hi_out, lo_out)
                }
            }

            impl From<$name> for (UInt<{nlimbs!($bits) / 2}>, UInt<{nlimbs!($bits) / 2}>) {
                fn from(num: $name) -> (UInt<{nlimbs!($bits) / 2}>, UInt<{nlimbs!($bits) / 2}>) {
                    num.split()
                }
            }
        )+
     };
}
