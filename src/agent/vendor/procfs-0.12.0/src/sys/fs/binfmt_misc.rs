use bitflags::bitflags;
use std::path::Path;

use crate::{read_value, ProcResult};

/// Returns true if the miscellaneous Binary Formats system is enabled.
pub fn enabled() -> ProcResult<bool> {
    let val: String = read_value("/proc/sys/fs/binfmt_misc/status")?;
    Ok(val == "enabled")
}

fn hex_to_vec(hex: &str) -> ProcResult<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return Err(build_internal_error!(format!(
            "Hex string {:?} has non-even length",
            hex
        )));
    }
    let mut idx = 0;
    let mut data = Vec::new();
    while idx < hex.len() {
        let byte = from_str!(u8, &hex[idx..idx + 2], 16);
        data.push(byte);
        idx += 2;
    }

    Ok(data)
}

#[derive(Debug, Clone)]
pub enum BinFmtData {
    /// A BinFmt entry based on a file extension (does not include the period)
    Extension(String),
    /// A BinFmt entry based on magic string matching
    Magic { offset: u8, magic: Vec<u8>, mask: Vec<u8> },
}

/// A registered binary format entry
///
/// For more info, see the kernel doc Documentation/admin-guide/binfmt-misc.rst
#[derive(Debug, Clone)]
pub struct BinFmtEntry {
    /// The name of the entry
    ///
    /// Corresponds to a file in /proc/sys/fs/binfmt_misc/
    pub name: String,
    /// Is the entry enabled or not
    pub enabled: bool,
    /// Full path to the interpreter to run this entry
    pub interpreter: String,
    ///
    pub flags: BinFmtFlags,
    pub data: BinFmtData,
}

impl BinFmtEntry {
    pub(crate) fn from_string(name: String, data: &str) -> ProcResult<Self> {
        let mut enabled = false;
        let mut interpreter = String::new();

        let mut ext = None;

        let mut offset = 0;
        let mut magic = Vec::new();
        let mut mask = Vec::new();
        let mut flags = BinFmtFlags::empty();

        for line in data.lines() {
            if line == "enabled" {
                enabled = true;
            } else if line.starts_with("interpreter ") {
                interpreter = line[12..].to_string();
            } else if line.starts_with("flags:") {
                flags = BinFmtFlags::from_str(&line[6..]);
            } else if line.starts_with("extension .") {
                ext = Some(line[11..].to_string());
            } else if line.starts_with("offset ") {
                offset = from_str!(u8, &line[7..]);
            } else if line.starts_with("magic ") {
                let hex = &line[6..];
                magic = hex_to_vec(dbg!(hex))?;
            } else if line.starts_with("mask ") {
                let hex = &line[5..];
                mask = hex_to_vec(hex)?;
            }
        }

        if !magic.is_empty() && mask.is_empty() {
            mask.resize(magic.len(), 0xff);
        }

        Ok(BinFmtEntry {
            name,
            enabled,
            interpreter,
            flags,
            data: if let Some(ext) = ext {
                BinFmtData::Extension(ext)
            } else {
                BinFmtData::Magic { magic, mask, offset }
            },
        })
    }
}

bitflags! {
    /// Various key flags
    pub struct BinFmtFlags: u8 {
            /// Preserve Argv[0]
            ///
            /// Legacy behavior of binfmt_misc is to overwrite the original argv[0] with the full path to the binary. When
            /// this flag is included, binfmt_misc will add an argument to the argument vector for this purpose, thus
            /// preserving the original `argv[0]`.
            ///
            /// For example, If your interp is set to `/bin/foo` and you run `blah` (which is in `/usr/local/bin`),
            /// then the kernel will execute `/bin/foo` with `argv[]` set to `["/bin/foo", "/usr/local/bin/blah", "blah"]`.
            ///
            /// The interp has to be aware of this so it can execute `/usr/local/bin/blah` with `argv[]` set to `["blah"]`.
            const P = 0x01;

            /// Open Binary
            ///
            /// Legacy behavior of binfmt_misc is to pass the full path
            /// of the binary to the interpreter as an argument. When this flag is
            /// included, binfmt_misc will open the file for reading and pass its
            /// descriptor as an argument, instead of the full path, thus allowing
            /// the interpreter to execute non-readable binaries. This feature
            /// should be used with care - the interpreter has to be trusted not to
            //// emit the contents of the non-readable binary.
            const O = 0x02;

            /// Credentials
            ///
            ///  Currently, the behavior of binfmt_misc is to calculate
            /// the credentials and security token of the new process according to
            /// the interpreter. When this flag is included, these attributes are
            /// calculated according to the binary. It also implies the `O` flag.
            /// This feature should be used with care as the interpreter
            /// will run with root permissions when a setuid binary owned by root
            /// is run with binfmt_misc.
            const C = 0x04;

            /// Fix binary
            ///
            /// The usual behaviour of binfmt_misc is to spawn the
            /// binary lazily when the misc format file is invoked.  However,
            /// this doesn't work very well in the face of mount namespaces and
            /// changeroots, so the `F` mode opens the binary as soon as the
            /// emulation is installed and uses the opened image to spawn the
            /// emulator, meaning it is always available once installed,
            /// regardless of how the environment changes.
            const F = 0x08;
    }
}

