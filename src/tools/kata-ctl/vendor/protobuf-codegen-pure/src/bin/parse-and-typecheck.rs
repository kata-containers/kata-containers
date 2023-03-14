extern crate protobuf_codegen_pure;

use std::env;
use std::path::Path;
use std::process::exit;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.len() != 2 {
        eprintln!(
            "usage: {} <input.proto> <include>",
            env::args().next().unwrap()
        );
        exit(1);
    }

    eprintln!(
        "{} is not a part of public interface",
        env::args().next().unwrap()
    );

    let input = vec![Path::new(&args[0][..])];
    let includes = vec![Path::new(&args[1][..])];
    let t =
        protobuf_codegen_pure::parse_and_typecheck(&includes, &input).expect("parse_and_typecheck");
    for fd in t.file_descriptors {
        println!("{:#?}", fd);
    }
}
