//! Checks that we defined reader type aliases correctly
#![allow(dead_code)]
use sha3::digest::ExtendableOutput;

fn shake128(v: sha3::Shake128) -> sha3::Shake128Reader {
    v.finalize_xof()
}

fn shake256(v: sha3::Shake256) -> sha3::Shake256Reader {
    v.finalize_xof()
}

fn cshake128(v: sha3::CShake128) -> sha3::CShake128Reader {
    v.finalize_xof()
}

fn cshake256(v: sha3::CShake256) -> sha3::CShake256Reader {
    v.finalize_xof()
}
