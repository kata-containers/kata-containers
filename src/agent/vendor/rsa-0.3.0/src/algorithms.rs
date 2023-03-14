use digest::DynDigest;
use num_bigint::traits::ModInverse;
use num_bigint::{BigUint, RandPrime};
use num_traits::{FromPrimitive, One, Zero};
use rand::Rng;

use crate::errors::{Error, Result};
use crate::key::RSAPrivateKey;

/// Default exponent for RSA keys.
const EXP: u64 = 65537;

// Generates a multi-prime RSA keypair of the given bit
// size and the given random source, as suggested in [1]. Although the public
// keys are compatible (actually, indistinguishable) from the 2-prime case,
// the private keys are not. Thus it may not be possible to export multi-prime
// private keys in certain formats or to subsequently import them into other
// code.
//
// Table 1 in [2] suggests maximum numbers of primes for a given size.
//
// [1] US patent 4405829 (1972, expired)
// [2] http://www.cacr.math.uwaterloo.ca/techreports/2006/cacr2006-16.pdf
pub fn generate_multi_prime_key<R: Rng>(
    rng: &mut R,
    nprimes: usize,
    bit_size: usize,
) -> Result<RSAPrivateKey> {
    if nprimes < 2 {
        return Err(Error::NprimesTooSmall);
    }

    if bit_size < 64 {
        let prime_limit = (1u64 << (bit_size / nprimes) as u64) as f64;

        // pi aproximates the number of primes less than prime_limit
        let mut pi = prime_limit / (prime_limit.ln() - 1f64);
        // Generated primes start with 0b11, so we can only use a quarter of them.
        pi /= 4f64;
        // Use a factor of two to ensure taht key generation terminates in a
        // reasonable amount of time.
        pi /= 2f64;

        if pi < nprimes as f64 {
            return Err(Error::TooFewPrimes);
        }
    }

    let mut primes = vec![BigUint::zero(); nprimes];
    let n_final: BigUint;
    let d_final: BigUint;

    'next: loop {
        let mut todo = bit_size;
        // `gen_prime` should set the top two bits in each prime.
        // Thus each prime has the form
        //   p_i = 2^bitlen(p_i) × 0.11... (in base 2).
        // And the product is:
        //   P = 2^todo × α
        // where α is the product of nprimes numbers of the form 0.11...
        //
        // If α < 1/2 (which can happen for nprimes > 2), we need to
        // shift todo to compensate for lost bits: the mean value of 0.11...
        // is 7/8, so todo + shift - nprimes * log2(7/8) ~= bits - 1/2
        // will give good results.
        if nprimes >= 7 {
            todo += (nprimes - 2) / 5;
        }

        for (i, prime) in primes.iter_mut().enumerate() {
            *prime = rng.gen_prime(todo / (nprimes - i));
            todo -= prime.bits();
        }

        // Makes sure that primes is pairwise unequal.
        for (i, prime1) in primes.iter().enumerate() {
            for prime2 in primes.iter().take(i) {
                if prime1 == prime2 {
                    continue 'next;
                }
            }
        }

        let mut n = BigUint::one();
        let mut totient = BigUint::one();

        for prime in &primes {
            n *= prime;
            totient *= prime - BigUint::one();
        }

        if n.bits() != bit_size {
            // This should never happen for nprimes == 2 because
            // gen_prime should set the top two bits in each prime.
            // For nprimes > 2 we hope it does not happen often.
            continue 'next;
        }

        let exp = BigUint::from_u64(EXP).expect("invalid static exponent");
        if let Some(d) = exp.mod_inverse(totient) {
            n_final = n;
            d_final = d.to_biguint().unwrap();
            break;
        }
    }

    Ok(RSAPrivateKey::from_components(
        n_final,
        BigUint::from_u64(EXP).expect("invalid static exponent"),
        d_final,
        primes,
    ))
}

/// Mask generation function.
///
/// Panics if out is larger than 2**32. This is in accordance with RFC 8017 - PKCS #1 B.2.1
pub fn mgf1_xor(out: &mut [u8], digest: &mut dyn DynDigest, seed: &[u8]) {
    let mut counter = [0u8; 4];
    let mut i = 0;

    const MAX_LEN: u64 = std::u32::MAX as u64 + 1;
    assert!(out.len() as u64 <= MAX_LEN);

    while i < out.len() {
        let mut digest_input = vec![0u8; seed.len() + 4];
        digest_input[0..seed.len()].copy_from_slice(seed);
        digest_input[seed.len()..].copy_from_slice(&counter);

        digest.update(digest_input.as_slice());
        let digest_output = &*digest.finalize_reset();
        let mut j = 0;
        loop {
            if j >= digest_output.len() || i >= out.len() {
                break;
            }

            out[i] ^= digest_output[j];
            j += 1;
            i += 1;
        }
        inc_counter(&mut counter);
    }
}

fn inc_counter(counter: &mut [u8; 4]) {
    for i in (0..4).rev() {
        counter[i] = counter[i].wrapping_add(1);
        if counter[i] != 0 {
            // No overflow
            return;
        }
    }
}
