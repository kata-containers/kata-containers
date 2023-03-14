use rand::Rng;

use digest::DynDigest;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

use crate::algorithms::mgf1_xor;
use crate::errors::{Error, Result};
use crate::key::{self, PrivateKey, PublicKey};

// 2**61 -1 (pow is not const yet)
// TODO: This is the maximum for SHA-1, unclear from the RFC what the values are for other hashing functions.
const MAX_LABEL_LEN: u64 = 2_305_843_009_213_693_951;

/// Encrypts the given message with RSA and the padding
/// scheme from PKCS#1 OAEP.  The message must be no longer than the
/// length of the public modulus minus (2+ 2*hash.size()).
#[inline]
pub fn encrypt<R: Rng, K: PublicKey>(
    rng: &mut R,
    pub_key: &K,
    msg: &[u8],
    digest: &mut dyn DynDigest,
    label: Option<String>,
) -> Result<Vec<u8>> {
    key::check_public(pub_key)?;

    let k = pub_key.size();

    let h_size = digest.output_size();

    if msg.len() + 2 * h_size + 2 > k {
        return Err(Error::MessageTooLong);
    }

    let label = label.unwrap_or_default();
    if label.len() as u64 > MAX_LABEL_LEN {
        return Err(Error::LabelTooLong);
    }

    let mut em = vec![0u8; k];

    let (_, payload) = em.split_at_mut(1);
    let (seed, db) = payload.split_at_mut(h_size);
    rng.fill(seed);

    // Data block DB =  pHash || PS || 01 || M
    let db_len = k - h_size - 1;

    digest.update(label.as_bytes());
    let p_hash = digest.finalize_reset();
    db[0..h_size].copy_from_slice(&*p_hash);
    db[db_len - msg.len() - 1] = 1;
    db[db_len - msg.len()..].copy_from_slice(msg);

    mgf1_xor(db, digest, seed);
    mgf1_xor(seed, digest, db);

    pub_key.raw_encryption_primitive(&em, pub_key.size())
}

/// Decrypts a plaintext using RSA and the padding scheme from pkcs1# OAEP
/// If an `rng` is passed, it uses RSA blinding to avoid timing side-channel attacks.
///
/// Note that whether this function returns an error or not discloses secret
/// information. If an attacker can cause this function to run repeatedly and
/// learn whether each instance returned an error then they can decrypt and
/// forge signatures as if they had the private key. See
/// `decrypt_session_key` for a way of solving this problem.
#[inline]
pub fn decrypt<R: Rng, SK: PrivateKey>(
    rng: Option<&mut R>,
    priv_key: &SK,
    ciphertext: &[u8],
    digest: &mut dyn DynDigest,
    label: Option<String>,
) -> Result<Vec<u8>> {
    key::check_public(priv_key)?;

    let res = decrypt_inner(rng, priv_key, ciphertext, digest, label)?;
    if res.is_none().into() {
        return Err(Error::Decryption);
    }

    let (out, index) = res.unwrap();

    Ok(out[index as usize..].to_vec())
}

/// Decrypts ciphertext using `priv_key` and blinds the operation if
/// `rng` is given. It returns one or zero in valid that indicates whether the
/// plaintext was correctly structured.
#[inline]
fn decrypt_inner<R: Rng, SK: PrivateKey>(
    rng: Option<&mut R>,
    priv_key: &SK,
    ciphertext: &[u8],
    digest: &mut dyn DynDigest,
    label: Option<String>,
) -> Result<CtOption<(Vec<u8>, u32)>> {
    let k = priv_key.size();
    if k < 11 {
        return Err(Error::Decryption);
    }

    let h_size = digest.output_size();

    if ciphertext.len() != k || k < h_size * 2 + 2 {
        return Err(Error::Decryption);
    }

    let mut em = priv_key.raw_decryption_primitive(rng, ciphertext, priv_key.size())?;

    let label = label.unwrap_or_default();
    if label.len() as u64 > MAX_LABEL_LEN {
        return Err(Error::LabelTooLong);
    }

    digest.update(label.as_bytes());

    let expected_p_hash = &*digest.finalize_reset();

    let first_byte_is_zero = em[0].ct_eq(&0u8);

    let (_, payload) = em.split_at_mut(1);
    let (seed, db) = payload.split_at_mut(h_size);

    mgf1_xor(seed, digest, db);
    mgf1_xor(db, digest, seed);

    let hash_are_equal = db[0..h_size].ct_eq(expected_p_hash);

    // The remainder of the plaintext must be zero or more 0x00, followed
    // by 0x01, followed by the message.
    //   looking_for_index: 1 if we are still looking for the 0x01
    //   index: the offset of the first 0x01 byte
    //   zero_before_one: 1 if we saw a non-zero byte before the 1
    let mut looking_for_index = Choice::from(1u8);
    let mut index = 0u32;
    let mut nonzero_before_one = Choice::from(0u8);

    for (i, el) in db.iter().skip(h_size).enumerate() {
        let equals0 = el.ct_eq(&0u8);
        let equals1 = el.ct_eq(&1u8);
        index.conditional_assign(&(i as u32), looking_for_index & equals1);
        looking_for_index &= !equals1;
        nonzero_before_one |= looking_for_index & !equals0;
    }

    let valid = first_byte_is_zero & hash_are_equal & !nonzero_before_one & !looking_for_index;

    Ok(CtOption::new((em, index + 2 + (h_size * 2) as u32), valid))
}
