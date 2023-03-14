use std::ffi::OsString;

fn main() {
    let args: Vec<OsString> = go_flag::parse(|_| ());
    for arg in &args {
        println!("{}", arg.to_string_lossy());
    }
}
