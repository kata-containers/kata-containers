/// Signs data using the openpgp crate, Sequoia's low-level API.

use std::env;
use std::io;

use anyhow::Context;

use sequoia_openpgp as openpgp;

use crate::openpgp::parse::Parse;
use crate::openpgp::serialize::stream::{Armorer, Message, LiteralWriter, Signer};
use crate::openpgp::policy::StandardPolicy as P;

fn main() -> openpgp::Result<()> {
    let p = &P::new();
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        return Err(anyhow::anyhow!("A simple signing filter.\n\n\
                Usage: {} <secret-keyfile> [<secret-keyfile>...] \
                <input >output\n", args[0]));
    }

    // Read the transferable secret keys from the given files.
    let mut keys = Vec::new();
    for filename in &args[1..] {
        let tsk = openpgp::Cert::from_file(filename)
            .context("Failed to read key")?;
        let mut n = 0;

        for key in tsk.keys()
            .with_policy(p, None).alive().revoked(false).for_signing().secret()
            .map(|ka| ka.key())
        {
            keys.push({
                let mut key = key.clone();
                if key.secret().is_encrypted() {
                    let password = rpassword::prompt_password(format!(
                        "Please enter password to decrypt {}/{}: ",
                        tsk, key
                    ))?;
                    let algo = key.pk_algo();
                    key.secret_mut()
                        .decrypt_in_place(algo, &password.into())
                        .context("decryption failed")?;
                }
                n += 1;
                key.into_keypair()?
            });
        }

        if n == 0 {
            return Err(anyhow::anyhow!("Found no suitable signing key on {}", tsk));
        }
    }

    // Compose a writer stack corresponding to the output format and
    // packet structure we want.
    let mut sink = io::stdout();

    // Stream an OpenPGP message.
    let message = Message::new(&mut sink);

    let message = Armorer::new(message).build()?;

    // Now, create a signer that emits the signature(s).
    let mut signer =
        Signer::new(message, keys.pop().context("No key for signing")?);
    for s in keys {
        signer = signer.add_signer(s);
    }
    let message = signer.build().context("Failed to create signer")?;

    // Then, create a literal writer to wrap the data in a literal
    // message packet.
    let mut message = LiteralWriter::new(message).build()
        .context("Failed to create literal writer")?;

    // Copy all the data.
    io::copy(&mut io::stdin(), &mut message)
        .context("Failed to sign data")?;

    // Finally, teardown the stack to ensure all the data is written.
    message.finalize()
        .context("Failed to write data")?;

    Ok(())
}
