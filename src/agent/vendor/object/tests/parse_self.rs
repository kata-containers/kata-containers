use object::{File, Object};
use std::{env, fs};

#[test]
fn parse_self() {
    let exe = env::current_exe().unwrap();
    let data = fs::read(exe).unwrap();
    let object = File::parse(&data).unwrap();
    assert!(object.entry() != 0);
    assert!(object.sections().count() != 0);
}
