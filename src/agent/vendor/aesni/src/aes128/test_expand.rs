use super::expand::expand;
use crate::utils::check;

#[test]
fn test() {
    let enc_keys = expand(&[0x00; 16]).0;
    check(
        &enc_keys,
        &[
            [0x0000000000000000, 0x0000000000000000],
            [0x6263636362636363, 0x6263636362636363],
            [0x9b9898c9f9fbfbaa, 0x9b9898c9f9fbfbaa],
            [0x90973450696ccffa, 0xf2f457330b0fac99],
            [0xee06da7b876a1581, 0x759e42b27e91ee2b],
            [0x7f2e2b88f8443e09, 0x8dda7cbbf34b9290],
            [0xec614b851425758c, 0x99ff09376ab49ba7],
            [0x217517873550620b, 0xacaf6b3cc61bf09b],
            [0x0ef903333ba96138, 0x97060a04511dfa9f],
            [0xb1d4d8e28a7db9da, 0x1d7bb3de4c664941],
            [0xb4ef5bcb3e92e211, 0x23e951cf6f8f188e],
        ],
    );

    let enc_keys = expand(&[0xff; 16]).0;
    check(
        &enc_keys,
        &[
            [0xffffffffffffffff, 0xffffffffffffffff],
            [0xe8e9e9e917161616, 0xe8e9e9e917161616],
            [0xadaeae19bab8b80f, 0x525151e6454747f0],
            [0x090e2277b3b69a78, 0xe1e7cb9ea4a08c6e],
            [0xe16abd3e52dc2746, 0xb33becd8179b60b6],
            [0xe5baf3ceb766d488, 0x045d385013c658e6],
            [0x71d07db3c6b6a93b, 0xc2eb916bd12dc98d],
            [0xe90d208d2fbb89b6, 0xed5018dd3c7dd150],
            [0x96337366b988fad0, 0x54d8e20d68a5335d],
            [0x8bf03f233278c5f3, 0x66a027fe0e0514a3],
            [0xd60a3588e472f07b, 0x82d2d7858cd7c326],
        ],
    );

    let enc_keys = expand(&[
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        0x0f,
    ])
    .0;
    check(
        &enc_keys,
        &[
            [0x0001020304050607, 0x08090a0b0c0d0e0f],
            [0xd6aa74fdd2af72fa, 0xdaa678f1d6ab76fe],
            [0xb692cf0b643dbdf1, 0xbe9bc5006830b3fe],
            [0xb6ff744ed2c2c9bf, 0x6c590cbf0469bf41],
            [0x47f7f7bc95353e03, 0xf96c32bcfd058dfd],
            [0x3caaa3e8a99f9deb, 0x50f3af57adf622aa],
            [0x5e390f7df7a69296, 0xa7553dc10aa31f6b],
            [0x14f9701ae35fe28c, 0x440adf4d4ea9c026],
            [0x47438735a41c65b9, 0xe016baf4aebf7ad2],
            [0x549932d1f0855768, 0x1093ed9cbe2c974e],
            [0x13111d7fe3944a17, 0xf307a78b4d2b30c5],
        ],
    );

    let enc_keys = expand(&[
        0x69, 0x20, 0xe2, 0x99, 0xa5, 0x20, 0x2a, 0x6d, 0x65, 0x6e, 0x63, 0x68, 0x69, 0x74, 0x6f,
        0x2a,
    ])
    .0;
    check(
        &enc_keys,
        &[
            [0x6920e299a5202a6d, 0x656e636869746f2a],
            [0xfa8807605fa82d0d, 0x3ac64e6553b2214f],
            [0xcf75838d90ddae80, 0xaa1be0e5f9a9c1aa],
            [0x180d2f1488d08194, 0x22cb6171db62a0db],
            [0xbaed96ad323d1739, 0x10f67648cb94d693],
            [0x881b4ab2ba265d8b, 0xaad02bc36144fd50],
            [0xb34f195d096944d6, 0xa3b96f15c2fd9245],
            [0xa7007778ae6933ae, 0x0dd05cbbcf2dcefe],
            [0xff8bccf251e2ff5c, 0x5c32a3e7931f6d19],
            [0x24b7182e7555e772, 0x29674495ba78298c],
            [0xae127cdadb479ba8, 0xf220df3d4858f6b1],
        ],
    );

    let enc_keys = expand(&[
        0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf, 0x4f,
        0x3c,
    ])
    .0;
    check(
        &enc_keys,
        &[
            [0x2b7e151628aed2a6, 0xabf7158809cf4f3c],
            [0xa0fafe1788542cb1, 0x23a339392a6c7605],
            [0xf2c295f27a96b943, 0x5935807a7359f67f],
            [0x3d80477d4716fe3e, 0x1e237e446d7a883b],
            [0xef44a541a8525b7f, 0xb671253bdb0bad00],
            [0xd4d1c6f87c839d87, 0xcaf2b8bc11f915bc],
            [0x6d88a37a110b3efd, 0xdbf98641ca0093fd],
            [0x4e54f70e5f5fc9f3, 0x84a64fb24ea6dc4f],
            [0xead27321b58dbad2, 0x312bf5607f8d292f],
            [0xac7766f319fadc21, 0x28d12941575c006e],
            [0xd014f9a8c9ee2589, 0xe13f0cc8b6630ca6],
        ],
    );
}
