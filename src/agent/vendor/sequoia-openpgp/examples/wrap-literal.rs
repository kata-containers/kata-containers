/// Wraps a stream of data into a literal data packet using the
/// openpgp crate, Sequoia's low-level API.
///
/// It is also used to generate test vectors for the armor subsystem.

use std::env;
use std::io;

use anyhow::Context;

use sequoia_openpgp as openpgp;

use crate::openpgp::serialize::stream::{Armorer, Message, LiteralWriter};

fn main() -> openpgp::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 1 {
        return Err(anyhow::anyhow!("A simple filter wrapping data into a literal data packet.\n\n\
                Usage: {} <input >output\n", args[0]));
    }

    // Compose a writer stack corresponding to the output format and
    // packet structure we want.
    let mut sink = io::stdout();

    // Stream an OpenPGP message.
    let message = Message::new(&mut sink);

    let message = Armorer::new(message).build()?;

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
