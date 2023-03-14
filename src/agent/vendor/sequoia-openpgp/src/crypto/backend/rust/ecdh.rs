//! Elliptic Curve Diffie-Hellman.

use std::convert::TryInto;

use rand07::rngs::OsRng;

use crate::{Error, Result};
use crate::crypto::SessionKey;
use crate::crypto::mem::Protected;
use crate::crypto::ecdh::{encrypt_wrap, decrypt_unwrap};
use crate::crypto::mpi::{self, Ciphertext, SecretKeyMaterial, MPI};
use crate::packet::{key, Key};
use crate::types::Curve;

const CURVE25519_SIZE: usize = 32;

/// Wraps a session key using Elliptic Curve Diffie-Hellman.
#[allow(non_snake_case)]
pub fn encrypt<R>(recipient: &Key<key::PublicParts, R>,
                  session_key: &SessionKey)
    -> Result<Ciphertext>
    where R: key::KeyRole
{
    let (curve, q) = match recipient.mpis() {
        mpi::PublicKey::ECDH { curve, q, .. } => (curve, q),
        _ => return Err(Error::InvalidArgument("Expected an ECDHPublicKey".into()).into()),
    };

    let (VB, shared) = match curve {
        Curve::Cv25519 => {
            use x25519_dalek::{EphemeralSecret, PublicKey};

            // Decode the recipient's public key.
            let R: [u8; CURVE25519_SIZE] = q.decode_point(curve)?.0.try_into()?;
            let recipient_key = PublicKey::from(R);

            // Generate a keypair and perform Diffie-Hellman.
            let secret = EphemeralSecret::new(OsRng);
            let public = PublicKey::from(&secret);
            let shared = secret.diffie_hellman(&recipient_key);

            // Encode our public key. We need to add an encoding
            // octet in front of the key.
            let mut VB = [0; 1 + CURVE25519_SIZE];
            VB[0] = 0x40;
            VB[1..].copy_from_slice(public.as_bytes());
            let VB = MPI::new(&VB);

            // Encode the shared secret.
            let shared: &[u8] = shared.as_bytes();
            let shared = Protected::from(shared);

            (VB, shared)
        },
        Curve::NistP256 => {
            use p256::{EncodedPoint, PublicKey, ecdh::EphemeralSecret};

            // Decode the recipient's public key.
            let recipient_key = PublicKey::from_sec1_bytes(q.value())?;

            // Generate a keypair and perform Diffie-Hellman.
            let secret = EphemeralSecret::random(
                &mut p256::elliptic_curve::rand_core::OsRng);
            let public = EncodedPoint::from(PublicKey::from(&secret));
            let shared = secret.diffie_hellman(&recipient_key);

            // Encode our public key.
            let VB = MPI::new(public.as_bytes());

            // Encode the shared secret.
            let shared: &[u8] = shared.as_bytes();
            let shared = Protected::from(shared);

            (VB, shared)
        },
        _ =>
            return Err(Error::UnsupportedEllipticCurve(curve.clone()).into()),
    };

    encrypt_wrap(recipient, session_key, VB, &shared)
}

/// Unwraps a session key using Elliptic Curve Diffie-Hellman.
#[allow(non_snake_case)]
pub fn decrypt<R>(recipient: &Key<key::PublicParts, R>,
                  recipient_sec: &SecretKeyMaterial,
                  ciphertext: &Ciphertext)
    -> Result<SessionKey>
    where R: key::KeyRole
{
    let (curve, scalar, e) = match (recipient.mpis(), recipient_sec, ciphertext) {
        (mpi::PublicKey::ECDH { ref curve, ..},
        SecretKeyMaterial::ECDH { ref scalar, },
        Ciphertext::ECDH { ref e, .. }) => (curve, scalar, e),
         _ => return Err(Error::InvalidArgument("Expected an ECDHPublicKey".into()).into()),
    };

    let S: Protected = match curve {
        Curve::Cv25519 => {
            use x25519_dalek::{PublicKey, StaticSecret};

            // Get the public part V of the ephemeral key.
            let V: [u8; CURVE25519_SIZE] = e.decode_point(curve)?.0.try_into()?;
            let V = PublicKey::from(V);

            let mut scalar: [u8; CURVE25519_SIZE] =
                scalar.value_padded(CURVE25519_SIZE).as_ref().try_into()?;
            scalar.reverse();
            let r = StaticSecret::from(scalar);

            let secret = r.diffie_hellman(&V);
            Vec::from(secret.to_bytes()).into()
        },
        Curve::NistP256 => {
            use p256::{
                SecretKey,
                PublicKey,
                elliptic_curve::ecdh::diffie_hellman,
            };

            const NISTP256_SIZE: usize = 32;

            // Get the public part V of the ephemeral key.
            let V = dbg!(PublicKey::from_sec1_bytes(e.value()))?;

            let scalar: [u8; NISTP256_SIZE] =
                scalar.value_padded(NISTP256_SIZE).as_ref().try_into()?;
            let r = SecretKey::from_bytes(scalar)?;

            let secret = diffie_hellman(r.secret_scalar(), V.as_affine());
            Vec::from(secret.as_bytes().as_slice()).into()
        },
        _ => {
            return Err(Error::UnsupportedEllipticCurve(curve.clone()).into());
        },
    };

    decrypt_unwrap(recipient, &S, ciphertext)
}
