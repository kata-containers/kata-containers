#[macro_use]
extern crate nonzero_ext;

#[test]
fn works_in_exprs() {
    let one = nonzero!(1u32);
    println!("{}", one);
    println!("{}", nonzero!(1u8));
}
