/// Keccak-f1600 on ARMv8.4-A with FEAT_SHA3.
///
/// See p. K12.2.2  p. 11,749 of the ARM Reference manual.
/// Adapted from the Keccak-f1600 implementation in the XKCP/K12.
/// see <https://github.com/XKCP/K12/blob/df6a21e6d1f34c1aa36e8d702540899c97dba5a0/lib/ARMv8Asha3/KeccakP-1600-ARMv8Asha3.S#L69>
#[target_feature(enable = "sha3")]
pub unsafe fn f1600_armv8_sha3_asm(state: &mut [u64; 25]) {
    core::arch::asm!("
        // Read state
        ld1.1d {{ v0- v3}}, [x0], #32
        ld1.1d {{ v4- v7}}, [x0], #32
        ld1.1d {{ v8-v11}}, [x0], #32
        ld1.1d {{v12-v15}}, [x0], #32
        ld1.1d {{v16-v19}}, [x0], #32
        ld1.1d {{v20-v23}}, [x0], #32
        ld1.1d {{v24}},     [x0]
        sub x0, x0, #192

        // Loop 24 rounds
        // NOTE: This loop actually computes two f1600 functions in
        // parallel, in both the lower and the upper 64-bit of the
        // 128-bit registers v0-v24.
        mov	x8, #24
    0:  sub	x8, x8, #1

        // Theta Calculations
        eor3.16b   v25, v20, v15, v10
        eor3.16b   v26, v21, v16, v11
        eor3.16b   v27, v22, v17, v12
        eor3.16b   v28, v23, v18, v13
        eor3.16b   v29, v24, v19, v14
        eor3.16b   v25, v25,  v5,  v0
        eor3.16b   v26, v26,  v6,  v1
        eor3.16b   v27, v27,  v7,  v2
        eor3.16b   v28, v28,  v8,  v3
        eor3.16b   v29, v29,  v9,  v4
        rax1.2d    v30, v25, v27
        rax1.2d    v31, v26, v28
        rax1.2d    v27, v27, v29
        rax1.2d    v28, v28, v25
        rax1.2d    v29, v29, v26

        // Rho and Phi
        eor.16b     v0,  v0, v29
        xar.2d     v25,  v1, v30, #64 -  1
        xar.2d      v1,  v6, v30, #64 - 44
        xar.2d      v6,  v9, v28, #64 - 20
        xar.2d      v9, v22, v31, #64 - 61
        xar.2d     v22, v14, v28, #64 - 39
        xar.2d     v14, v20, v29, #64 - 18
        xar.2d     v26,  v2, v31, #64 - 62
        xar.2d      v2, v12, v31, #64 - 43
        xar.2d     v12, v13, v27, #64 - 25
        xar.2d     v13, v19, v28, #64 -  8
        xar.2d     v19, v23, v27, #64 - 56
        xar.2d     v23, v15, v29, #64 - 41
        xar.2d     v15,  v4, v28, #64 - 27
        xar.2d     v28, v24, v28, #64 - 14
        xar.2d     v24, v21, v30, #64 -  2
        xar.2d      v8,  v8, v27, #64 - 55
        xar.2d      v4, v16, v30, #64 - 45
        xar.2d     v16,  v5, v29, #64 - 36
        xar.2d      v5,  v3, v27, #64 - 28
        xar.2d     v27, v18, v27, #64 - 21
        xar.2d      v3, v17, v31, #64 - 15
        xar.2d     v30, v11, v30, #64 - 10
        xar.2d     v31,  v7, v31, #64 -  6
        xar.2d     v29, v10, v29, #64 -  3

        // Chi and Iota
        bcax.16b   v20, v26, v22,  v8
        bcax.16b   v21,  v8, v23, v22
        bcax.16b   v22, v22, v24, v23
        bcax.16b   v23, v23, v26, v24
        bcax.16b   v24, v24,  v8, v26

        ld1r.2d    {{v26}}, [x1], #8

        bcax.16b   v17, v30, v19,  v3
        bcax.16b   v18,  v3, v15, v19
        bcax.16b   v19, v19, v16, v15
        bcax.16b   v15, v15, v30, v16
        bcax.16b   v16, v16,  v3, v30

        bcax.16b   v10, v25, v12, v31
        bcax.16b   v11, v31, v13, v12
        bcax.16b   v12, v12, v14, v13
        bcax.16b   v13, v13, v25, v14
        bcax.16b   v14, v14, v31, v25

        bcax.16b    v7, v29,  v9,  v4
        bcax.16b    v8,  v4,  v5,  v9
        bcax.16b    v9,  v9,  v6,  v5
        bcax.16b    v5,  v5, v29,  v6
        bcax.16b    v6,  v6,  v4, v29

        bcax.16b    v3, v27,  v0, v28
        bcax.16b    v4, v28,  v1,  v0
        bcax.16b    v0,  v0,  v2,  v1
        bcax.16b    v1,  v1, v27,  v2
        bcax.16b    v2,  v2, v28, v27

        eor.16b v0,v0,v26

        // Rounds loop
        cbnz    w8, 0b

        // Write state
        st1.1d	{{ v0- v3}}, [x0], #32
        st1.1d	{{ v4- v7}}, [x0], #32
        st1.1d	{{ v8-v11}}, [x0], #32
        st1.1d	{{v12-v15}}, [x0], #32
        st1.1d	{{v16-v19}}, [x0], #32
        st1.1d	{{v20-v23}}, [x0], #32
        st1.1d	{{v24}},     [x0]
    ",
        in("x0") state.as_mut_ptr(),
        in("x1") crate::RC.as_ptr(),
        clobber_abi("C"),
        options(nostack)
    );
}

#[cfg(all(test, target_feature = "sha3"))]
mod tests {
    use super::*;

    #[test]
    fn test_keccak_f1600() {
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

        let mut state = [0u64; 25];
        unsafe { keccak_f1600(&mut state) };
        assert_eq!(state, state_first);
        unsafe { keccak_f1600(&mut state) };
        assert_eq!(state, state_second);
    }
}
