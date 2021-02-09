extern crate protobuf_codegen_pure;

use std::env;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    assert_eq!(2, args.len());
    let includes = vec![Path::new(&args[0][..])];
    let input = vec![Path::new(&args[1][..])];
    let t =
        protobuf_codegen_pure::parse_and_typecheck(&includes, &input).expect("parse_and_typecheck");
    for fd in t.file_descriptors {
        println!("{:?}", fd);
    }
}
