use clap::Parser;
use fuser::MountOption;
use log::debug;
use std::io::{self, Error, ErrorKind};
use zerocopy::byteorder::{LE, U32, U64};
use zerocopy::FromBytes;

mod fs;

// TODO: Remove this and import from dm-verity crate.
#[derive(Default, zerocopy::AsBytes, zerocopy::FromBytes, zerocopy::Unaligned)]
#[repr(C)]
pub struct VeritySuperBlock {
    pub data_block_size: U32<LE>,
    pub hash_block_size: U32<LE>,
    pub data_block_count: U64<LE>,
}

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

fn main() -> io::Result<()> {
    env_logger::init();
    let args = Args::parse();
    let mountpoint = std::fs::canonicalize(&args.directory)?;
    let file = std::fs::File::open(&args.source)?;

    // Check that the filesystem is tar.
    if let Some(t) = &args.r#type {
        if t != "tar" {
            debug!("Bad file system: {t}");
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "File system (-t) must be \"tar\"",
            ));
        }
    }

    // Parse all options.
    let mut options = Vec::new();
    for opts in &args.options {
        for opt in opts.split(',') {
            debug!("Parsing option {opt}");
            let fsopt = match opt {
                "dev" => MountOption::Dev,
                "nodev" => MountOption::NoDev,
                "suid" => MountOption::Suid,
                "nosuid" => MountOption::NoSuid,
                "ro" => MountOption::RO,
                "exec" => MountOption::Exec,
                "noexec" => MountOption::NoExec,
                "atime" => MountOption::Atime,
                "noatime" => MountOption::NoAtime,
                "dirsync" => MountOption::DirSync,
                "sync" => MountOption::Sync,
                "async" => MountOption::Async,
                "rw" => {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
                        "Tar file system are always read-only",
                    ));
                }
                _ => {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
                        format!("Unknown option ({opt})"),
                    ));
                }
            };
            options.push(fsopt);
        }
    }

    let contents = unsafe { memmap::Mmap::map(&file)? };
    let vsb = VeritySuperBlock::read_from_prefix(&contents[contents.len() - 512..]).unwrap();

    debug!("Size: {}", contents.len());
    debug!("Data block size: {}", vsb.data_block_size);
    debug!("Hash block size: {}", vsb.hash_block_size);
    debug!("Data block count: {}", vsb.data_block_count);

    let sb_offset = u64::from(vsb.data_block_size) * u64::from(vsb.data_block_count);
    let tar = fs::Tar::new(contents, sb_offset)?;

    daemonize::Daemonize::new()
        .start()
        .map_err(|e| Error::new(ErrorKind::Other, e))?;

    fuser::mount2(tar, mountpoint, &options)
}
