use base64::{engine::general_purpose, Engine as _};
use clap::Parser;
use std::io::{self, Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::{env::set_current_dir, process::Command};

#[derive(Parser, Debug)]
struct Args {
    /// The source tarfs file.
    source: String,

    /// The directory on which to mount.
    directory: String,

    /// The filesystem type.
    #[arg(short)]
    r#type: Option<String>,

    /// The filesystem options.
    #[arg(short, long)]
    options: Vec<String>,
}

const LAYER: &str = "io.katacontainers.fs-opt.layer=";
const LAYER_SRC_PREFIX: &str = "io.katacontainers.fs-opt.layer-src-prefix=";

struct Layer {
    src: PathBuf,
    fs: String,
    opts: String,
}

fn parse_layers(args: &Args) -> io::Result<Vec<Layer>> {
    let mut layers = Vec::new();
    let mut prefix = Path::new("");

    for group in &args.options {
        for opt in group.split(',') {
            if let Some(p) = opt.strip_prefix(LAYER_SRC_PREFIX) {
                prefix = Path::new(p);
                continue;
            }

            let encoded = if let Some(e) = opt.strip_prefix(LAYER) {
                e
            } else {
                continue;
            };

            let decoded = general_purpose::STANDARD
                .decode(encoded)
                .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
            let info = std::str::from_utf8(&decoded)
                .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;

            let mut fields = info.split(',');
            let src = if let Some(p) = fields.next() {
                if !p.is_empty() && p.as_bytes()[0] != b'/' {
                    prefix.join(Path::new(p))
                } else {
                    Path::new(p).to_path_buf()
                }
            } else {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!("Missing path from {info}"),
                ));
            };

            let fs = if let Some(f) = fields.next() {
                f.into()
            } else {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!("Missing filesystem type from {info}"),
                ));
            };

            let fs_opts = fields
                .filter(|o| !o.starts_with("io.katacontainers."))
                .fold(String::new(), |a, b| {
                    if a.is_empty() {
                        b.into()
                    } else {
                        format!("{a},{b}")
                    }
                });
            layers.push(Layer {
                src,
                fs,
                opts: fs_opts,
            });
        }
    }

    Ok(layers)
}

struct Unmounter(Vec<String>, tempfile::TempDir);
impl Drop for Unmounter {
    fn drop(&mut self) {
        for n in &self.0 {
            let p = self.1.path().join(n);
            match Command::new("umount").arg(&p).status() {
                Err(e) => eprintln!("Unable to run umount command: {e}"),
                Ok(s) => {
                    if !s.success() {
                        eprintln!("Unable to unmount {:?}: {s}", p);
                    }
                }
            }
        }
    }
}

fn main() -> io::Result<()> {
    let args = &Args::parse();
    let layers = parse_layers(args)?;
    let mut unmounter = Unmounter(Vec::new(), tempfile::tempdir()?);

    // Mount all layers.
    //
    // We use the `mount` command instead of a syscall because we want leverage the additional work
    // that `mount` does, for example, using helper binaries to mount.
    for (i, layer) in layers.iter().enumerate() {
        let n = i.to_string();
        let p = unmounter.1.path().join(&n);
        std::fs::create_dir_all(&p)?;
        println!("Mounting {:?} to {:?}", layer.src, p);

        let status = Command::new("mount")
            .arg(&layer.src)
            .arg(&p)
            .arg("-t")
            .arg(&layer.fs)
            .arg("-o")
            .arg(&layer.opts)
            .status()?;
        if !status.success() {
            return Err(Error::new(
                ErrorKind::Other,
                format!("failed to mount {:?}: {status}", &layer.src),
            ));
        }

        unmounter.0.push(n);
    }

    // Mont the overlay if we have multiple layers, otherwise do a bind-mount.
    let mp = std::fs::canonicalize(&args.directory)?;
    if unmounter.0.len() == 1 {
        let p = unmounter.1.path().join(unmounter.0.first().unwrap());
        let status = Command::new("mount")
            .arg(&p)
            .arg(&mp)
            .args(&["-t", "bind", "-o", "bind"])
            .status()?;
        if !status.success() {
            return Err(Error::new(
                ErrorKind::Other,
                format!("failed to bind mount: {status}"),
            ));
        }
    } else {
        let saved = std::env::current_dir()?;
        set_current_dir(unmounter.1.path())?;

        let status = Command::new("mount")
            .arg("none")
            .arg(&mp)
            .args(&[
                "-t",
                "overlay",
                "-o",
                &format!("lowerdir={}", unmounter.0.join(":")),
            ])
            .status()?;
        if !status.success() {
            return Err(Error::new(
                ErrorKind::Other,
                format!("failed to mount overlay: {status}"),
            ));
        }

        set_current_dir(saved)?;
    }

    Ok(())
}
