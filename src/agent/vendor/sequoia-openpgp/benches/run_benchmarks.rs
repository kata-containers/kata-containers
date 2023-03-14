use criterion::criterion_main;

mod common;

mod sign_message;
use sign_message::benches as sign;
mod verify_message;
use verify_message::benches as verify;
mod encrypt_message;
use encrypt_message::benches as encrypt;
mod decrypt_message;
use decrypt_message::benches as decrypt;
mod encrypt_sign_message;
use encrypt_sign_message::benches as encrypt_sign;
mod decrypt_verify_message;
use decrypt_verify_message::benches as decrypt_verify;
mod generate_cert;
use generate_cert::benches as generate_cert;
mod parse_cert;
use parse_cert::benches as parse_cert;
mod merge_cert;
use merge_cert::benches as merge_cert;

// Add all benchmark functions here
criterion_main!(
    sign,
    verify,
    encrypt_sign,
    decrypt_verify,
    encrypt,
    decrypt,
    generate_cert,
    parse_cert,
    merge_cert,
);
