/// Extracts the Web-Of-Trust, i.e. the certification relation, from
/// SKS packet dump using the openpgp crate, Sequoia's low-level API.
///
/// Note that to achieve reasonable performance, you need to compile
/// Sequoia and this program with optimizations:
///
///     % cargo run -p sequoia-openpgp --example web-of-trust --release \
///           -- <packet-dump> [<packet-dump> ...]

use std::env;

use anyhow::Context;

use sequoia_openpgp as openpgp;

use crate::openpgp::KeyID;
use crate::openpgp::cert::prelude::*;
use crate::openpgp::parse::Parse;

fn main() -> openpgp::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        return Err(anyhow::anyhow!("Extracts the certification relation from OpenPGP packet dumps.\
                \n\nUsage: {} <packet-dump> [<packet-dump> ...]\n", args[0]));
    }

    // The issuer refers to a (sub)key, but we want to use the primary
    // keys as identifiers.  But, because there are no tools besides
    // Sequoia that support certification-capable subkeys, we will
    // assume for now that the issuer is always a primary key.

    eprintln!("Format: certifier, user-id, key");

    // For each input file, create a parser.
    for input in &args[1..] {
        eprintln!("Parsing {}...", input);
        let parser = CertParser::from_file(input)
            .context("Failed to create reader")?;

        for cert in parser {
            match cert {
                Ok(cert) => {
                    let keyid = cert.keyid();
                    for uidb in cert.userids() {
                        for tps in uidb.certifications() {
                            for issuer in tps.get_issuers() {
                                println!("{}, {:?}, {}",
                                         KeyID::from(issuer).as_u64()?,
                                         String::from_utf8_lossy(
                                             uidb.userid().value()),
                                         keyid.as_u64()?);
                            }
                        }
                    }
                },
                Err(e) =>
                    eprintln!("Parsing Cert failed: {}", e),
            }
        }
    }

    Ok(())
}
