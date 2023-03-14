use std::ffi::OsString;

fn main() {
    let mut force = false;
    let mut lines = 10_i32;
    let args: Vec<OsString> = go_flag::parse(|flags| {
        flags.add_flag("f", &mut force);
        flags.add_flag("lines", &mut lines);
    });
    println!("force = {:?}", force);
    println!("lines = {:?}", lines);
    for arg in &args {
        println!("{}", arg.to_string_lossy());
    }
}
