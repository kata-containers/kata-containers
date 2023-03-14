//! Illustrates how to create keys and certificates for group
//! conversations.
//!
//! This example, when run, generates four artifacts:
//!
//! ```text
//! % cargo run -p sequoia-openpgp --example generate-group-key cabal@example.org
//! [...]
//! Writing certificate to cabal@example.org.cert.pgp
//! Writing full key to cabal@example.org.key.pgp
//! Writing key with detached primary to cabal@example.org.only_subkey.pgp
//! Writing revocation certificate to cabal@example.org.revocation.pgp
//! ```
//!
//! - If you want to receive unsolicited encrypted messages on the
//!   address (e.g. in case of a security contact or a public mailing
//!   list), you should publish this in a suitable way, e.g. using
//!   WKD.  Consider authenticating this certificate using OpenPGP-CA.
//!
//! - Distribute the certificate freely if you want to receive
//!   unsolicited encrypted messages.
//!
//! - Distribute the key with the detached primary among the group,
//!   for example by including it in an email encrypted to all
//!   members.
//!
//! - Make a backup of full key and revocation certificate.
//!
//! - Consider distributing the revocation certificate among a select,
//!   trusted group that can help when disaster strikes.  The security
//!   implication of handing out the revocation certificate is a
//!   denial-of-service vector.

use std::fs::File;
use std::time::Duration;

use sequoia_openpgp as openpgp;
use openpgp::armor;
use openpgp::cert::prelude::*;
use openpgp::types::KeyFlags;
use openpgp::serialize::Serialize;

fn main() -> openpgp::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let name = if let Some(n) = args.get(1).cloned() {
        n
    } else {
        return Err(anyhow::anyhow!(
            "Missing list address parameter.\n\n\
             Usage: {} <LIST-ADDRESS>",
            args.get(0).cloned().unwrap_or("generate-group-key".into())));
    };

    // Generate the key.
    let (cert, revocation) = CertBuilder::new()
        .set_validity_period(Duration::new(5 * 365 * 24 * 60 * 60, 0))
        .add_userid(format!("<{}>", name))
        .add_subkey(KeyFlags::empty()
                    .set_transport_encryption()
                    .set_group_key(),
                    None,
                    None)
        .generate()?;

    // First, emit the certificate.
    let n = format!("{}.cert.pgp", name);
    eprintln!("Writing certificate to {}", n);
    cert.armored().serialize(&mut File::create(n)?)?;

    // Second, emit the key.  This includes all secret key material.
    // Back this up.
    let n = format!("{}.key.pgp", name);
    eprintln!("Writing full key to {}", n);
    cert.as_tsk().armored().serialize(&mut File::create(n)?)?;

    // Third, emit they key, but only include the encryption subkey's
    // secret key material.
    let n = format!("{}.only_subkey.pgp", name);
    eprintln!("Writing key with detached primary to {}", n);
    cert.as_tsk()
        .set_filter(|k| k.fingerprint() != cert.fingerprint())
        .emit_secret_key_stubs(true) // Enable GnuPG-style.
        .armored()
        .serialize(&mut File::create(n)?)?;

    // Finally, emit a revocation certificate.  Back this up.
    let n = format!("{}.revocation.pgp", name);
    eprintln!("Writing revocation certificate to {}", n);

    // Be fancy and include comments in the revocation cert.
    let mut comments = cert.armor_headers();
    comments.insert(0, "Revocation certificate for the following key:".into());
    comments.insert(1, "".into());
    let mut w = armor::Writer::with_headers(
        File::create(n)?,
        armor::Kind::PublicKey,
        comments.iter().map(|c| ("Comment", c)))?;
    openpgp::Packet::from(revocation).serialize(&mut w)?;
    w.finalize()?;

    Ok(())
}
