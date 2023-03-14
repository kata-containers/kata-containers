use super::expand::expand;
use crate::utils::check;

#[test]
fn test() {
    let enc_keys = expand(&[0x00; 32]).0;
    check(
        &enc_keys,
        &[
            [0x0000000000000000, 0x0000000000000000],
            [0x0000000000000000, 0x0000000000000000],
            [0x6263636362636363, 0x6263636362636363],
            [0xaafbfbfbaafbfbfb, 0xaafbfbfbaafbfbfb],
            [0x6f6c6ccf0d0f0fac, 0x6f6c6ccf0d0f0fac],
            [0x7d8d8d6ad7767691, 0x7d8d8d6ad7767691],
            [0x5354edc15e5be26d, 0x31378ea23c38810e],
            [0x968a81c141fcf750, 0x3c717a3aeb070cab],
            [0x9eaa8f28c0f16d45, 0xf1c6e3e7cdfe62e9],
            [0x2b312bdf6acddc8f, 0x56bca6b5bdbbaa1e],
            [0x6406fd52a4f79017, 0x553173f098cf1119],
            [0x6dbba90b07767584, 0x51cad331ec71792f],
            [0xe7b0e89c4347788b, 0x16760b7b8eb91a62],
            [0x74ed0ba1739b7e25, 0x2251ad14ce20d43b],
            [0x10f80a1753bf729c, 0x45c979e7cb706385],
        ],
    );

    let enc_keys = expand(&[0xff; 32]).0;
    check(
        &enc_keys,
        &[
            [0xffffffffffffffff, 0xffffffffffffffff],
            [0xffffffffffffffff, 0xffffffffffffffff],
            [0xe8e9e9e917161616, 0xe8e9e9e917161616],
            [0x0fb8b8b8f0474747, 0x0fb8b8b8f0474747],
            [0x4a4949655d5f5f73, 0xb5b6b69aa2a0a08c],
            [0x355858dcc51f1f9b, 0xcaa7a7233ae0e064],
            [0xafa80ae5f2f75596, 0x4741e30ce5e14380],
            [0xeca0421129bf5d8a, 0xe318faa9d9f81acd],
            [0xe60ab7d014fde246, 0x53bc014ab65d42ca],
            [0xa2ec6e658b5333ef, 0x684bc946b1b3d38b],
            [0x9b6c8a188f91685e, 0xdc2d69146a702bde],
            [0xa0bd9f782beeac97, 0x43a565d1f216b65a],
            [0xfc22349173b35ccf, 0xaf9e35dbc5ee1e05],
            [0x0695ed132d7b4184, 0x6ede24559cc8920f],
            [0x546d424f27de1e80, 0x88402b5b4dae355e],
        ],
    );

    let enc_keys = expand(&[
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d,
        0x1e, 0x1f,
    ])
    .0;
    check(
        &enc_keys,
        &[
            [0x0001020304050607, 0x08090a0b0c0d0e0f],
            [0x1011121314151617, 0x18191a1b1c1d1e1f],
            [0xa573c29fa176c498, 0xa97fce93a572c09c],
            [0x1651a8cd0244beda, 0x1a5da4c10640bade],
            [0xae87dff00ff11b68, 0xa68ed5fb03fc1567],
            [0x6de1f1486fa54f92, 0x75f8eb5373b8518d],
            [0xc656827fc9a79917, 0x6f294cec6cd5598b],
            [0x3de23a75524775e7, 0x27bf9eb45407cf39],
            [0x0bdc905fc27b0948, 0xad5245a4c1871c2f],
            [0x45f5a66017b2d387, 0x300d4d33640a820a],
            [0x7ccff71cbeb4fe54, 0x13e6bbf0d261a7df],
            [0xf01afafee7a82979, 0xd7a5644ab3afe640],
            [0x2541fe719bf50025, 0x8813bbd55a721c0a],
            [0x4e5a6699a9f24fe0, 0x7e572baacdf8cdea],
            [0x24fc79ccbf0979e9, 0x371ac23c6d68de36],
        ],
    );

    let enc_keys = expand(&[
        0x60, 0x3d, 0xeb, 0x10, 0x15, 0xca, 0x71, 0xbe, 0x2b, 0x73, 0xae, 0xf0, 0x85, 0x7d, 0x77,
        0x81, 0x1f, 0x35, 0x2c, 0x07, 0x3b, 0x61, 0x08, 0xd7, 0x2d, 0x98, 0x10, 0xa3, 0x09, 0x14,
        0xdf, 0xf4,
    ])
    .0;
    check(
        &enc_keys,
        &[
            [0x603deb1015ca71be, 0x2b73aef0857d7781],
            [0x1f352c073b6108d7, 0x2d9810a30914dff4],
            [0x9ba354118e6925af, 0xa51a8b5f2067fcde],
            [0xa8b09c1a93d194cd, 0xbe49846eb75d5b9a],
            [0xd59aecb85bf3c917, 0xfee94248de8ebe96],
            [0xb5a9328a2678a647, 0x983122292f6c79b3],
            [0x812c81addadf48ba, 0x24360af2fab8b464],
            [0x98c5bfc9bebd198e, 0x268c3ba709e04214],
            [0x68007bacb2df3316, 0x96e939e46c518d80],
            [0xc814e20476a9fb8a, 0x5025c02d59c58239],
            [0xde1369676ccc5a71, 0xfa2563959674ee15],
            [0x5886ca5d2e2f31d7, 0x7e0af1fa27cf73c3],
            [0x749c47ab18501dda, 0xe2757e4f7401905a],
            [0xcafaaae3e4d59b34, 0x9adf6acebd10190d],
            [0xfe4890d1e6188d0b, 0x046df344706c631e],
        ],
    );
}
