// Copyright (c) 2017 Martijn Rijkeboer <mrr@sru-systems.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// These tests are based on Argon's test.c test suite.

use argon2::{Config, Error, ThreadMode, Variant, Version};
use hex::ToHex;

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2d_version10_1() {
    hash_test(
        Variant::Argon2d,
        Version::Version10,
        2,
        65536,
        1,
        b"password",
        b"somesalt",
        "2ec0d925358f5830caf0c1cc8a3ee58b34505759428b859c79b72415f51f9221",
        "$argon2d$m=65536,t=2,p=1$c29tZXNhbHQ\
               $LsDZJTWPWDDK8MHMij7lizRQV1lCi4WcebckFfUfkiE",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2d_version10_2() {
    hash_test(
        Variant::Argon2d,
        Version::Version10,
        2,
        1048576,
        1,
        b"password",
        b"somesalt",
        "28ae0d1037991b55faec18425274d5f9169b2f44bee19b45b7a561d489d6019a",
        "$argon2d$m=1048576,t=2,p=1$c29tZXNhbHQ\
               $KK4NEDeZG1X67BhCUnTV+RabL0S+4ZtFt6Vh1InWAZo",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2d_version10_3() {
    hash_test(
        Variant::Argon2d,
        Version::Version10,
        2,
        262144,
        1,
        b"password",
        b"somesalt",
        "41c89760d85b80ba1be7e959ebd16390bfb4176db9466d70f670457ccade4ec8",
        "$argon2d$m=262144,t=2,p=1$c29tZXNhbHQ\
               $QciXYNhbgLob5+lZ69FjkL+0F225Rm1w9nBFfMreTsg",
    );
}

#[test]
fn test_argon2d_version10_4() {
    hash_test(
        Variant::Argon2d,
        Version::Version10,
        2,
        256,
        1,
        b"password",
        b"somesalt",
        "bd404868ff00c52e7543c8332e6a772a5724892d7e328d5cf253bbc8e726b371",
        "$argon2d$m=256,t=2,p=1$c29tZXNhbHQ\
               $vUBIaP8AxS51Q8gzLmp3KlckiS1+Mo1c8lO7yOcms3E",
    );
}

#[test]
fn test_argon2d_version10_5() {
    hash_test(
        Variant::Argon2d,
        Version::Version10,
        2,
        256,
        2,
        b"password",
        b"somesalt",
        "6a91d02b9f8854ba0841f04aa6e53c1d3374c0a0c646b8e431b03de805b91ec3",
        "$argon2d$m=256,t=2,p=2$c29tZXNhbHQ\
               $apHQK5+IVLoIQfBKpuU8HTN0wKDGRrjkMbA96AW5HsM",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2d_version10_6() {
    hash_test(
        Variant::Argon2d,
        Version::Version10,
        1,
        65536,
        1,
        b"password",
        b"somesalt",
        "05d1d0a85f499e9397b9c1c936b20f366a7273ccf259e2edfdb44ca8f86dd11f",
        "$argon2d$m=65536,t=1,p=1$c29tZXNhbHQ\
               $BdHQqF9JnpOXucHJNrIPNmpyc8zyWeLt/bRMqPht0R8",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2d_version10_7() {
    hash_test(
        Variant::Argon2d,
        Version::Version10,
        4,
        65536,
        1,
        b"password",
        b"somesalt",
        "99e787faced20949df5a3b9c8620712b45cfea061716adf8b13efb7feee44084",
        "$argon2d$m=65536,t=4,p=1$c29tZXNhbHQ\
               $meeH+s7SCUnfWjuchiBxK0XP6gYXFq34sT77f+7kQIQ",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2d_version10_8() {
    hash_test(
        Variant::Argon2d,
        Version::Version10,
        2,
        65536,
        1,
        b"differentpassword",
        b"somesalt",
        "b9c2e4a4ec9a6db99ca4e2b549dd64256305b95754b0a70ad757cd1ff7eed4a4",
        "$argon2d$m=65536,t=2,p=1$c29tZXNhbHQ\
               $ucLkpOyabbmcpOK1Sd1kJWMFuVdUsKcK11fNH/fu1KQ",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2d_version10_9() {
    hash_test(
        Variant::Argon2d,
        Version::Version10,
        2,
        65536,
        1,
        b"password",
        b"diffsalt",
        "d1da14196c233e0a3cb687aa6e6c465cb50669ffdb891158c2b75722395f14d5",
        "$argon2d$m=65536,t=2,p=1$ZGlmZnNhbHQ\
               $0doUGWwjPgo8toeqbmxGXLUGaf/biRFYwrdXIjlfFNU",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2d_version13_1() {
    hash_test(
        Variant::Argon2d,
        Version::Version13,
        2,
        65536,
        1,
        b"password",
        b"somesalt",
        "955e5d5b163a1b60bba35fc36d0496474fba4f6b59ad53628666f07fb2f93eaf",
        "$argon2d$v=19$m=65536,t=2,p=1$c29tZXNhbHQ\
               $lV5dWxY6G2C7o1/DbQSWR0+6T2tZrVNihmbwf7L5Pq8",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2d_version13_2() {
    hash_test(
        Variant::Argon2d,
        Version::Version13,
        2,
        1048576,
        1,
        b"password",
        b"somesalt",
        "c2d680a6e7ac1b75e2c2b1e5e71b1701584869492efea4a76ccc91fec622d1ae",
        "$argon2d$v=19$m=1048576,t=2,p=1$c29tZXNhbHQ\
               $wtaApuesG3XiwrHl5xsXAVhIaUku/qSnbMyR/sYi0a4",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2d_version13_3() {
    hash_test(
        Variant::Argon2d,
        Version::Version13,
        2,
        262144,
        1,
        b"password",
        b"somesalt",
        "9678b3379ce20ddc96fa38f045c8cdc96282287ff2848efd867582b9528a2155",
        "$argon2d$v=19$m=262144,t=2,p=1$c29tZXNhbHQ\
               $lnizN5ziDdyW+jjwRcjNyWKCKH/yhI79hnWCuVKKIVU",
    );
}

#[test]
fn test_argon2d_version13_4() {
    hash_test(
        Variant::Argon2d,
        Version::Version13,
        2,
        256,
        1,
        b"password",
        b"somesalt",
        "25c4ee8ba448054b49efc804e478b9d823be1f9bd2e99f51d6ec4007a3a1501f",
        "$argon2d$v=19$m=256,t=2,p=1$c29tZXNhbHQ\
               $JcTui6RIBUtJ78gE5Hi52CO+H5vS6Z9R1uxAB6OhUB8",
    );
}

#[test]
fn test_argon2d_version13_5() {
    hash_test(
        Variant::Argon2d,
        Version::Version13,
        2,
        256,
        2,
        b"password",
        b"somesalt",
        "7b69c92d7c3889aad1281dbc8baefc12cc37c80f1c75e33ef2c2d40c28ebc573",
        "$argon2d$v=19$m=256,t=2,p=2$c29tZXNhbHQ\
               $e2nJLXw4iarRKB28i678Esw3yA8cdeM+8sLUDCjrxXM",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2d_version13_6() {
    hash_test(
        Variant::Argon2d,
        Version::Version13,
        1,
        65536,
        1,
        b"password",
        b"somesalt",
        "8193708c030f09b121526b0efdffce738fef0ddb13937a65d43f3f1eab9aa802",
        "$argon2d$v=19$m=65536,t=1,p=1$c29tZXNhbHQ\
               $gZNwjAMPCbEhUmsO/f/Oc4/vDdsTk3pl1D8/HquaqAI",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2d_version13_7() {
    hash_test(
        Variant::Argon2d,
        Version::Version13,
        4,
        65536,
        1,
        b"password",
        b"somesalt",
        "cd1e2816cf8895fffe535b87b7a0aa3612f73c6ce063de83b1e7b4621ca0afe6",
        "$argon2d$v=19$m=65536,t=4,p=1$c29tZXNhbHQ\
               $zR4oFs+Ilf/+U1uHt6CqNhL3PGzgY96Dsee0Yhygr+Y",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2d_version13_8() {
    hash_test(
        Variant::Argon2d,
        Version::Version13,
        2,
        65536,
        1,
        b"differentpassword",
        b"somesalt",
        "a34dafc893182d521ae467bbfdce60f973a1223c4615654efeb0bfef8a930e25",
        "$argon2d$v=19$m=65536,t=2,p=1$c29tZXNhbHQ\
               $o02vyJMYLVIa5Ge7/c5g+XOhIjxGFWVO/rC/74qTDiU",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2d_version13_9() {
    hash_test(
        Variant::Argon2d,
        Version::Version13,
        2,
        65536,
        1,
        b"password",
        b"diffsalt",
        "f00eb8a999a1c6949c61fbbee3c3cece5066e3ff5c79aed421266055e2bf4dd7",
        "$argon2d$v=19$m=65536,t=2,p=1$ZGlmZnNhbHQ\
               $8A64qZmhxpScYfu+48POzlBm4/9cea7UISZgVeK/Tdc",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2i_version10_1() {
    hash_test(
        Variant::Argon2i,
        Version::Version10,
        2,
        65536,
        1,
        b"password",
        b"somesalt",
        "f6c4db4a54e2a370627aff3db6176b94a2a209a62c8e36152711802f7b30c694",
        "$argon2i$m=65536,t=2,p=1$c29tZXNhbHQ\
               $9sTbSlTio3Biev89thdrlKKiCaYsjjYVJxGAL3swxpQ",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2i_version10_2() {
    hash_test(
        Variant::Argon2i,
        Version::Version10,
        2,
        1048576,
        1,
        b"password",
        b"somesalt",
        "9690ec55d28d3ed32562f2e73ea62b02b018757643a2ae6e79528459de8106e9",
        "$argon2i$m=1048576,t=2,p=1$c29tZXNhbHQ\
               $lpDsVdKNPtMlYvLnPqYrArAYdXZDoq5ueVKEWd6BBuk",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2i_version10_3() {
    hash_test(
        Variant::Argon2i,
        Version::Version10,
        2,
        262144,
        1,
        b"password",
        b"somesalt",
        "3e689aaa3d28a77cf2bc72a51ac53166761751182f1ee292e3f677a7da4c2467",
        "$argon2i$m=262144,t=2,p=1$c29tZXNhbHQ\
               $Pmiaqj0op3zyvHKlGsUxZnYXURgvHuKS4/Z3p9pMJGc",
    );
}

#[test]
fn test_argon2i_version10_4() {
    hash_test(
        Variant::Argon2i,
        Version::Version10,
        2,
        256,
        1,
        b"password",
        b"somesalt",
        "fd4dd83d762c49bdeaf57c47bdcd0c2f1babf863fdeb490df63ede9975fccf06",
        "$argon2i$m=256,t=2,p=1$c29tZXNhbHQ\
               $/U3YPXYsSb3q9XxHvc0MLxur+GP960kN9j7emXX8zwY",
    );
}

#[test]
fn test_argon2i_version10_5() {
    hash_test(
        Variant::Argon2i,
        Version::Version10,
        2,
        256,
        2,
        b"password",
        b"somesalt",
        "b6c11560a6a9d61eac706b79a2f97d68b4463aa3ad87e00c07e2b01e90c564fb",
        "$argon2i$m=256,t=2,p=2$c29tZXNhbHQ\
               $tsEVYKap1h6scGt5ovl9aLRGOqOth+AMB+KwHpDFZPs",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2i_version10_6() {
    hash_test(
        Variant::Argon2i,
        Version::Version10,
        1,
        65536,
        1,
        b"password",
        b"somesalt",
        "81630552b8f3b1f48cdb1992c4c678643d490b2b5eb4ff6c4b3438b5621724b2",
        "$argon2i$m=65536,t=1,p=1$c29tZXNhbHQ\
               $gWMFUrjzsfSM2xmSxMZ4ZD1JCytetP9sSzQ4tWIXJLI",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2i_version10_7() {
    hash_test(
        Variant::Argon2i,
        Version::Version10,
        4,
        65536,
        1,
        b"password",
        b"somesalt",
        "f212f01615e6eb5d74734dc3ef40ade2d51d052468d8c69440a3a1f2c1c2847b",
        "$argon2i$m=65536,t=4,p=1$c29tZXNhbHQ\
               $8hLwFhXm6110c03D70Ct4tUdBSRo2MaUQKOh8sHChHs",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2i_version10_8() {
    hash_test(
        Variant::Argon2i,
        Version::Version10,
        2,
        65536,
        1,
        b"differentpassword",
        b"somesalt",
        "e9c902074b6754531a3a0be519e5baf404b30ce69b3f01ac3bf21229960109a3",
        "$argon2i$m=65536,t=2,p=1$c29tZXNhbHQ\
               $6ckCB0tnVFMaOgvlGeW69ASzDOabPwGsO/ISKZYBCaM",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2i_version10_9() {
    hash_test(
        Variant::Argon2i,
        Version::Version10,
        2,
        65536,
        1,
        b"password",
        b"diffsalt",
        "79a103b90fe8aef8570cb31fc8b22259778916f8336b7bdac3892569d4f1c497",
        "$argon2i$m=65536,t=2,p=1$ZGlmZnNhbHQ\
               $eaEDuQ/orvhXDLMfyLIiWXeJFvgza3vaw4kladTxxJc",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2i_version13_1() {
    hash_test(
        Variant::Argon2i,
        Version::Version13,
        2,
        65536,
        1,
        b"password",
        b"somesalt",
        "c1628832147d9720c5bd1cfd61367078729f6dfb6f8fea9ff98158e0d7816ed0",
        "$argon2i$v=19$m=65536,t=2,p=1$c29tZXNhbHQ\
               $wWKIMhR9lyDFvRz9YTZweHKfbftvj+qf+YFY4NeBbtA",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2i_version13_2() {
    hash_test(
        Variant::Argon2i,
        Version::Version13,
        2,
        1048576,
        1,
        b"password",
        b"somesalt",
        "d1587aca0922c3b5d6a83edab31bee3c4ebaef342ed6127a55d19b2351ad1f41",
        "$argon2i$v=19$m=1048576,t=2,p=1$c29tZXNhbHQ\
               $0Vh6ygkiw7XWqD7asxvuPE667zQu1hJ6VdGbI1GtH0E",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2i_version13_3() {
    hash_test(
        Variant::Argon2i,
        Version::Version13,
        2,
        262144,
        1,
        b"password",
        b"somesalt",
        "296dbae80b807cdceaad44ae741b506f14db0959267b183b118f9b24229bc7cb",
        "$argon2i$v=19$m=262144,t=2,p=1$c29tZXNhbHQ\
               $KW266AuAfNzqrUSudBtQbxTbCVkmexg7EY+bJCKbx8s",
    );
}

#[test]
fn test_argon2i_version13_4() {
    hash_test(
        Variant::Argon2i,
        Version::Version13,
        2,
        256,
        1,
        b"password",
        b"somesalt",
        "89e9029f4637b295beb027056a7336c414fadd43f6b208645281cb214a56452f",
        "$argon2i$v=19$m=256,t=2,p=1$c29tZXNhbHQ\
               $iekCn0Y3spW+sCcFanM2xBT63UP2sghkUoHLIUpWRS8",
    );
}

#[test]
fn test_argon2i_version13_5() {
    hash_test(
        Variant::Argon2i,
        Version::Version13,
        2,
        256,
        2,
        b"password",
        b"somesalt",
        "4ff5ce2769a1d7f4c8a491df09d41a9fbe90e5eb02155a13e4c01e20cd4eab61",
        "$argon2i$v=19$m=256,t=2,p=2$c29tZXNhbHQ\
               $T/XOJ2mh1/TIpJHfCdQan76Q5esCFVoT5MAeIM1Oq2E",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2i_version13_6() {
    hash_test(
        Variant::Argon2i,
        Version::Version13,
        1,
        65536,
        1,
        b"password",
        b"somesalt",
        "d168075c4d985e13ebeae560cf8b94c3b5d8a16c51916b6f4ac2da3ac11bbecf",
        "$argon2i$v=19$m=65536,t=1,p=1$c29tZXNhbHQ\
               $0WgHXE2YXhPr6uVgz4uUw7XYoWxRkWtvSsLaOsEbvs8",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2i_version13_7() {
    hash_test(
        Variant::Argon2i,
        Version::Version13,
        4,
        65536,
        1,
        b"password",
        b"somesalt",
        "aaa953d58af3706ce3df1aefd4a64a84e31d7f54175231f1285259f88174ce5b",
        "$argon2i$v=19$m=65536,t=4,p=1$c29tZXNhbHQ\
               $qqlT1YrzcGzj3xrv1KZKhOMdf1QXUjHxKFJZ+IF0zls",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2i_version13_8() {
    hash_test(
        Variant::Argon2i,
        Version::Version13,
        2,
        65536,
        1,
        b"differentpassword",
        b"somesalt",
        "14ae8da01afea8700c2358dcef7c5358d9021282bd88663a4562f59fb74d22ee",
        "$argon2i$v=19$m=65536,t=2,p=1$c29tZXNhbHQ\
               $FK6NoBr+qHAMI1jc73xTWNkCEoK9iGY6RWL1n7dNIu4",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2i_version13_9() {
    hash_test(
        Variant::Argon2i,
        Version::Version13,
        2,
        65536,
        1,
        b"password",
        b"diffsalt",
        "b0357cccfbef91f3860b0dba447b2348cbefecadaf990abfe9cc40726c521271",
        "$argon2i$v=19$m=65536,t=2,p=1$ZGlmZnNhbHQ\
               $sDV8zPvvkfOGCw26RHsjSMvv7K2vmQq/6cxAcmxSEnE",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2id_version10_1() {
    hash_test(
        Variant::Argon2id,
        Version::Version10,
        2,
        65536,
        1,
        b"password",
        b"somesalt",
        "980ebd24a4e667f16346f9d4a78b175728783613e0cc6fb17c2ec884b16435df",
        "$argon2id$v=16$m=65536,t=2,p=1$c29tZXNhbHQ\
               $mA69JKTmZ/FjRvnUp4sXVyh4NhPgzG+xfC7IhLFkNd8",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2id_version10_2() {
    hash_test(
        Variant::Argon2id,
        Version::Version10,
        2,
        1048576,
        1,
        b"password",
        b"somesalt",
        "ce622fc2053acd224c06e7be122f9e9554b62b8a414b032056b46123422e32ab",
        "$argon2id$v=16$m=1048576,t=2,p=1$c29tZXNhbHQ\
               $zmIvwgU6zSJMBue+Ei+elVS2K4pBSwMgVrRhI0IuMqs",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2id_version10_3() {
    hash_test(
        Variant::Argon2id,
        Version::Version10,
        2,
        262144,
        1,
        b"password",
        b"somesalt",
        "01cbafb58f2dc764adced1ab9a90fcb62f6ed8a066023b6ba0c204db6c9b90cd",
        "$argon2id$v=16$m=262144,t=2,p=1$c29tZXNhbHQ\
               $AcuvtY8tx2StztGrmpD8ti9u2KBmAjtroMIE22ybkM0",
    );
}

#[test]
fn test_argon2id_version10_4() {
    hash_test(
        Variant::Argon2id,
        Version::Version10,
        2,
        256,
        1,
        b"password",
        b"somesalt",
        "da070e576e50f2f38a3c897cbddc6c7fb4028e870971ff9eae7b4e1879295e6e",
        "$argon2id$v=16$m=256,t=2,p=1$c29tZXNhbHQ\
               $2gcOV25Q8vOKPIl8vdxsf7QCjocJcf+erntOGHkpXm4",
    );
}

#[test]
fn test_argon2id_version10_5() {
    hash_test(
        Variant::Argon2id,
        Version::Version10,
        2,
        256,
        2,
        b"password",
        b"somesalt",
        "f8aabb5315c63cddcdb3b4a021550928e525699da8fcbd1c2b0b1ccd35cc87a7",
        "$argon2id$v=16$m=256,t=2,p=2$c29tZXNhbHQ\
               $+Kq7UxXGPN3Ns7SgIVUJKOUlaZ2o/L0cKwsczTXMh6c",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2id_version10_6() {
    hash_test(
        Variant::Argon2id,
        Version::Version10,
        1,
        65536,
        1,
        b"password",
        b"somesalt",
        "8e0f310407a989013a823defbd6cbae54d86b35d1ac16e4f88eb4645e1357956",
        "$argon2id$v=16$m=65536,t=1,p=1$c29tZXNhbHQ\
               $jg8xBAepiQE6gj3vvWy65U2Gs10awW5PiOtGReE1eVY",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2id_version10_7() {
    hash_test(
        Variant::Argon2id,
        Version::Version10,
        4,
        65536,
        1,
        b"password",
        b"somesalt",
        "16653426e582143da6a0ff75daac9fa94c21ac2c221c96bd3645a5c15c2d962a",
        "$argon2id$v=16$m=65536,t=4,p=1$c29tZXNhbHQ\
               $FmU0JuWCFD2moP912qyfqUwhrCwiHJa9NkWlwVwtlio",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2id_version10_8() {
    hash_test(
        Variant::Argon2id,
        Version::Version10,
        2,
        65536,
        1,
        b"differentpassword",
        b"somesalt",
        "75709d4c96a2c235696a77eefeecf33e64221f470b26177c19e5e27642612dbb",
        "$argon2id$v=16$m=65536,t=2,p=1$c29tZXNhbHQ\
               $dXCdTJaiwjVpanfu/uzzPmQiH0cLJhd8GeXidkJhLbs",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2id_version10_9() {
    hash_test(
        Variant::Argon2id,
        Version::Version10,
        2,
        65536,
        1,
        b"password",
        b"diffsalt",
        "f0b57cfe03a9d78c4700e2fe802002343660a56b1b5d21ead788e6bb44ef48d3",
        "$argon2id$v=16$m=65536,t=2,p=1$ZGlmZnNhbHQ\
               $8LV8/gOp14xHAOL+gCACNDZgpWsbXSHq14jmu0TvSNM",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2id_version13_1() {
    hash_test(
        Variant::Argon2id,
        Version::Version13,
        2,
        65536,
        1,
        b"password",
        b"somesalt",
        "09316115d5cf24ed5a15a31a3ba326e5cf32edc24702987c02b6566f61913cf7",
        "$argon2id$v=19$m=65536,t=2,p=1$c29tZXNhbHQ\
               $CTFhFdXPJO1aFaMaO6Mm5c8y7cJHAph8ArZWb2GRPPc",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2id_version13_2() {
    hash_test(
        Variant::Argon2id,
        Version::Version13,
        2,
        1048576,
        1,
        b"password",
        b"somesalt",
        "16a1cb8a061cf1f8b16fb195fe559fa6d1435e43dded2f4fb07293176be892fa",
        "$argon2id$v=19$m=1048576,t=2,p=1$c29tZXNhbHQ\
               $FqHLigYc8fixb7GV/lWfptFDXkPd7S9PsHKTF2vokvo",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2id_version13_3() {
    hash_test(
        Variant::Argon2id,
        Version::Version13,
        2,
        262144,
        1,
        b"password",
        b"somesalt",
        "78fe1ec91fb3aa5657d72e710854e4c3d9b9198c742f9616c2f085bed95b2e8c",
        "$argon2id$v=19$m=262144,t=2,p=1$c29tZXNhbHQ\
               $eP4eyR+zqlZX1y5xCFTkw9m5GYx0L5YWwvCFvtlbLow",
    );
}

#[test]
fn test_argon2id_version13_4() {
    hash_test(
        Variant::Argon2id,
        Version::Version13,
        2,
        256,
        1,
        b"password",
        b"somesalt",
        "9dfeb910e80bad0311fee20f9c0e2b12c17987b4cac90c2ef54d5b3021c68bfe",
        "$argon2id$v=19$m=256,t=2,p=1$c29tZXNhbHQ\
               $nf65EOgLrQMR/uIPnA4rEsF5h7TKyQwu9U1bMCHGi/4",
    );
}

#[test]
fn test_argon2id_version13_5() {
    hash_test(
        Variant::Argon2id,
        Version::Version13,
        2,
        256,
        2,
        b"password",
        b"somesalt",
        "6d093c501fd5999645e0ea3bf620d7b8be7fd2db59c20d9fff9539da2bf57037",
        "$argon2id$v=19$m=256,t=2,p=2$c29tZXNhbHQ\
               $bQk8UB/VmZZF4Oo79iDXuL5/0ttZwg2f/5U52iv1cDc",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2id_version13_6() {
    hash_test(
        Variant::Argon2id,
        Version::Version13,
        1,
        65536,
        1,
        b"password",
        b"somesalt",
        "f6a5adc1ba723dddef9b5ac1d464e180fcd9dffc9d1cbf76cca2fed795d9ca98",
        "$argon2id$v=19$m=65536,t=1,p=1$c29tZXNhbHQ\
               $9qWtwbpyPd3vm1rB1GThgPzZ3/ydHL92zKL+15XZypg",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2id_version13_7() {
    hash_test(
        Variant::Argon2id,
        Version::Version13,
        4,
        65536,
        1,
        b"password",
        b"somesalt",
        "9025d48e68ef7395cca9079da4c4ec3affb3c8911fe4f86d1a2520856f63172c",
        "$argon2id$v=19$m=65536,t=4,p=1$c29tZXNhbHQ\
               $kCXUjmjvc5XMqQedpMTsOv+zyJEf5PhtGiUghW9jFyw",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2id_version13_8() {
    hash_test(
        Variant::Argon2id,
        Version::Version13,
        2,
        65536,
        1,
        b"differentpassword",
        b"somesalt",
        "0b84d652cf6b0c4beaef0dfe278ba6a80df6696281d7e0d2891b817d8c458fde",
        "$argon2id$v=19$m=65536,t=2,p=1$c29tZXNhbHQ\
               $C4TWUs9rDEvq7w3+J4umqA32aWKB1+DSiRuBfYxFj94",
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn test_argon2id_version13_9() {
    hash_test(
        Variant::Argon2id,
        Version::Version13,
        2,
        65536,
        1,
        b"password",
        b"diffsalt",
        "bdf32b05ccc42eb15d58fd19b1f856b113da1e9a5874fdcc544308565aa8141c",
        "$argon2id$v=19$m=65536,t=2,p=1$ZGlmZnNhbHQ\
               $vfMrBczELrFdWP0ZsfhWsRPaHppYdP3MVEMIVlqoFBw",
    );
}

#[test]
fn test_verify_encoded_with_missing_dollar_before_salt_version10() {
    let encoded = "$argon2i$m=65536,t=2,p=1c29tZXNhbHQ\
                   $9sTbSlTio3Biev89thdrlKKiCaYsjjYVJxGAL3swxpQ";
    let password = b"password";
    let res = argon2::verify_encoded(encoded, password);
    assert_eq!(res, Err(Error::DecodingFail));
}

#[test]
fn test_verify_encoded_with_missing_dollar_before_salt_version13() {
    let encoded = "$argon2i$v=19$m=65536,t=2,p=1c29tZXNhbHQ\
                   $wWKIMhR9lyDFvRz9YTZweHKfbftvj+qf+YFY4NeBbtA";
    let password = b"password";
    let res = argon2::verify_encoded(encoded, password);
    assert_eq!(res, Err(Error::DecodingFail));
}

#[test]
fn test_verify_encoded_with_missing_dollar_before_hash_version10() {
    let encoded = "$argon2i$m=65536,t=2,p=1$c29tZXNhbHQ\
                   9sTbSlTio3Biev89thdrlKKiCaYsjjYVJxGAL3swxpQ";
    let password = b"password";
    let res = argon2::verify_encoded(encoded, password);
    assert_eq!(res, Err(Error::DecodingFail));
}

#[test]
fn test_verify_encoded_with_missing_dollar_before_hash_version13() {
    let encoded = "$argon2i$v=19$m=65536,t=2,p=1$c29tZXNhbHQ\
                   wWKIMhR9lyDFvRz9YTZweHKfbftvj+qf+YFY4NeBbtA";
    let password = b"password";
    let res = argon2::verify_encoded(encoded, password);
    assert_eq!(res, Err(Error::DecodingFail));
}

#[test]
fn test_verify_encoded_with_too_short_salt_version10() {
    let encoded = "$argon2i$m=65536,t=2,p=1$\
                   $9sTbSlTio3Biev89thdrlKKiCaYsjjYVJxGAL3swxpQ";
    let password = b"password";
    let res = argon2::verify_encoded(encoded, password);
    assert_eq!(res, Err(Error::SaltTooShort));
}

#[test]
fn test_verify_encoded_with_too_short_salt_version13() {
    let encoded = "$argon2i$v=19$m=65536,t=2,p=1$\
                   $9sTbSlTio3Biev89thdrlKKiCaYsjjYVJxGAL3swxpQ";
    let password = b"password";
    let res = argon2::verify_encoded(encoded, password);
    assert_eq!(res, Err(Error::SaltTooShort));
}

#[cfg(not(debug_assertions))]
#[test]
fn test_verify_encoded_with_wrong_password_version10() {
    let encoded = "$argon2i$m=65536,t=2,p=1$c29tZXNhbHQ\
                   $b2G3seW+uPzerwQQC+/E1K50CLLO7YXy0JRcaTuswRo";
    let password = b"password"; // should be passwore
    let res = argon2::verify_encoded(encoded, password);
    assert_eq!(res, Ok(false));
}

#[cfg(not(debug_assertions))]
#[test]
fn test_verify_encoded_with_wrong_password_version13() {
    let encoded = "$argon2i$v=19$m=65536,t=2,p=1$c29tZXNhbHQ\
                   $8iIuixkI73Js3G1uMbezQXD0b8LG4SXGsOwoQkdAQIM";
    let password = b"password"; // should be passwore
    let res = argon2::verify_encoded(encoded, password);
    assert_eq!(res, Ok(false));
}

#[test]
fn test_encoded_len_returns_correct_length() {
    assert_eq!(argon2::encoded_len(Variant::Argon2d, 256, 1, 1, 8, 32), 83);
    assert_eq!(
        argon2::encoded_len(Variant::Argon2i, 4096, 10, 10, 8, 32),
        86
    );
    assert_eq!(
        argon2::encoded_len(Variant::Argon2id, 65536, 100, 10, 8, 32),
        89
    );
}

#[test]
fn test_hash_raw_with_not_enough_memory() {
    let pwd = b"password";
    let salt = b"diffsalt";
    let config = Config {
        variant: Variant::Argon2i,
        version: Version::Version13,
        mem_cost: 2,
        time_cost: 2,
        lanes: 1,
        thread_mode: ThreadMode::Sequential,
        secret: &[],
        ad: &[],
        hash_length: 32,
    };
    let res = argon2::hash_raw(pwd, salt, &config);
    assert_eq!(res, Err(Error::MemoryTooLittle));
}

#[test]
fn test_hash_raw_with_too_short_salt() {
    let pwd = b"password";
    let salt = b"s";
    let config = Config {
        variant: Variant::Argon2i,
        version: Version::Version13,
        mem_cost: 2048,
        time_cost: 2,
        lanes: 1,
        thread_mode: ThreadMode::Sequential,
        secret: &[],
        ad: &[],
        hash_length: 32,
    };
    let res = argon2::hash_raw(pwd, salt, &config);
    assert_eq!(res, Err(Error::SaltTooShort));
}

fn hash_test(
    var: Variant,
    ver: Version,
    t: u32,
    m: u32,
    p: u32,
    pwd: &[u8],
    salt: &[u8],
    hex: &str,
    enc: &str,
) {
    let threads = if cfg!(feature = "crossbeam-utils") { p } else { 1 };
    let config = Config {
        variant: var,
        version: ver,
        mem_cost: m,
        time_cost: t,
        lanes: p,
        thread_mode: ThreadMode::from_threads(threads),
        secret: &[],
        ad: &[],
        hash_length: 32,
    };
    let hash = argon2::hash_raw(pwd, salt, &config).unwrap();
    let hex_str = hash.encode_hex::<String>();
    assert_eq!(hex_str.as_str(), hex);

    let encoded = argon2::hash_encoded(pwd, salt, &config).unwrap();
    let result = argon2::verify_encoded(encoded.as_str(), pwd).unwrap();
    assert!(result);

    let result = argon2::verify_encoded(enc, pwd).unwrap();
    assert!(result);
}
