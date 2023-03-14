/// Asymmetrically encrypts and pads OpenPGP messages using the
/// openpgp crate, Sequoia's low-level API.

use std::env;
use std::io;

use anyhow::Context;

use sequoia_openpgp as openpgp;

use crate::openpgp::types::KeyFlags;
use crate::openpgp::parse::Parse;
use crate::openpgp::serialize::stream::{
    Armorer, Message, LiteralWriter, Encryptor, padding::*,
};
use crate::openpgp::policy::StandardPolicy as P;

fn main() -> openpgp::Result<()> {
    let p = &P::new();
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        return Err(anyhow::anyhow!("A simple encryption filter.\n\n\
                Usage: {} [at-rest|for-transport] <keyfile> [<keyfile>...] \
                <input >output\n", args[0]));
    }

    let mode = match args[1].as_ref() {
        "at-rest" => KeyFlags::empty().set_storage_encryption(),
        "for-transport" => KeyFlags::empty().set_transport_encryption(),
        x => return Err(anyhow::anyhow!("invalid mode: {:?}, \
                     must be either 'at-rest' or 'for-transport'",
                    x)),
    };

    // Read the certificates from the given files.
    let certs: Vec<openpgp::Cert> = args[2..].iter().map(|f| {
        openpgp::Cert::from_file(f)
    }).collect::<openpgp::Result<Vec<_>>>().context("Failed to read key")?;

    // Build a list of recipient subkeys.
    let mut recipients = Vec::new();
    for cert in certs.iter() {
        // Make sure we add at least one subkey from every
        // certificate.
        let mut found_one = false;
        for key in cert.keys().with_policy(p, None)
            .supported().alive().revoked(false).key_flags(&mode)
        {
            recipients.push(key);
            found_one = true;
        }

        if ! found_one {
            return Err(anyhow::anyhow!("No suitable encryption subkey for {}",
                                       cert));
        }
    }

    // Compose a writer stack corresponding to the output format and
    // packet structure we want.
    let mut sink = io::stdout();

    // Stream an OpenPGP message.
    let message = Message::new(&mut sink);

    let message = Armorer::new(message).build()?;

    // We want to encrypt a literal data packet.
    let message = Encryptor::for_recipients(message, recipients)
        .build().context("Failed to create encryptor")?;

    let message = Padder::new(message)
        .build().context("Failed to create padder")?;

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
