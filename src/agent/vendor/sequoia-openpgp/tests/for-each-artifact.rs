use std::env;
use std::fmt::Write;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::anyhow;

use sequoia_openpgp as openpgp;
use crate::openpgp::parse::*;
use crate::openpgp::PacketPile;
use crate::openpgp::Result;
use crate::openpgp::serialize::{Serialize, SerializeInto};

mod for_each_artifact {
    use super::*;

    // Pretty print buf as a hexadecimal string.
    fn hex(buf: &[u8]) -> Result<String> {
        let mut s = String::new();
        for (i, b) in buf.iter().enumerate() {
            if i % 32 == 0 {
                if i > 0 {
                    write!(s, "\n")?;
                }
                write!(s, "{:04}:", i)?;
            }
            if i % 2 == 0 {
                write!(s, " ")?;
            }
            if i % 8 == 0 {
                write!(s, " ")?;
            }
            write!(s, "{:02X}", b)?;
        }
        writeln!(s, "")?;
        Ok(s)
    }

    // Find the first difference between two serialized messages.
    fn diff_serialized(a: &[u8], b: &[u8]) -> Result<()> {
        if a == b {
            return Ok(())
        }

        // There's a difference.  Find it.
        let p = PacketPile::from_bytes(a)?.into_children();
        let p_len = p.len();

        let q = PacketPile::from_bytes(b)?.into_children();
        let q_len = q.len();

        let mut offset = 0;
        for (i, (p, q)) in p.zip(q).enumerate() {
            let a = &a[offset..offset+p.serialized_len()];
            let b = &b[offset..offset+q.serialized_len()];

            if a == b {
                offset += p.serialized_len();
                continue;
            }

            eprintln!("Difference detected at packet #{}, offset: {}",
                      i, offset);

            eprintln!(" left packet: {:?}", p);
            eprintln!("right packet: {:?}", q);
            eprintln!(" left hex ({} bytes):\n{}", a.len(), hex(a)?);
            eprintln!("right hex ({} bytes):\n{}", b.len(), hex(b)?);

            return Err(anyhow!("Packets #{} differ at offset {}",
                               i, offset));
        }

        assert!(p_len != q_len);
        eprintln!("Differing number of packets: {} bytes vs. {} bytes",
                  p_len, q_len);

        return Err(
            anyhow!("Differing number of packets: {} bytes vs. {} bytes",
                    p_len, q_len));
    }

    #[test]
    fn packet_roundtrip() {
        for_all_files(&test_data_dir(), |src| {
            for_all_packets(src, |p| {
                let mut v = Vec::new();
                p.serialize(&mut v)?;
                let q = openpgp::Packet::from_bytes(&v)?;
                if p != &q {
                    return Err(anyhow::anyhow!(
                        "assertion failed: p == q\np = {:?}\nq = {:?}", p, q));
                }
                let w = p.to_vec()?;
                if v != w {
                    return Err(anyhow::anyhow!(
                        "assertion failed: v == w\nv = {:?}\nw = {:?}", v, w));
                }
                Ok(())
            })
        }).unwrap();
    }

    #[test]
    fn cert_roundtrip() {
        for_all_files(&test_data_dir(), |src| {
            let p = if let Ok(cert) = openpgp::Cert::from_file(src) {
                cert
            } else {
                // Ignore non-Cert files.
                return Ok(());
            };

            let mut v = Vec::new();
            p.as_tsk().serialize(&mut v)?;
            let q = openpgp::Cert::from_bytes(&v)?;
            if p != q {
                eprintln!("roundtripping {:?} failed", src);

                let p_: Vec<_> = p.clone().into_packets().collect();
                let q_: Vec<_> = q.clone().into_packets().collect();
                eprintln!("original: {} packets; roundtripped: {} packets",
                          p_.len(), q_.len());

                for (i, (p, q)) in p_.iter().zip(q_.iter()).enumerate() {
                    if p != q {
                        eprintln!("First difference at packet {}:\nOriginal: {:?}\nNew: {:?}",
                                  i, p, q);
                        break;
                    }
                }

                eprintln!("This is the recovered cert:\n{}",
                          String::from_utf8_lossy(
                              &q.armored().to_vec().unwrap()));
            }
            assert_eq!(p, q, "roundtripping {:?} failed", src);

            let w = p.as_tsk().to_vec().unwrap();
            assert_eq!(v, w,
                       "Serialize and SerializeInto disagree on {:?}", p);

            // Check that
            // Cert::strip_secret_key_material().into_packets() and
            // Cert::to_vec() agree.  (Cert::into_packets() returns
            // secret keys if secret key material is present;
            // Cert::to_vec only ever returns public keys.)
            let v = p.to_vec()?;
            let mut buf = Vec::new();
            for p in p.clone().strip_secret_key_material().into_packets() {
                p.serialize(&mut buf)?;
            }
            if let Err(_err) = diff_serialized(&buf, &v) {
                panic!("Checking that \
                        Cert::strip_secret_key_material().into_packets() \
                        and Cert::to_vec() agree.");
            }

            // Check that Cert::into_packets() and
            // Cert::as_tsk().to_vec() agree.
            let v = p.as_tsk().to_vec()?;
            let mut buf = Vec::new();
            for p in p.into_packets() {
                p.serialize(&mut buf)?;
            }
            if let Err(_err) = diff_serialized(&buf, &v) {
                panic!("Checking that Cert::into_packets() \
                        and Cert::as_tsk().to_vec() agree.");
            }

            Ok(())
        }).unwrap();
    }

