fn main() {
    print!("{}", ::std::env::args().skip(1).next().unwrap());
}
