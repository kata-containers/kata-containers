//! Keccak [sponge function](https://en.wikipedia.org/wiki/Sponge_function).
//!
//! If you are looking for SHA-3 hash functions take a look at [`sha3`][1] and
//! [`tiny-keccak`][2] crates.
//!
//! To disable loop unrolling (e.g. for constraint targets) use `no_unroll`
//! feature.
//!
//! ```
//! // Test vectors are from KeccakCodePackage
//! let mut data = [0u64; 25];
//!
//! keccak::f1600(&mut data);
//! assert_eq!(data, [
//!     0xF1258F7940E1DDE7, 0x84D5CCF933C0478A, 0xD598261EA65AA9EE, 0xBD1547306F80494D,
//!     0x8B284E056253D057, 0xFF97A42D7F8E6FD4, 0x90FEE5A0A44647C4, 0x8C5BDA0CD6192E76,
//!     0xAD30A6F71B19059C, 0x30935AB7D08FFC64, 0xEB5AA93F2317D635, 0xA9A6E6260D712103,
//!     0x81A57C16DBCF555F, 0x43B831CD0347C826, 0x01F22F1A11A5569F, 0x05E5635A21D9AE61,
//!     0x64BEFEF28CC970F2, 0x613670957BC46611, 0xB87C5A554FD00ECB, 0x8C3EE88A1CCF32C8,
//!     0x940C7922AE3A2614, 0x1841F924A2C509E4, 0x16F53526E70465C2, 0x75F644E97F30A13B,
//!     0xEAF1FF7B5CECA249,
//! ]);
//!
//! keccak::f1600(&mut data);
//! assert_eq!(data, [
//!     0x2D5C954DF96ECB3C, 0x6A332CD07057B56D, 0x093D8D1270D76B6C, 0x8A20D9B25569D094,
//!     0x4F9C4F99E5E7F156, 0xF957B9A2DA65FB38, 0x85773DAE1275AF0D, 0xFAF4F247C3D810F7,
//!     0x1F1B9EE6F79A8759, 0xE4FECC0FEE98B425, 0x68CE61B6B9CE68A1, 0xDEEA66C4BA8F974F,
//!     0x33C43D836EAFB1F5, 0xE00654042719DBD9, 0x7CF8A9F009831265, 0xFD5449A6BF174743,
//!     0x97DDAD33D8994B40, 0x48EAD5FC5D0BE774, 0xE3B8C8EE55B7B03C, 0x91A0226E649E42E9,
//!     0x900E3129E7BADD7B, 0x202A9EC5FAA3CCE8, 0x5B3402464E1C3DB6, 0x609F4E62A44C1059,
//!     0x20D06CD26A8FBF5C,
//! ]);
//! ```
//!
//! [1]: https://docs.rs/sha3
//! [2]: https://docs.rs/tiny-keccak
#![no_std]
#![allow(non_upper_case_globals)]
#![cfg_attr(feature = "simd", feature(portable_simd))]

use core::{
    convert::TryInto,
    fmt::Debug,
    mem::size_of,
    ops::{BitAnd, BitAndAssign, BitXor, BitXorAssign, Not},
};

#[rustfmt::skip]
mod unroll;

#[cfg(all(target_arch = "aarch64", feature = "asm"))]
mod armv8;

#[cfg(all(target_arch = "aarch64", feature = "asm"))]
cpufeatures::new!(armv8_sha3_intrinsics, "sha3");

const PLEN: usize = 25;

const RHO: [u32; 24] = [
    1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56, 8, 25, 43, 62, 18, 39, 61, 20, 44,
];

const PI: [usize; 24] = [
    10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4, 15, 23, 19, 13, 12, 2, 20, 14, 22, 9, 6, 1,
];

const RC: [u64; 24] = [
    0x0000000000000001,
    0x0000000000008082,
    0x800000000000808a,
    0x8000000080008000,
    0x000000000000808b,
    0x0000000080000001,
    0x8000000080008081,
    0x8000000000008009,
    0x000000000000008a,
    0x0000000000000088,
    0x0000000080008009,
    0x000000008000000a,
    0x000000008000808b,
    0x800000000000008b,
    0x8000000000008089,
    0x8000000000008003,
    0x8000000000008002,
    0x8000000000000080,
    0x000000000000800a,
    0x800000008000000a,
    0x8000000080008081,
    0x8000000000008080,
    0x0000000080000001,
    0x8000000080008008,
];

