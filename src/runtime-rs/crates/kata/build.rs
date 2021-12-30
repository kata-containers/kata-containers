use vergen::{vergen, Config};

fn main() {
    // Generate the default 'cargo:' instruction output
    vergen(Config::default()).unwrap();
}
