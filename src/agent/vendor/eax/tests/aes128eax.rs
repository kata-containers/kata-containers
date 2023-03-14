//! Test vectors from Appendix G:
//! https://web.cs.ucdavis.edu/~rogaway/papers/eax.pdf
use aes::Aes128;
use eax::Eax;

aead::new_test!(aes128eax, "aes128eax", Eax<Aes128>);