pub trait LaneSize:
    Copy
    + Clone
    + Debug
    + Default
    + PartialEq
    + BitAndAssign
    + BitAnd<Output = Self>
    + BitXorAssign
    + BitXor<Output = Self>
    + Not<Output = Self>
{
    const KECCAK_F_ROUND_COUNT: usize;

    fn truncate_rc(rc: u64) -> Self;
    fn rotate_left(self, n: u32) -> Self;
}

macro_rules! impl_lanesize {
    ($type:ty, $round:expr, $truncate:expr) => {
        impl LaneSize for $type {
            const KECCAK_F_ROUND_COUNT: usize = $round;

            fn truncate_rc(rc: u64) -> Self {
                $truncate(rc)
            }

            fn rotate_left(self, n: u32) -> Self {
                self.rotate_left(n)
            }
        }
    };
}

impl_lanesize!(u8, 18, |rc: u64| { rc.to_le_bytes()[0] });
impl_lanesize!(u16, 20, |rc: u64| {
    let tmp = rc.to_le_bytes();
    Self::from_le_bytes(tmp[..size_of::<Self>()].try_into().unwrap())
});
impl_lanesize!(u32, 22, |rc: u64| {
    let tmp = rc.to_le_bytes();
    Self::from_le_bytes(tmp[..size_of::<Self>()].try_into().unwrap())
});
impl_lanesize!(u64, 24, |rc: u64| { rc });

macro_rules! impl_keccak {
    ($name:ident, $type:ty) => {
        /// Keccak-f sponge function
        pub fn $name(state: &mut [$type; PLEN]) {
            keccak_p(state, <$type>::KECCAK_F_ROUND_COUNT);
        }
    };
}

impl_keccak!(f200, u8);
impl_keccak!(f400, u16);
impl_keccak!(f800, u32);

#[cfg(not(all(target_arch = "aarch64", feature = "asm")))]
impl_keccak!(f1600, u64);

#[cfg(all(target_arch = "aarch64", feature = "asm"))]
pub fn f1600(state: &mut [u64; PLEN]) {
    if armv8_sha3_intrinsics::get() {
        unsafe { armv8::f1600_armv8_sha3_asm(state) }
    } else {
        keccak_p(state, u64::KECCAK_F_ROUND_COUNT);
    }
}

#[cfg(feature = "simd")]
/// SIMD implementations for Keccak-f1600 sponge function
pub mod simd {
    pub use core::simd::{u64x2, u64x4, u64x8};
    use {keccak_p, LaneSize, PLEN};

    macro_rules! impl_lanesize_simd_u64xn {
        ($type:ty) => {
            impl LaneSize for $type {
                const KECCAK_F_ROUND_COUNT: usize = 24;

                fn truncate_rc(rc: u64) -> Self {
                    Self::splat(rc)
                }

                fn rotate_left(self, n: u32) -> Self {
                    self << Self::splat(n.into()) | self >> Self::splat((64 - n).into())
                }
            }
        };
    }

    impl_lanesize_simd_u64xn!(u64x2);
    impl_lanesize_simd_u64xn!(u64x4);
    impl_lanesize_simd_u64xn!(u64x8);

    impl_keccak!(f1600x2, u64x2);
    impl_keccak!(f1600x4, u64x4);
    impl_keccak!(f1600x8, u64x8);
}

