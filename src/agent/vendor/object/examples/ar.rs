use std::{env, fs, process};

fn main() {
    let mut args = env::args().skip(1);
    if args.len() < 2 {
        eprintln!(
            "Usage: {} <op> <archive> [<member>...]",
            env::args().next().unwrap()
        );
        process::exit(1);
    }

    let op = args.next().unwrap();
    let file_path = args.next().unwrap();

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
    let archive = match object::read::archive::ArchiveFile::parse(&*file) {
        Ok(file) => file,
        Err(err) => {
            println!("Failed to parse file '{}': {}", file_path, err);
            return;
        }
    };
    match op.chars().next().unwrap() {
        't' => {
            println!("kind: {:?}", archive.kind());
            for member in archive.members() {
                let member = member.unwrap();
                println!("{}", String::from_utf8_lossy(member.name()));
            }
        }
        op => println!("Invalid operation: {}", op),
    }
}
