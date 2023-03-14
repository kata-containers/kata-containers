//! Test vectors from GOST R 34.13-2015:
//! https://tc26.ru/standard/gost/GOST_R_3413-2015.pdf
use cmac::{Cmac, Mac, NewMac};
use hex_literal::hex;
use kuznyechik::Kuznyechik;
use magma::Magma;

#[test]
#[rustfmt::skip]
fn cmac_kuznyechik() {
    let key = hex!("
        8899aabbccddeeff0011223344556677
        fedcba98765432100123456789abcdef
    ");
    let pt = hex!("
        1122334455667700ffeeddccbbaa9988
        00112233445566778899aabbcceeff0a
        112233445566778899aabbcceeff0a00
        2233445566778899aabbcceeff0a0011
    ");
    let mac_res = hex!("
        336f4d296059fbe34ddeb35b37749c67
    ");
    let mut mac = Cmac::<Kuznyechik>::new_varkey(&key).unwrap();
    mac.update(&pt);
    let tag_bytes = mac.finalize().into_bytes();
    assert_eq!(&tag_bytes[..mac_res.len()], &mac_res);
}

#[test]
#[rustfmt::skip]
fn cmac_magma() {
    let key = hex!("
        ffeeddccbbaa99887766554433221100
        f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff
    ");
    let pt = hex!("
        92def06b3c130a59db54c704f8189d20
        4a98fb2e67a8024c8912409b17b57e41
    ");
    let mac_res = hex!("154e72102030c5bb");
    let mut mac = Cmac::<Magma>::new_varkey(&key).unwrap();
    mac.update(&pt);
    let tag_bytes = mac.finalize().into_bytes();
    assert_eq!(&tag_bytes[..mac_res.len()], &mac_res);
}
