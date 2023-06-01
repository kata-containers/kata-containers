use generic_array::typenum::Unsigned;
use sha2::{digest::OutputSizeUser, Sha256};
use std::{env, fs::File, fs::OpenOptions, io, io::Seek, process};
use verity::{append_tree, traverse_file, Verity};

fn main() -> io::Result<()> {
    let argv: Vec<String> = env::args().collect();
    if argv.len() != 3 && argv.len() != 4 {
        eprintln!("Usage: {} <r|t|a> <device.bin> [tree.bin]", argv[0]);
        process::exit(1);
    }

    let mut reader = File::open(&argv[2])?;
    let file_size = reader.seek(io::SeekFrom::End(0))?;
    reader.rewind()?;

    if file_size == 0 {
        eprintln!("Empty input file.");
        process::exit(1);
    }

    let salt = [0u8; <Sha256 as OutputSizeUser>::OutputSize::USIZE];

    match argv[1].as_ref() {
        // Append the tree to the file.
        "a" => {
            let mut file = OpenOptions::new().read(true).write(true).open(&argv[2])?;
            println!("Root hash: {:x}", append_tree::<Sha256>(&mut file)?);
        }

        // Create the tree in a separate file.
        "t" => {
            if argv.len() != 4 {
                eprintln!("Must specify the name of the output file");
                process::exit(1);
            }

            let mut writer = File::create(&argv[3])?;
            let verity = Verity::<Sha256>::new(file_size, 4096, 4096, &salt, 0)?;
            println!(
                "Root hash: {:x}",
                traverse_file(
                    &mut reader,
                    0,
                    false,
                    verity,
                    &mut verity::write_to(&mut writer)
                )?
            );
        }

        // Calculate the root hash without writing the tree.
        "r" => {
            let verity = Verity::<Sha256>::new(file_size, 4096, 4096, &salt, 0)?;
            println!(
                "Root hash: {:x}",
                traverse_file(&mut reader, 0, false, verity, &mut verity::no_write)?
            );
        }

        _ => {
            eprintln!("Unknown command: {}", argv[1]);
            process::exit(1);
        }
    }

    Ok(())
}