#[allow(unused_assignments)]
/// Generic Keccak-p sponge function
pub fn keccak_p<L: LaneSize>(state: &mut [L; PLEN], round_count: usize) {
    if round_count > L::KECCAK_F_ROUND_COUNT {
        panic!("A round_count greater than KECCAK_F_ROUND_COUNT is not supported!");
    }

    // https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.202.pdf#page=25
    // "the rounds of KECCAK-p[b, nr] match the last rounds of KECCAK-f[b]"
    let round_consts = &RC[(L::KECCAK_F_ROUND_COUNT - round_count)..L::KECCAK_F_ROUND_COUNT];

    // not unrolling this loop results in a much smaller function, plus
    // it positively influences performance due to the smaller load on I-cache
    for &rc in round_consts {
        let mut array = [L::default(); 5];

        // Theta
        unroll5!(x, {
            unroll5!(y, {
                array[x] ^= state[5 * y + x];
            });
        });

        unroll5!(x, {
            unroll5!(y, {
                let t1 = array[(x + 4) % 5];
                let t2 = array[(x + 1) % 5].rotate_left(1);
                state[5 * y + x] ^= t1 ^ t2;
            });
        });

        // Rho and pi
        let mut last = state[1];
        unroll24!(x, {
            array[0] = state[PI[x]];
            state[PI[x]] = last.rotate_left(RHO[x]);
            last = array[0];
        });

        // Chi
        unroll5!(y_step, {
            let y = 5 * y_step;

            unroll5!(x, {
                array[x] = state[y + x];
            });

            unroll5!(x, {
                let t1 = !array[(x + 1) % 5];
                let t2 = array[(x + 2) % 5];
                state[y + x] = array[x] ^ (t1 & t2);
            });
        });

        // Iota
        state[0] ^= L::truncate_rc(rc);
    }
}

#[cfg(test)]
mod tests {
    use {keccak_p, LaneSize, PLEN};

    fn keccak_f<L: LaneSize>(state_first: [L; PLEN], state_second: [L; PLEN]) {
        let mut state = [L::default(); PLEN];

        keccak_p(&mut state, L::KECCAK_F_ROUND_COUNT);
        assert_eq!(state, state_first);

        keccak_p(&mut state, L::KECCAK_F_ROUND_COUNT);
        assert_eq!(state, state_second);
    }

    #[test]
    fn keccak_f200() {
        // Test vectors are copied from XKCP (eXtended Keccak Code Package)
        // https://github.com/XKCP/XKCP/blob/master/tests/TestVectors/KeccakF-200-IntermediateValues.txt
        let state_first = [
            0x3C, 0x28, 0x26, 0x84, 0x1C, 0xB3, 0x5C, 0x17, 0x1E, 0xAA, 0xE9, 0xB8, 0x11, 0x13,
            0x4C, 0xEA, 0xA3, 0x85, 0x2C, 0x69, 0xD2, 0xC5, 0xAB, 0xAF, 0xEA,
        ];
        let state_second = [
            0x1B, 0xEF, 0x68, 0x94, 0x92, 0xA8, 0xA5, 0x43, 0xA5, 0x99, 0x9F, 0xDB, 0x83, 0x4E,
            0x31, 0x66, 0xA1, 0x4B, 0xE8, 0x27, 0xD9, 0x50, 0x40, 0x47, 0x9E,
        ];

        keccak_f::<u8>(state_first, state_second);
    }

    #[test]
    fn keccak_f400() {
        // Test vectors are copied from XKCP (eXtended Keccak Code Package)
        // https://github.com/XKCP/XKCP/blob/master/tests/TestVectors/KeccakF-400-IntermediateValues.txt
        let state_first = [
            0x09F5, 0x40AC, 0x0FA9, 0x14F5, 0xE89F, 0xECA0, 0x5BD1, 0x7870, 0xEFF0, 0xBF8F, 0x0337,
            0x6052, 0xDC75, 0x0EC9, 0xE776, 0x5246, 0x59A1, 0x5D81, 0x6D95, 0x6E14, 0x633E, 0x58EE,
            0x71FF, 0x714C, 0xB38E,
        ];
        let state_second = [
            0xE537, 0xD5D6, 0xDBE7, 0xAAF3, 0x9BC7, 0xCA7D, 0x86B2, 0xFDEC, 0x692C, 0x4E5B, 0x67B1,
            0x15AD, 0xA7F7, 0xA66F, 0x67FF, 0x3F8A, 0x2F99, 0xE2C2, 0x656B, 0x5F31, 0x5BA6, 0xCA29,
            0xC224, 0xB85C, 0x097C,
        ];

        keccak_f::<u16>(state_first, state_second);
    }

