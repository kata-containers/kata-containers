use caps::Capability;
use std::str::FromStr;

fn main() {
    let input = std::env::args().nth(1).expect("missing argument");
    match Capability::from_str(&input.to_uppercase()) {
        Ok(p) => println!(
            "Parsed: {} -> index={}, bitmask={}",
            p,
            p.index(),
            p.bitmask()
        ),
        Err(e) => println!("{}", e),
    }
}
