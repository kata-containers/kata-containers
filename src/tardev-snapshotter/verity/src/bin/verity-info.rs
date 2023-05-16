use std::{env, fs::File, io, io::Read, io::Seek, process};
use zerocopy::AsBytes;

fn main() -> io::Result<()> {
    let argv: Vec<String> = env::args().collect();
    if argv.len() != 2 {
        eprintln!("Usage: {} <verity-file.bin>", argv[0]);
        process::exit(1);
    }

    let mut file = File::open(&argv[1])?;
    let size = file.seek(io::SeekFrom::End(0))?;
    if size < 4096 {
        eprintln!("File is too small: {size}");
        process::exit(1);
    }

    file.seek(std::io::SeekFrom::End(-4096))?;
    let mut buf = [0u8; 4096];
    file.read_exact(&mut buf)?;

    let mut sb = verity::SuperBlock::default();
    sb.as_bytes_mut()
        .copy_from_slice(&buf[4096 - 512..][..std::mem::size_of::<verity::SuperBlock>()]);
    let data_block_size = u64::from(sb.data_block_size.get());
    let hash_block_size = u64::from(sb.hash_block_size.get());
    let data_size = if let Some(v) = sb.data_block_count.get().checked_mul(data_block_size) {
        v
    } else {
        eprintln!("Overflow when calculating the data size");
        process::exit(1);
    };

    if data_size > size {
        eprintln!("Data size ({data_size}) is greater than device size ({size})");
        process::exit(1);
    }

    println!("Data block size: {data_block_size}");
    println!("Data block clount: {}", sb.data_block_count.get());
    println!("Hash block size: {hash_block_size}");
    println!("Hash offset: {data_size}");
    println!("veritysetup verify --data-block-size={data_block_size}  --data-blocks={} --hash-block-size={hash_block_size} --hash-offset={data_size} --no-superblock -s 0000000000000000000000000000000000000000000000000000000000000000 {} {}", sb.data_block_count.get(), argv[1], argv[1]);

    Ok(())
}