    #[test]
    fn keccak_f800() {
        // Test vectors are copied from XKCP (eXtended Keccak Code Package)
        // https://github.com/XKCP/XKCP/blob/master/tests/TestVectors/KeccakF-800-IntermediateValues.txt
        let state_first = [
            0xE531D45D, 0xF404C6FB, 0x23A0BF99, 0xF1F8452F, 0x51FFD042, 0xE539F578, 0xF00B80A7,
            0xAF973664, 0xBF5AF34C, 0x227A2424, 0x88172715, 0x9F685884, 0xB15CD054, 0x1BF4FC0E,
            0x6166FA91, 0x1A9E599A, 0xA3970A1F, 0xAB659687, 0xAFAB8D68, 0xE74B1015, 0x34001A98,
            0x4119EFF3, 0x930A0E76, 0x87B28070, 0x11EFE996,
        ];
        let state_second = [
            0x75BF2D0D, 0x9B610E89, 0xC826AF40, 0x64CD84AB, 0xF905BDD6, 0xBC832835, 0x5F8001B9,
            0x15662CCE, 0x8E38C95E, 0x701FE543, 0x1B544380, 0x89ACDEFF, 0x51EDB5DE, 0x0E9702D9,
            0x6C19AA16, 0xA2913EEE, 0x60754E9A, 0x9819063C, 0xF4709254, 0xD09F9084, 0x772DA259,
            0x1DB35DF7, 0x5AA60162, 0x358825D5, 0xB3783BAB,
        ];

        keccak_f::<u32>(state_first, state_second);
    }

    #[test]
    fn keccak_f1600() {
        // Test vectors are copied from XKCP (eXtended Keccak Code Package)
        // https://github.com/XKCP/XKCP/blob/master/tests/TestVectors/KeccakF-1600-IntermediateValues.txt
        let state_first = [
            0xF1258F7940E1DDE7,
            0x84D5CCF933C0478A,
            0xD598261EA65AA9EE,
            0xBD1547306F80494D,
            0x8B284E056253D057,
            0xFF97A42D7F8E6FD4,
            0x90FEE5A0A44647C4,
            0x8C5BDA0CD6192E76,
            0xAD30A6F71B19059C,
            0x30935AB7D08FFC64,
            0xEB5AA93F2317D635,
            0xA9A6E6260D712103,
            0x81A57C16DBCF555F,
            0x43B831CD0347C826,
            0x01F22F1A11A5569F,
            0x05E5635A21D9AE61,
            0x64BEFEF28CC970F2,
            0x613670957BC46611,
            0xB87C5A554FD00ECB,
            0x8C3EE88A1CCF32C8,
            0x940C7922AE3A2614,
            0x1841F924A2C509E4,
            0x16F53526E70465C2,
            0x75F644E97F30A13B,
            0xEAF1FF7B5CECA249,
        ];
        let state_second = [
            0x2D5C954DF96ECB3C,
            0x6A332CD07057B56D,
            0x093D8D1270D76B6C,
            0x8A20D9B25569D094,
            0x4F9C4F99E5E7F156,
            0xF957B9A2DA65FB38,
            0x85773DAE1275AF0D,
            0xFAF4F247C3D810F7,
            0x1F1B9EE6F79A8759,
            0xE4FECC0FEE98B425,
            0x68CE61B6B9CE68A1,
            0xDEEA66C4BA8F974F,
            0x33C43D836EAFB1F5,
            0xE00654042719DBD9,
            0x7CF8A9F009831265,
            0xFD5449A6BF174743,
            0x97DDAD33D8994B40,
            0x48EAD5FC5D0BE774,
            0xE3B8C8EE55B7B03C,
            0x91A0226E649E42E9,
            0x900E3129E7BADD7B,
            0x202A9EC5FAA3CCE8,
            0x5B3402464E1C3DB6,
            0x609F4E62A44C1059,
            0x20D06CD26A8FBF5C,
        ];

        keccak_f::<u64>(state_first, state_second);
    }

