use std::{env, fs::OpenOptions, io, process};
use tarindex::append_index;

fn main() -> io::Result<()> {
    let argv: Vec<String> = env::args().collect();
    if argv.len() != 2 {
        eprintln!("Usage: {} <file.tar>", argv[0]);
        process::exit(1);
    }

    let mut file = OpenOptions::new().read(true).write(true).open(&argv[1])?;
    append_index(&mut file)
}
