use object::{Object, ObjectComdat, ObjectSection, ObjectSymbol};
use std::{env, fs, process};

fn main() {
    let arg_len = env::args().len();
    if arg_len <= 1 {
        eprintln!("Usage: {} <file> ...", env::args().next().unwrap());
        process::exit(1);
    }

    for file_path in env::args().skip(1) {
        if arg_len > 2 {
            println!();
            println!("{}:", file_path);
        }

        let file = match fs::File::open(&file_path) {
            Ok(file) => file,
            Err(err) => {
                println!("Failed to open file '{}': {}", file_path, err,);
                return;
            }
        };
        let file = match unsafe { memmap::Mmap::map(&file) } {
            Ok(mmap) => mmap,
            Err(err) => {
                println!("Failed to map file '{}': {}", file_path, err,);
                return;
            }
        };
        let file = match object::File::parse(&*file) {
            Ok(file) => file,
            Err(err) => {
                println!("Failed to parse file '{}': {}", file_path, err);
                return;
            }
        };

        match file.mach_uuid() {
            Ok(Some(uuid)) => println!("Mach UUID: {:x?}", uuid),
            Ok(None) => {}
            Err(e) => println!("Failed to parse Mach UUID: {}", e),
        }
        match file.build_id() {
            Ok(Some(build_id)) => println!("Build ID: {:x?}", build_id),
            Ok(None) => {}
            Err(e) => println!("Failed to parse build ID: {}", e),
        }
        match file.gnu_debuglink() {
            Ok(Some((filename, crc))) => println!(
                "GNU debug link: {} CRC: {:08x}",
                String::from_utf8_lossy(filename),
                crc,
            ),
            Ok(None) => {}
            Err(e) => println!("Failed to parse GNU debug link: {}", e),
        }

        for segment in file.segments() {
            println!("{:?}", segment);
        }

        for section in file.sections() {
            println!("{}: {:?}", section.index().0, section);
        }

        for comdat in file.comdats() {
            print!("{:?} Sections:", comdat);
            for section in comdat.sections() {
                print!(" {}", section.0);
            }
            println!();
        }

        for symbol in file.symbols() {
            println!("{}: {:?}", symbol.index().0, symbol);
        }

        for section in file.sections() {
            if section.relocations().next().is_some() {
                println!(
                    "\n{} relocations",
                    section.name().unwrap_or("<invalid name>")
                );
                for relocation in section.relocations() {
                    println!("{:?}", relocation);
                }
            }
        }
    }
}
