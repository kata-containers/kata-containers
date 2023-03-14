//! A drop-in replacement for coreutils' sha1sum utility.
//!
//! The sums are computed using Marc Stevens' modified SHA1 that
//! detects collision attacks.  When checking, the input should be a
//! former output of this program.  The default mode is to print a
//! line with checksum, a space, a character indicating input mode
//! ('*' for binary, ' ' for text or where binary is insignificant),
//! and name for each FILE.
//!
//! If a collision is detected, '*coll*' is printed in front of the
//! file name.
//!
//! This program implements the same interface as coreutils' sha1sum,
//! modulo error messages printed to stderr, handling of non-UTF8
//! filenames, and bugs.

use std::fs;
use std::io::{self, BufRead};
use std::path::PathBuf;
use structopt::StructOpt;

use sha1collisiondetection::*;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "sha1cdsum",
    about = "Print or check SHA1 (160-bit) checksums with \
             collision detection.",
    after_help = "\
The last five options are useful only when verifying checksums.

The sums are computed using Marc Stevens' modified SHA1 that detects
collision attacks.  When checking, the input should be a former output
of this program.  The default mode is to print a line with checksum, a
space, a character indicating input mode ('*' for binary, ' ' for text
or where binary is insignificant), and name for each FILE.

If a collision is detected, '*coll*' is printed in front of the file
name.

Note: There is no difference between binary mode and text mode on GNU
systems.

This program implements the same interface as coreutils' sha1sum,
modulo error messages printed to stderr, handling of non-UTF8
filenames, and bugs.")
]
struct Opt {
    /// read in binary mode
    #[structopt(short, long, conflicts_with("text"))]
    binary: bool,

    /// read SHA1 sums from the FILEs and check them
    #[structopt(short, long, conflicts_with("tag"))]
    check: bool,

    /// create a BSD-style checksum
    #[structopt(long, conflicts_with("check"), conflicts_with("text"))]
    tag: bool,

    /// read in text mode
    #[structopt(short, long, conflicts_with("binary"), conflicts_with("tag"))]
    text: bool,

    /// end each output line with NUL, not newline, and disable file
    /// name escaping
    #[structopt(short, long, conflicts_with("check"))]
    zero: bool,

    /// don't fail or report status for missing files
    #[structopt(long, display_order = 1000)]
    ignore_missing: bool,

    /// don't print OK for each successfully verified file
    #[structopt(long, display_order = 1000)]
    quiet: bool,

    /// don't output anything, status code shows success
    #[structopt(long, display_order = 1000)]
    status: bool,

    /// exit non-zero for improperly formatted checksum lines
    #[structopt(long, display_order = 1000)]
    strict: bool,

    /// warn about improperly formatted checksum lines
    #[structopt(short, long, display_order = 1000)]
    warn: bool,

    /// Input file(s).  With no FILE, or when FILE is -, read standard
    /// input.
    files: Vec<PathBuf>,
}

fn main() -> io::Result<()> {
    let mut opt = Opt::from_args();

    if cfg!(windows) && opt.text {
        eprintln!("Opening files in text mode is not supported.");
        std::process::exit(1);
    }

    if opt.files.is_empty() {
        opt.files.push("-".into());
    }

    if opt.check {
        check(opt)
    } else {
        compute(opt)
    }
}

fn check(opt: Opt) -> io::Result<()> {
    let mut ok = true;

    for checkfile_name in opt.files.iter() {
        ok &= check_file(&opt, checkfile_name).unwrap_or(false);
    }

    if ! ok {
        std::process::exit(1);
    }
    Ok(())
}

