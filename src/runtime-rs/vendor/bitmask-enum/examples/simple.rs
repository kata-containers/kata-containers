use bitmask_enum::bitmask;

#[bitmask(u8)]
enum Bitmask {
    Flag1, // defaults to 0d00000001
    Flag2, // defaults to 0d00000010
    Flag3, // defaults to 0d00000100
}

// It is possible to impl on the bitmask and use its
impl Bitmask {
    fn _set_to(&mut self, val: u8) {
        self.bits = val
    }
}

// bitmask has const bitwise operator methods
const CONST_BM: Bitmask = Bitmask::Flag2.or(Bitmask::Flag3);

fn main() {
    println!("{:#010b}", CONST_BM); // 0b00000110

    // Bitmask that contains Flag1 and Flag3
    let bm = Bitmask::Flag1 | Bitmask::Flag3;

    println!("{:#010b}", bm); // 0b00000101

    // Does bm intersect one of CONST_BM
    println!("{}", bm.intersects(CONST_BM)); // true

    // Does bm contain all of CONST_BM
    println!("{}", bm.contains(CONST_BM)); // false
}
