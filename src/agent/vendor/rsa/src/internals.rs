use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use num_bigint::{BigInt, BigUint, IntoBigInt, IntoBigUint, ModInverse, RandBigInt, ToBigInt};
use num_traits::{One, Signed, Zero};
use rand_core::{CryptoRng, RngCore};
use zeroize::Zeroize;

use crate::errors::{Error, Result};
use crate::key::{PublicKeyParts, RsaPrivateKey};

/// Raw RSA encryption of m with the public key. No padding is performed.
#[inline]
pub fn encrypt<K: PublicKeyParts>(key: &K, m: &BigUint) -> BigUint {
    m.modpow(key.e(), key.n())
}

/// Performs raw RSA decryption with no padding, resulting in a plaintext `BigUint`.
/// Peforms RSA blinding if an `Rng` is passed.
#[inline]
pub fn decrypt<R: RngCore + CryptoRng>(
    mut rng: Option<&mut R>,
    priv_key: &RsaPrivateKey,
    c: &BigUint,
) -> Result<BigUint> {
    if c >= priv_key.n() {
        return Err(Error::Decryption);
    }

    if priv_key.n().is_zero() {
        return Err(Error::Decryption);
    }

    let mut ir = None;

    let c = if let Some(ref mut rng) = rng {
        let (blinded, unblinder) = blind(rng, priv_key, c);
        ir = Some(unblinder);
        Cow::Owned(blinded)
    } else {
        Cow::Borrowed(c)
    };

    let m = match priv_key.precomputed {
        None => c.modpow(priv_key.d(), priv_key.n()),
        Some(ref precomputed) => {
            // We have the precalculated values needed for the CRT.

            let p = &priv_key.primes()[0];
            let q = &priv_key.primes()[1];

            let mut m = c.modpow(&precomputed.dp, p).into_bigint().unwrap();
            let mut m2 = c.modpow(&precomputed.dq, q).into_bigint().unwrap();

            m -= &m2;

            let mut primes: Vec<_> = priv_key
                .primes()
                .iter()
                .map(ToBigInt::to_bigint)
                .map(Option::unwrap)
                .collect();

            while m.is_negative() {
                m += &primes[0];
            }
            m *= &precomputed.qinv;
            m %= &primes[0];
            m *= &primes[1];
            m += &m2;

            let mut c = c.into_owned().into_bigint().unwrap();
            for (i, value) in precomputed.crt_values.iter().enumerate() {
                let prime = &primes[2 + i];
                m2 = c.modpow(&value.exp, prime);
                m2 -= &m;
                m2 *= &value.coeff;
                m2 %= prime;
                while m2.is_negative() {
                    m2 += prime;
                }
                m2 *= &value.r;
                m += &m2;
            }

            // clear tmp values
            for prime in primes.iter_mut() {
                prime.zeroize();
            }
            primes.clear();
            c.zeroize();
            m2.zeroize();

            m.into_biguint().expect("failed to decrypt")
        }
    };

    match ir {
        Some(ref ir) => {
            // unblind
            Ok(unblind(priv_key, &m, &ir))
        }
        None => Ok(m),
    }
}

/// Performs RSA decryption, resulting in a plaintext `BigUint`.
/// Peforms RSA blinding if an `Rng` is passed.
/// This will also check for errors in the CRT computation.
#[inline]
pub fn decrypt_and_check<R: RngCore + CryptoRng>(
    rng: Option<&mut R>,
    priv_key: &RsaPrivateKey,
    c: &BigUint,
) -> Result<BigUint> {
    let m = decrypt(rng, priv_key, c)?;

    // In order to defend against errors in the CRT computation, m^e is
    // calculated, which should match the original ciphertext.
    let check = encrypt(priv_key, &m);

    if c != &check {
        return Err(Error::Internal);
    }

    Ok(m)
}

/// Returns the blinded c, along with the unblinding factor.
pub fn blind<R: RngCore + CryptoRng, K: PublicKeyParts>(
    rng: &mut R,
    key: &K,
    c: &BigUint,
) -> (BigUint, BigUint) {
    // Blinding involves multiplying c by r^e.
    // Then the decryption operation performs (m^e * r^e)^d mod n
    // which equals mr mod n. The factor of r can then be removed
    // by multiplying by the multiplicative inverse of r.

    let mut r: BigUint;
    let mut ir: Option<BigInt>;
    let unblinder;
    loop {
        r = rng.gen_biguint_below(key.n());
        if r.is_zero() {
            r = BigUint::one();
        }
        ir = r.clone().mod_inverse(key.n());
        if let Some(ir) = ir {
            if let Some(ub) = ir.into_biguint() {
                unblinder = ub;
                break;
            }
        }
    }

    let c = {
        let mut rpowe = r.modpow(key.e(), key.n()); // N != 0
        let mut c = c * &rpowe;
        c %= key.n();

        rpowe.zeroize();

        c
    };

    (c, unblinder)
}

/// Given an m and and unblinding factor, unblind the m.
pub fn unblind(key: impl PublicKeyParts, m: &BigUint, unblinder: &BigUint) -> BigUint {
    (m * unblinder) % key.n()
}

/// Returns a new vector of the given length, with 0s left padded.
#[inline]
pub fn left_pad(input: &[u8], size: usize) -> Vec<u8> {
    let n = if input.len() > size {
        size
    } else {
        input.len()
    };

    let mut out = vec![0u8; size];
    out[size - n..].copy_from_slice(input);
    out
}
