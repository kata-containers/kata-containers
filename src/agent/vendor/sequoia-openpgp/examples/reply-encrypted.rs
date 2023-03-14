//! Demonstrates how to reply to an encrypted message without having
//! everyone's certs.
//!
//! This example demonstrates how to fall back to the original
//! message's session key in order to encrypt a reply.
//!
//! Replying to an encrypted message usually requires the encryption
//! (sub)keys for every recipient.  If even one key is not available,
//! it is not possible to encrypt the new session key.  Rather than
//! falling back to replying unencrypted, one can reuse the original
//! message's session key that was encrypted for every recipient and
//! reuse the original PKESKs.
//!
//! Decrypts an asymmetrically-encrypted OpenPGP message using the
//! openpgp crate, Sequoia's low-level API, remembering the session
//! key and PKESK packets.  It then encrypts a new message reusing
//! both the session key and PKESK packets.
//!
//! # Examples
//!
//! First, we generate two keys.  Second, we encrypt a message for
//! both certs.  We then decrypt the original message using Alice's
//! key and this example program, composing an encrypted reply reusing
//! the session key and PKESK packets.  Finally, we decrypt the reply
//! using Bob's key.
//!
//! ```sh
//! $ sqop generate-key alice@example.org > alice.pgp
//! $ sqop generate-key bob@example.org > bob.pgp
//! $ echo Original message | sqop encrypt alice.pgp bob.pgp > original.pgp
//! $ echo Reply | cargo run -p sequoia-openpgp --example reply-encrypted -- \
//!                    original.pgp alice.pgp > reply.pgp
//! $ sqop decrypt --session-key-out original.sk bob.pgp < reply.pgp
//! Encrypted using AES with 256-bit key
//! - Original message:
//! Original message
//! - Reusing (AES with 256-bit key, 62F3EADC...) with 2 PKESK packets
//! Reply
//! $ cat original.sk
//! 9:62F3EADC98E1D3D34495E79264B5959391B4FABB2B2A2B7E03861F92D0B03161
//! ```

use std::collections::HashMap;
use std::env;
use std::io;

use anyhow::Context;

use sequoia_openpgp as openpgp;

use openpgp::{KeyID, Fingerprint};
use openpgp::cert::prelude::*;
use openpgp::packet::prelude::*;
use openpgp::crypto::{KeyPair, SessionKey};
use openpgp::types::SymmetricAlgorithm;
use openpgp::parse::{Parse, stream::*};
use openpgp::serialize::{Serialize, stream::*};
use openpgp::policy::Policy;
use openpgp::policy::StandardPolicy as P;

pub fn main() -> openpgp::Result<()> {
    let p = &P::new();

    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        return Err(anyhow::anyhow!("Reply-to-all without having all certs.\n\n\
                Usage: {} <encrypted-msg> <keyfile> [<keyfile>...] \
                <plaintext >ciphertext\n", args[0]));
    }

    let encrypted_message = &args[1];

    // Read the transferable secret keys from the given files.
    let certs =
        args[2..].iter().map(|f| {
            openpgp::Cert::from_file(f)
        }).collect::<openpgp::Result<Vec<_>>>()
        .context("Failed to read key")?;

    // Now, create a decryptor with a helper using the given Certs.
    let mut decryptor =
        DecryptorBuilder::from_file(encrypted_message)?
        .with_policy(p, None, Helper::new(p, certs))?;

    // Finally, stream the decrypted data to stderr.
    eprintln!("- Original message:");
    io::copy(&mut decryptor, &mut io::stderr())
        .context("Decryption failed")?;

    let (algo, sk, pkesks) = decryptor.into_helper().recycling_bin.unwrap();
    eprintln!("- Reusing ({}, {}) with {} PKESK packets",
              algo, openpgp::fmt::hex::encode(&sk), pkesks.len());

    // Compose a writer stack corresponding to the output format and
    // packet structure we want.
    let mut sink = io::stdout();

    // Stream an OpenPGP message.
    let message = Message::new(&mut sink);
    let mut message = Armorer::new(message).build()?;

    // Emit the stashed PKESK packets.
    for p in pkesks {
        openpgp::Packet::from(p).serialize(&mut message)?;
    }

    // We want to encrypt a literal data packet.
    let message = Encryptor::with_session_key(message, algo, sk)?
        .build().context("Failed to create encryptor")?;

    let mut message = LiteralWriter::new(message).build()
        .context("Failed to create literal writer")?;

    // Copy stdin to our writer stack to encrypt the data.
    io::copy(&mut io::stdin(), &mut message)
        .context("Failed to encrypt")?;

    // Finally, finalize the OpenPGP message by tearing down the
    // writer stack.
    message.finalize()?;


    Ok(())
}

