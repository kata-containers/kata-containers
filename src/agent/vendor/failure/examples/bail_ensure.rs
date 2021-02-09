#[macro_use]
extern crate failure;

use failure::Error;

fn bailer() -> Result<(), Error> {
    // bail!("ruh roh");
    bail!("ruh {}", "roh");
}

fn ensures() -> Result<(), Error> {
    ensure!(true, "true is false");
    ensure!(false, "false is false");
    Ok(())
}

fn main() {
    match bailer() {
        Ok(_) => println!("ok"),
        Err(e) => println!("{}", e),
    }
    match ensures() {
        Ok(_) => println!("ok"),
        Err(e) => println!("{}", e),
    }
}
