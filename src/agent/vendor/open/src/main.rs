use std::{env, process};

fn main() {
    let path_or_url = match env::args().nth(1) {
        Some(arg) => arg,
        None => {
            eprintln!("usage: open <path-or-url>");
            process::exit(1);
        }
    };

    match open::that(&path_or_url) {
        Ok(()) => println!("Opened '{}' successfully.", path_or_url),
        Err(err) => {
            eprintln!("An error occurred when opening '{}': {}", path_or_url, err);
            process::exit(3);
        }
    }
}