/// This helper provides secrets for the decryption, fetches public
/// keys for the signature verification and implements the
/// verification policy.
struct Helper {
    keys: HashMap<KeyID, (Fingerprint, KeyPair)>,
    recycling_bin: Option<(SymmetricAlgorithm, SessionKey, Vec<PKESK>)>,
}

impl Helper {
    /// Creates a Helper for the given Certs with appropriate secrets.
    fn new(p: &dyn Policy, certs: Vec<openpgp::Cert>) -> Self {
        // Map (sub)KeyIDs to primary fingerprints and secrets.
        let mut keys = HashMap::new();
        for cert in certs {
            for ka in cert.keys().unencrypted_secret().with_policy(p, None)
                .supported()
                .for_storage_encryption().for_transport_encryption()
            {
                keys.insert(ka.key().keyid(),
                            (cert.fingerprint(),
                             ka.key().clone().into_keypair().unwrap()));
            }
        }

        Helper {
            keys,
            recycling_bin: None,
        }
    }
}

impl DecryptionHelper for Helper {
    fn decrypt<D>(&mut self,
                  pkesks: &[openpgp::packet::PKESK],
                  _skesks: &[openpgp::packet::SKESK],
                  sym_algo: Option<SymmetricAlgorithm>,
                  mut decrypt: D)
                  -> openpgp::Result<Option<openpgp::Fingerprint>>
        where D: FnMut(SymmetricAlgorithm, &SessionKey) -> bool
    {
        // Try each PKESK until we succeed.
        let mut recipient = None;
        let mut encryption_context = None;
        for pkesk in pkesks {
            if let Some((fp, pair)) = self.keys.get_mut(pkesk.recipient()) {
                if pkesk.decrypt(pair, sym_algo)
                    .map(|(algo, session_key)| {
                        let success = decrypt(algo, &session_key);
                        if success {
                            // Keep a copy the algorithm, session key,
                            // and all PKESK packets for the reply.
                            encryption_context =
                                Some((
                                    algo,
                                    session_key.clone(),
                                    pkesks.iter().cloned().collect(),
                                ));
                        }
                        success
                    })
                    .unwrap_or(false)
                {
                    recipient = Some(fp.clone());
                    break;
                }
            }
        }

        // Store for later use.
        self.recycling_bin = encryption_context;
        Ok(recipient)
    }
}

impl VerificationHelper for Helper {
    fn get_certs(&mut self, _ids: &[openpgp::KeyHandle])
                       -> openpgp::Result<Vec<openpgp::Cert>> {
        Ok(Vec::new()) // Feed the Certs to the verifier here.
    }
    fn check(&mut self, structure: MessageStructure)
             -> openpgp::Result<()> {
        for layer in structure.iter() {
            match layer {
                MessageLayer::Compression { algo } =>
                    eprintln!("Compressed using {}", algo),
                MessageLayer::Encryption { sym_algo, aead_algo } =>
                    if let Some(aead_algo) = aead_algo {
                        eprintln!("Encrypted and protected using {}/{}",
                                  sym_algo, aead_algo);
                    } else {
                        eprintln!("Encrypted using {}", sym_algo);
                    },
                MessageLayer::SignatureGroup { ref results } =>
                    for result in results {
                        match result {
                            Ok(GoodChecksum { ka, .. }) => {
                                eprintln!("Good signature from {}", ka.cert());
                            },
                            Err(e) =>
                                eprintln!("Error: {:?}", e),
                        }
                    }
            }
        }
        Ok(()) // Implement your verification policy here.
    }
}