impl BinFmtFlags {
    pub(crate) fn from_str(s: &str) -> Self {
        s.chars()
            .filter_map(|c| match c {
                'P' => Some(BinFmtFlags::P),
                'O' => Some(BinFmtFlags::O),
                'C' => Some(BinFmtFlags::C),
                'F' => Some(BinFmtFlags::F),
                _ => None,
            })
            .fold(BinFmtFlags::empty(), |a, b| a | b)
    }
}

pub fn list() -> ProcResult<Vec<BinFmtEntry>> {
    let path = Path::new("/proc/sys/fs/binfmt_misc/");

    let mut v = Vec::new();

    for entry in wrap_io_error!(path, path.read_dir())? {
        let entry = entry?;
        if entry.file_name() == "status" || entry.file_name() == "register" {
            // these entries do not represent real entries
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();

        let data = std::fs::read_to_string(entry.path())?;

        v.push(BinFmtEntry::from_string(name, &data)?);
    }

    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enabled() {
        match enabled() {
            Ok(_) => {}
            Err(crate::ProcError::NotFound(_)) => {}
            Err(e) => panic!("{}", e),
        }
    }

    #[test]
    fn parse_magic() {
        let mask = "7f454c460201010000000000000000000200f300";

        let data = hex_to_vec(mask).unwrap();
        println!("{:?}", data);
        assert_eq!(data.len(), 20);
        assert_eq!(data[0], 0x7f);
        assert_eq!(data[1], 0x45);

        assert!(hex_to_vec("a").is_err());
        assert!(hex_to_vec("zz").is_err());
    }

    #[test]
    fn flags_parsing() {
        assert!(BinFmtFlags::from_str("").is_empty());

        let flags = BinFmtFlags::from_str("F");
        assert_eq!(flags, BinFmtFlags::F);

        let flags = BinFmtFlags::from_str("OCF");
        assert_eq!(flags, BinFmtFlags::F | BinFmtFlags::C | BinFmtFlags::O);
    }

    #[test]
    fn binfmt() {
        let data = r#"enabled
interpreter /usr/bin/qemu-riscv64-static
flags: OCF
offset 12
magic 7f454c460201010000000000000000000200f300
mask ffffffffffffff00fffffffffffffffffeffffff"#;

        let entry = BinFmtEntry::from_string("test".to_owned(), data).unwrap();
        println!("{:#?}", entry);
        assert_eq!(entry.flags, BinFmtFlags::F | BinFmtFlags::C | BinFmtFlags::O);
        assert!(entry.enabled);
        assert_eq!(entry.interpreter, "/usr/bin/qemu-riscv64-static");
        if let BinFmtData::Magic { offset, magic, mask } = entry.data {
            assert_eq!(offset, 12);
            assert_eq!(magic.len(), mask.len());
            assert_eq!(
                magic,
                vec![
                    0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x02, 0x00, 0xf3, 0x00
                ]
            );
        } else {
            panic!("Unexpected data");
        }

        let data = r#"enabled
interpreter /bin/hello
flags:
extension .hello"#;
        let entry = BinFmtEntry::from_string("test".to_owned(), data).unwrap();
        println!("{:#?}", entry);
        assert_eq!(entry.flags, BinFmtFlags::empty());
        assert!(entry.enabled);
        assert_eq!(entry.interpreter, "/bin/hello");
        if let BinFmtData::Extension(ext) = entry.data {
            assert_eq!(ext, "hello");
        } else {
            panic!("Unexpected data");
        }
    }

    #[test]
    fn live() {
        for entry in super::list().unwrap() {
            println!("{:?}", entry);
        }
    }
}
