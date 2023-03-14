use bitmask_enum::bitmask;

#[bitmask(u8)]
enum Bitmask {
    Flag5 = 0b00010000,
    Flag3 = 0b00000100,
    Flag1 = 0b00000001,

    Flag51_1 = 0b00010000 | 0b00000001,
    Flag51_2 = Self::Flag5.or(Self::Flag1).bits,
    Flag51_3 = Self::Flag5.bits | Self::Flag1.bits,

    Flag513 = {
        let flag51 = Self::Flag51_1.bits;
        flag51 | Self::Flag3.bits
    },
}

fn main() {
    let bm = Bitmask::Flag5 | Bitmask::Flag1;

    println!("{:#010b}", bm); // 0b00010001
    println!("{}", bm == Bitmask::Flag51_1); // true

    println!("{:#010b}", Bitmask::Flag513); // 0b00010101
}
