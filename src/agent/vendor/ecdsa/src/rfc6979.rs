//! Support for computing deterministic ECDSA ephemeral scalar (`k`).
//!
//! Implementation of the algorithm described in RFC 6979 (Section 3.2):
//! <https://tools.ietf.org/html/rfc6979#section-3>

use crate::hazmat::FromDigest;
use elliptic_curve::{
    ff::PrimeField,
    generic_array::GenericArray,
    ops::Invert,
    weierstrass::Curve,
    zeroize::{Zeroize, Zeroizing},
    FieldBytes, NonZeroScalar, ProjectiveArithmetic, Scalar,
};
use hmac::{Hmac, Mac, NewMac};
use signature::digest::{BlockInput, FixedOutput, Reset, Update};

/// Generate ephemeral scalar `k` from the secret scalar and a digest of the
/// input message.
pub fn generate_k<C, D>(
    secret_scalar: &NonZeroScalar<C>,
    msg_digest: D,
    additional_data: &[u8],
) -> Zeroizing<NonZeroScalar<C>>
where
    C: Curve + ProjectiveArithmetic,
    D: FixedOutput<OutputSize = C::FieldSize> + BlockInput + Clone + Default + Reset + Update,
    Scalar<C>:
        PrimeField<Repr = FieldBytes<C>> + FromDigest<C> + Invert<Output = Scalar<C>> + Zeroize,
{
    let mut x = secret_scalar.to_repr();
    let h1 = Scalar::<C>::from_digest(msg_digest).to_repr();
    let mut hmac_drbg = HmacDrbg::<D>::new(&x, &h1, additional_data);
    x.zeroize();

    loop {
        let mut tmp = FieldBytes::<C>::default();
        hmac_drbg.generate_into(&mut tmp);
        if let Some(k) = NonZeroScalar::from_repr(tmp) {
            return Zeroizing::new(k);
        }
    }
}

/// Internal implementation of `HMAC_DRBG` as described in NIST SP800-90A:
/// <https://csrc.nist.gov/publications/detail/sp/800-90a/rev-1/final>
///
/// This is a HMAC-based deterministic random bit generator used internally
/// to compute a deterministic ECDSA ephemeral scalar `k`.
// TODO(tarcieri): use `hmac-drbg` crate when sorpaas/rust-hmac-drbg#3 is merged
struct HmacDrbg<D>
where
    D: BlockInput + FixedOutput + Clone + Default + Reset + Update,
{
    /// HMAC key `K` (see RFC 6979 Section 3.2.c)
    k: Hmac<D>,

    /// Chaining value `V` (see RFC 6979 Section 3.2.c)
    v: GenericArray<u8, D::OutputSize>,
}

impl<D> HmacDrbg<D>
where
    D: BlockInput + FixedOutput + Clone + Default + Reset + Update,
{
    /// Initialize `HMAC_DRBG`
    pub fn new(entropy_input: &[u8], nonce: &[u8], additional_data: &[u8]) -> Self {
        let mut k = Hmac::new(&Default::default());
        let mut v = GenericArray::default();

        for b in &mut v {
            *b = 0x01;
        }

        for i in 0..=1 {
            k.update(&v);
            k.update(&[i]);
            k.update(entropy_input);
            k.update(nonce);
            k.update(additional_data);
            k = Hmac::new_from_slice(&k.finalize().into_bytes()).unwrap();

            // Steps 3.2.e,g: v = HMAC_k(v)
            k.update(&v);
            v = k.finalize_reset().into_bytes();
        }

        Self { k, v }
    }

    /// Get the next `HMAC_DRBG` output
    pub fn generate_into(&mut self, out: &mut [u8]) {
        for out_chunk in out.chunks_mut(self.v.len()) {
            self.k.update(&self.v);
            self.v = self.k.finalize_reset().into_bytes();
            out_chunk.copy_from_slice(&self.v[..out_chunk.len()]);
        }

        self.k.update(&self.v);
        self.k.update(&[0x00]);
        self.k = Hmac::new_from_slice(&self.k.finalize_reset().into_bytes()).unwrap();
        self.k.update(&self.v);
        self.v = self.k.finalize_reset().into_bytes();
    }
}

#[cfg(test)]
mod tests {
    use super::generate_k;
    use elliptic_curve::{dev::NonZeroScalar, ff::PrimeField};
    use hex_literal::hex;
    use sha2::{Digest, Sha256};

    /// Test vector from RFC 6979 Appendix 2.5 (NIST P-256 + SHA-256)
    /// <https://tools.ietf.org/html/rfc6979#appendix-A.2.5>
    #[test]
    fn appendix_2_5_test_vector() {
        let x = NonZeroScalar::from_repr(
            hex!("c9afa9d845ba75166b5c215767b1d6934e50c3db36e89b127b8a622b120f6721").into(),
        )
        .unwrap();

        let digest = Sha256::new().chain("sample");
        let k = generate_k(&x, digest, &[]);

        assert_eq!(
            k.to_repr().as_slice(),
            &hex!("a6e3c57dd01abe90086538398355dd4c3b17aa873382b0f24d6129493d8aad60")[..]
        );
    }
}