    #[test]
    fn message_roundtrip() {
        for_all_files(&test_data_dir(), |src| {
            let p = if let Ok(msg) = openpgp::Message::from_file(src) {
                msg
            } else {
                // Ignore non-Message files.
                return Ok(());
            };

            let mut v = Vec::new();
            p.serialize(&mut v)?;
            let q = openpgp::Message::from_bytes(&v)?;
            assert_eq!(p, q, "roundtripping {:?} failed", src);

            let w = p.to_vec().unwrap();
            assert_eq!(v, w,
                       "Serialize and SerializeInto disagree on {:?}", p);
            Ok(())
        }).unwrap();
    }
}

/// Computes the path to the test directory.
fn test_data_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
        .as_ref()
        .expect("CARGO_MANIFEST_DIR not set"));

    manifest_dir.join("tests").join("data")
}

/// Maps the given function `fun` over all Rust files in `src`.
fn for_all_files<F>(src: &Path, mut fun: F) -> openpgp::Result<()>
    where F: FnMut(&Path) -> openpgp::Result<()>
{
    let mut dirs = vec![src.to_path_buf()];

    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                // XXX: Look at the file extension and skip non-PGP
                // files.  We need to do this because the
                // Armor-heuristic is so slow.  See #204.
                if let Some(extension) =
                    path.extension().and_then(|e| e.to_str())
                {
                    match extension {
                        "pgp" | "gpg" | "asc" | "key" => (),
                        e if e.contains("key") => (),
                        _ => continue,
                    }
                } else {
                    // No extension or not valid UTF-8.
                    continue;
                }

                eprintln!("Processing {:?}", path);
                match fun(&path) {
                    Ok(_) => (),
                    Err(e) => {
                        eprintln!("Failed on file {:?}:\n", path);
                        return Err(e);
                    },
                }
            }
            if path.is_dir() {
                dirs.push(path.clone());
            }
        }
    }
    Ok(())
}

/// Maps the given function `fun` over all packets in `src`.
fn for_all_packets<F>(src: &Path, mut fun: F) -> openpgp::Result<()>
    where F: FnMut(&openpgp::Packet) -> openpgp::Result<()>
{
    let ppb = PacketParserBuilder::from_file(src)?.buffer_unread_content();
    let mut ppr = if let Ok(ppr) = ppb.build() {
        ppr
    } else {
        // Ignore junk.
        return Ok(());
    };

    while let PacketParserResult::Some(pp) = ppr {
        match pp.recurse() {
            Ok((packet, ppr_)) => {
                ppr = ppr_;

                if let openpgp::Packet::Unknown(_) = packet {
                    continue;  // Ignore packets that we cannot parse.
                }

                match fun(&packet) {
                    Ok(_) => (),
                    Err(e) => {
                        eprintln!("Failed on packet {:?}:\n", packet);
                        let mut sink = io::stderr();
                        let mut w = openpgp::armor::Writer::new(
                            &mut sink,
                            openpgp::armor::Kind::File)?;
                        packet.serialize(&mut w)?;
                        w.finalize()?;
                        return Err(e);
                    },
                }
            },
            Err(_) => break,
        }
    }
    Ok(())
}
