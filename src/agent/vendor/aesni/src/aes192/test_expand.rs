use super::expand::expand;
use crate::utils::check;

#[test]
fn test() {
    let enc_keys = expand(&[0x00; 24]).0;
    check(
        &enc_keys,
        &[
            [0x0000000000000000, 0x0000000000000000],
            [0x0000000000000000, 0x6263636362636363],
            [0x6263636362636363, 0x6263636362636363],
            [0x9b9898c9f9fbfbaa, 0x9b9898c9f9fbfbaa],
            [0x9b9898c9f9fbfbaa, 0x90973450696ccffa],
            [0xf2f457330b0fac99, 0x90973450696ccffa],
            [0xc81d19a9a171d653, 0x53858160588a2df9],
            [0xc81d19a9a171d653, 0x7bebf49bda9a22c8],
            [0x891fa3a8d1958e51, 0x198897f8b8f941ab],
            [0xc26896f718f2b43f, 0x91ed1797407899c6],
            [0x59f00e3ee1094f95, 0x83ecbc0f9b1e0830],
            [0x0af31fa74a8b8661, 0x137b885ff272c7ca],
            [0x432ac886d834c0b6, 0xd2c7df11984c5970],
        ],
    );

    let enc_keys = expand(&[0xff; 24]).0;
    check(
        &enc_keys,
        &[
            [0xffffffffffffffff, 0xffffffffffffffff],
            [0xffffffffffffffff, 0xe8e9e9e917161616],
            [0xe8e9e9e917161616, 0xe8e9e9e917161616],
            [0xadaeae19bab8b80f, 0x525151e6454747f0],
            [0xadaeae19bab8b80f, 0xc5c2d8ed7f7a60e2],
            [0x2d2b3104686c76f4, 0xc5c2d8ed7f7a60e2],
            [0x1712403f686820dd, 0x454311d92d2f672d],
            [0xe8edbfc09797df22, 0x8f8cd3b7e7e4f36a],
            [0xa2a7e2b38f88859e, 0x67653a5ef0f2e57c],
            [0x2655c33bc1b13051, 0x6316d2e2ec9e577c],
            [0x8bfb6d227b09885e, 0x67919b1aa620ab4b],
            [0xc53679a929a82ed5, 0xa25343f7d95acba9],
            [0x598e482fffaee364, 0x3a989acd1330b418],
        ],
    );

    let enc_keys = expand(&[
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
    ])
    .0;
    check(
        &enc_keys,
        &[
            [0x0001020304050607, 0x08090a0b0c0d0e0f],
            [0x1011121314151617, 0x5846f2f95c43f4fe],
            [0x544afef55847f0fa, 0x4856e2e95c43f4fe],
            [0x40f949b31cbabd4d, 0x48f043b810b7b342],
            [0x58e151ab04a2a555, 0x7effb5416245080c],
            [0x2ab54bb43a02f8f6, 0x62e3a95d66410c08],
            [0xf501857297448d7e, 0xbdf1c6ca87f33e3c],
            [0xe510976183519b69, 0x34157c9ea351f1e0],
            [0x1ea0372a99530916, 0x7c439e77ff12051e],
            [0xdd7e0e887e2fff68, 0x608fc842f9dcc154],
            [0x859f5f237a8d5a3d, 0xc0c02952beefd63a],
            [0xde601e7827bcdf2c, 0xa223800fd8aeda32],
            [0xa4970a331a78dc09, 0xc418c271e3a41d5d],
        ],
    );

    let enc_keys = expand(&[
        0x8e, 0x73, 0xb0, 0xf7, 0xda, 0x0e, 0x64, 0x52, 0xc8, 0x10, 0xf3, 0x2b, 0x80, 0x90, 0x79,
        0xe5, 0x62, 0xf8, 0xea, 0xd2, 0x52, 0x2c, 0x6b, 0x7b,
    ])
    .0;
    check(
        &enc_keys,
        &[
            [0x8e73b0f7da0e6452, 0xc810f32b809079e5],
            [0x62f8ead2522c6b7b, 0xfe0c91f72402f5a5],
            [0xec12068e6c827f6b, 0x0e7a95b95c56fec2],
            [0x4db7b4bd69b54118, 0x85a74796e92538fd],
            [0xe75fad44bb095386, 0x485af05721efb14f],
            [0xa448f6d94d6dce24, 0xaa326360113b30e6],
            [0xa25e7ed583b1cf9a, 0x27f939436a94f767],
            [0xc0a69407d19da4e1, 0xec1786eb6fa64971],
            [0x485f703222cb8755, 0xe26d135233f0b7b3],
            [0x40beeb282f18a259, 0x6747d26b458c553e],
            [0xa7e1466c9411f1df, 0x821f750aad07d753],
            [0xca4005388fcc5006, 0x282d166abc3ce7b5],
            [0xe98ba06f448c773c, 0x8ecc720401002202],
        ],
    );
}
