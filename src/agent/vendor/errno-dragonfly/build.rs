extern crate gcc;

fn main() {
    gcc::compile_library("liberrno.a", &["src/errno.c"]);
}