    #[cfg(feature = "simd")]
    mod simd {
        use simd::{u64x2, u64x4, u64x8};
        use tests::keccak_f;

        macro_rules! impl_keccak_f1600xn {
            ($name:ident, $type:ty) => {
                #[test]
                fn $name() {
                    // Test vectors are copied from XKCP (eXtended Keccak Code Package)
                    // https://github.com/XKCP/XKCP/blob/master/tests/TestVectors/KeccakF-1600-IntermediateValues.txt
                    let state_first = [
                        <$type>::splat(0xF1258F7940E1DDE7),
                        <$type>::splat(0x84D5CCF933C0478A),
                        <$type>::splat(0xD598261EA65AA9EE),
                        <$type>::splat(0xBD1547306F80494D),
                        <$type>::splat(0x8B284E056253D057),
                        <$type>::splat(0xFF97A42D7F8E6FD4),
                        <$type>::splat(0x90FEE5A0A44647C4),
                        <$type>::splat(0x8C5BDA0CD6192E76),
                        <$type>::splat(0xAD30A6F71B19059C),
                        <$type>::splat(0x30935AB7D08FFC64),
                        <$type>::splat(0xEB5AA93F2317D635),
                        <$type>::splat(0xA9A6E6260D712103),
                        <$type>::splat(0x81A57C16DBCF555F),
                        <$type>::splat(0x43B831CD0347C826),
                        <$type>::splat(0x01F22F1A11A5569F),
                        <$type>::splat(0x05E5635A21D9AE61),
                        <$type>::splat(0x64BEFEF28CC970F2),
                        <$type>::splat(0x613670957BC46611),
                        <$type>::splat(0xB87C5A554FD00ECB),
                        <$type>::splat(0x8C3EE88A1CCF32C8),
                        <$type>::splat(0x940C7922AE3A2614),
                        <$type>::splat(0x1841F924A2C509E4),
                        <$type>::splat(0x16F53526E70465C2),
                        <$type>::splat(0x75F644E97F30A13B),
                        <$type>::splat(0xEAF1FF7B5CECA249),
                    ];
                    let state_second = [
                        <$type>::splat(0x2D5C954DF96ECB3C),
                        <$type>::splat(0x6A332CD07057B56D),
                        <$type>::splat(0x093D8D1270D76B6C),
                        <$type>::splat(0x8A20D9B25569D094),
                        <$type>::splat(0x4F9C4F99E5E7F156),
                        <$type>::splat(0xF957B9A2DA65FB38),
                        <$type>::splat(0x85773DAE1275AF0D),
                        <$type>::splat(0xFAF4F247C3D810F7),
                        <$type>::splat(0x1F1B9EE6F79A8759),
                        <$type>::splat(0xE4FECC0FEE98B425),
                        <$type>::splat(0x68CE61B6B9CE68A1),
                        <$type>::splat(0xDEEA66C4BA8F974F),
                        <$type>::splat(0x33C43D836EAFB1F5),
                        <$type>::splat(0xE00654042719DBD9),
                        <$type>::splat(0x7CF8A9F009831265),
                        <$type>::splat(0xFD5449A6BF174743),
                        <$type>::splat(0x97DDAD33D8994B40),
                        <$type>::splat(0x48EAD5FC5D0BE774),
                        <$type>::splat(0xE3B8C8EE55B7B03C),
                        <$type>::splat(0x91A0226E649E42E9),
                        <$type>::splat(0x900E3129E7BADD7B),
                        <$type>::splat(0x202A9EC5FAA3CCE8),
                        <$type>::splat(0x5B3402464E1C3DB6),
                        <$type>::splat(0x609F4E62A44C1059),
                        <$type>::splat(0x20D06CD26A8FBF5C),
                    ];

                    keccak_f::<$type>(state_first, state_second);
                }
            };
        }

        impl_keccak_f1600xn!(keccak_f1600x2, u64x2);
        impl_keccak_f1600xn!(keccak_f1600x4, u64x4);
        impl_keccak_f1600xn!(keccak_f1600x8, u64x8);
    }
}