fn check_file(opt: &Opt, checkfile_name: &PathBuf) -> io::Result<bool> {
    let checkfile_name_str =
        checkfile_name.as_os_str().to_string_lossy().to_string();

    // sha1sum opens and reads from stdin exactly once.
    let mut stdin_read = false;

    let mut improperly_formatted_lines = 0;
    let mut open_or_read_failures = 0;
    let mut mismatched_checksums = 0;
    let mut collisions = 0;
    let mut properly_formatted_lines = false;
    let mut matched_checksums = false;

    let source: Box<dyn io::Read> =
        if checkfile_name.as_os_str().to_str().map(|s| s == "-")
        .unwrap_or(false)
    {
        Box::new(io::stdin())
    } else {
        Box::new(fs::File::open(checkfile_name)?)
    };

    for (lineno, line) in io::BufReader::new(source).lines().enumerate() {
        let mut line = line?;
        let mut malformed = || {
            if opt.warn {
                eprintln!("{}: {}: improperly formatted SHA1 checksum line",
                          checkfile_name_str, lineno + 1);
            }
            if opt.strict {
                std::process::exit(1);
            }
            improperly_formatted_lines += 1;
        };

        if line.starts_with("#") {
            continue; // Ignore comment lines.
        }

        let escaped = line.starts_with("\\");
        if escaped {
            line = line[1..].to_string();
        }

        let l = line.len();
        let (name, expected_hex) =
            if line.starts_with("SHA1 (") {
                // BSD-style tag file:
                //
                // SHA1 (Cargo.lock) = ddbe9265f199766186bf8ebb33951e98ecfcb8cf
                (&line[6..l - 44], &line[l - 40..])
            } else if line.len() > 42 && &line[40..41] == " " {
                // Native format:
                //
                // ddbe9265f199766186bf8ebb33951e98ecfcb8cf  Cargo.lock
                //                                          ^ maybe '*'
                let binary_indicator = &line[41..42];
                if binary_indicator != " " && binary_indicator != "*" {
                    malformed();
                }
                (&line[42..], &line[..40])
            } else {
                malformed();
                continue;
            };

        if ! expected_hex.chars().all(|c| c.is_ascii_hexdigit()) {
            malformed();
        }
        properly_formatted_lines = true;

        let mut expected = Output::default();
        for (octet, hex) in expected.iter_mut().zip(
            expected_hex.as_bytes().chunks(2)
                .map(|chunk| std::str::from_utf8(chunk).unwrap()))
        {
            *octet = match u8::from_str_radix(hex, 16) {
                Ok(v) => v,
                Err(_) => {
                    malformed();
                    continue;
                },
            };
        }

        let f = if escaped {
            unescape_filename(&name)
        } else {
            name.to_string()
        };

        let mut ctx = Sha1CD::default();
        if f == "-" {
            if stdin_read {
                // Don't read again.
            } else {
                io::copy(&mut io::stdin(), &mut ctx)?;
                stdin_read = true;
            }
        } else {
            let mut file = match fs::File::open(&f) {
                Ok(f) => f,
                Err(e) => {
                    if e.kind() == io::ErrorKind::NotFound
                        && opt.ignore_missing
                    {
                        continue;
                    }
                    if ! opt.status {
                        eprintln!("sha1sum: {:?}: {}", f, e);
                        println!("{}: FAILED open or read", name);
                    }
                    open_or_read_failures += 1;
                    continue;
                },
            };
            io::copy(&mut file, &mut ctx)?;
        };
        let mut digest = Output::default();
        if let Err(_) = ctx.finalize_into_dirty_cd(&mut digest) {
            collisions += 1;
        }

        if f.contains("\n") {
            print!("\\{}", f.replace("\n", "\\n"));
        } else {
            print!("{}", f);
        }

        if digest == expected {
            matched_checksums = true;
            println!(": OK");
        } else {
            mismatched_checksums += 1;
            println!(": FAILED");
        }
    }

    if ! properly_formatted_lines {
        eprintln!("{}: no properly formatted SHA1 checksum lines found",
                  checkfile_name_str);
    } else {
        if ! opt.status {
            if improperly_formatted_lines > 0 {
                eprintln!("WARNING: {} line{} are improperly formatted",
                          improperly_formatted_lines,
                          if improperly_formatted_lines > 1 { "s" } else { "" },
                );
            }
            if open_or_read_failures > 0 {
                eprintln!("WARNING: {} listed file{} could not be read",
                          open_or_read_failures,
                          if open_or_read_failures > 1 { "s" } else { "" },
                );
            }
            if mismatched_checksums > 0 {
                eprintln!("WARNING: {} computed checksum{} did NOT match",
                          mismatched_checksums,
                          if mismatched_checksums > 1 { "s" } else { "" },
                );
            }
            if collisions > 0 {
                eprintln!("WARNING: {} collision{} were detected",
                          collisions,
                          if collisions > 1 { "s" } else { "" },
                );
            }

            if opt.ignore_missing && ! matched_checksums {
                eprintln!("{}: no file was verified", checkfile_name_str);
            }
        }
    }

    Ok(properly_formatted_lines
       && matched_checksums
       && mismatched_checksums == 0
       && open_or_read_failures == 0
       && (! opt.strict || improperly_formatted_lines == 0))
}

fn compute(opt: Opt) -> io::Result<()> {
    // sha1sum opens and reads from stdin exactly once.
    let mut stdin_read = false;

    for f in opt.files.iter() {
        let mut ctx = Sha1CD::default();
        if f.as_os_str().to_str().map(|s| s == "-").unwrap_or(false) {
            if stdin_read {
                // Don't read again.
            } else {
                io::copy(&mut io::stdin(), &mut ctx)?;
                stdin_read = true;
            }
        } else {
            io::copy(&mut fs::File::open(f)?, &mut ctx)?;
        };
        let mut digest = Output::default();
        let r = ctx.finalize_into_dirty_cd(&mut digest);

        let mut name = f.as_os_str().to_string_lossy().to_string();
        if ! opt.zero {
            if needs_escape(&name) {
                print!("\\");
            }
            name = escape_filename(name);
        }

        if opt.tag {
            // BSD-style tags.
            print!("SHA1 ({}) = ", name);

            if r.is_err() {
                print!("*coll* ");
            }

            for b in digest {
                print!("{:02x}", b);
            }
        } else {
            for b in digest {
                print!("{:02x}", b);
            }

            if r.is_ok() {
                print!(" ");
                if opt.binary {
                    print!("*");
                } else {
                    print!(" ");
                }
            } else {
                print!(" *coll* ");
            }
            print!("{}", name);
        }

        if opt.zero {
            print!("\x00");
        } else {
            println!();
        }
    }
    Ok(())
}

fn needs_escape<N: AsRef<str>>(n: N) -> bool {
    n.as_ref().contains("\n")
        || n.as_ref().contains("\\")
}

fn escape_filename<N: AsRef<str>>(n: N) -> String {
    n.as_ref().replace("\\", "\\\\").replace("\n", "\\n")
}

fn unescape_filename<N: AsRef<str>>(n: N) -> String {
    n.as_ref().replace("\\n", "\n").replace("\\\\", "\\")
}
