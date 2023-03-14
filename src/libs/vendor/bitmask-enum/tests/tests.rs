#[cfg(test)]
mod tests {
    use bitmask_enum::bitmask;

    #[bitmask]
    enum Bitmask {
        Flag1,
        Flag2,
        Flag3,
        Flag4,
        Flag5,
        Flag6,
        Flag7,
        Flag8,
    }

    #[test]
    fn test() {
        let mut bm = Bitmask::none();
        assert_eq!(bm, 0);

        bm |= Bitmask::Flag5;
        assert_eq!(bm, Bitmask::Flag5);

        bm |= Bitmask::Flag1 | Bitmask::Flag8;
        assert_eq!(bm, 0b10010001);

        bm &= !Bitmask::Flag1 & !Bitmask::Flag5;
        assert_eq!(bm, 0b10000000);

        bm |= !Bitmask::Flag8;
        assert_eq!(bm.is_all(), true);
    }

    #[test]
    fn test_bits() {
        let all = Bitmask::all();
        assert_eq!(all.bits(), std::usize::MAX);
    }

    #[test]
    fn test_all() {
        let all = Bitmask::all();
        assert_eq!(all.is_all(), true);
        assert_eq!(all, std::usize::MAX);
    }

    #[test]
    fn test_none() {
        let none = Bitmask::none();
        assert_eq!(none.is_none(), true);
        assert_eq!(none, std::usize::MIN);
    }

    #[test]
    fn test_intersects() {
        let bm = Bitmask::Flag4;
        assert_eq!(bm.intersects(Bitmask::Flag4), true);
        assert_eq!(bm.intersects(Bitmask::Flag4 | Bitmask::Flag1), true);
        assert_eq!(bm.intersects(Bitmask::Flag1), false);
    }

    #[test]
    fn test_contains() {
        let bm = Bitmask::Flag4 | Bitmask::Flag6;
        assert_eq!(bm.contains(Bitmask::Flag4), true);
        assert_eq!(bm.contains(Bitmask::Flag4 | Bitmask::Flag6), true);
        assert_eq!(bm.contains(Bitmask::Flag1), false);
        assert_eq!(bm.contains(Bitmask::Flag4 | Bitmask::Flag1), false);
    }

    #[test]
    fn test_from() {
        let mask: usize = 0b100010;
        let bm = Bitmask::from(mask);

        assert_eq!(bm, Bitmask::Flag2 | Bitmask::Flag6);

        let value: usize = bm.into();
        assert_eq!(value, mask);
    }

    #[test]
    fn test_types() {
        #[bitmask(usize)]
        enum BitmaskUsize {
            Flag1,
            Flag2,
        }
        assert_eq!(BitmaskUsize::Flag1, 0b01);
        assert_eq!(BitmaskUsize::Flag2, 0b10);

        #[bitmask(u8)]
        enum BitmaskU8 {
            Flag1,
            Flag2,
        }
        assert_eq!(BitmaskU8::Flag1, 0b01);
        assert_eq!(BitmaskU8::Flag2, 0b10);

        #[bitmask(u16)]
        enum BitmaskU16 {
            Flag1,
            Flag2,
        }
        assert_eq!(BitmaskU16::Flag1, 0b01);
        assert_eq!(BitmaskU16::Flag2, 0b10);

        #[bitmask(u32)]
        enum BitmaskU32 {
            Flag1,
            Flag2,
        }
        assert_eq!(BitmaskU32::Flag1, 0b01);
        assert_eq!(BitmaskU32::Flag2, 0b10);

        #[bitmask(u64)]
        enum BitmaskU64 {
            Flag1,
            Flag2,
        }
        assert_eq!(BitmaskU64::Flag1, 0b01);
        assert_eq!(BitmaskU64::Flag2, 0b10);

        #[bitmask(u128)]
        enum BitmaskU128 {
            Flag1,
            Flag2,
        }
        assert_eq!(BitmaskU128::Flag1, 0b01);
        assert_eq!(BitmaskU128::Flag2, 0b10);
        #[bitmask(isize)]
        enum BitmaskIsize {
            Flag1,
            Flag2,
        }
        assert_eq!(BitmaskUsize::Flag1, 0b01);
        assert_eq!(BitmaskUsize::Flag2, 0b10);

        #[bitmask(i8)]
        enum BitmaskI8 {
            Flag1,
            Flag2,
        }
        assert_eq!(BitmaskU8::Flag1, 0b01);
        assert_eq!(BitmaskU8::Flag2, 0b10);

        #[bitmask(i16)]
        enum BitmaskI16 {
            Flag1,
            Flag2,
        }
        assert_eq!(BitmaskU16::Flag1, 0b01);
        assert_eq!(BitmaskU16::Flag2, 0b10);

        #[bitmask(i32)]
        enum BitmaskI32 {
            Flag1,
            Flag2,
        }
        assert_eq!(BitmaskU32::Flag1, 0b01);
        assert_eq!(BitmaskU32::Flag2, 0b10);

        #[bitmask(i64)]
        enum BitmaskI64 {
            Flag1,
            Flag2,
        }
        assert_eq!(BitmaskU64::Flag1, 0b01);
        assert_eq!(BitmaskU64::Flag2, 0b10);

        #[bitmask(i128)]
        enum BitmaskI128 {
            Flag1,
            Flag2,
        }
        assert_eq!(BitmaskU128::Flag1, 0b01);
        assert_eq!(BitmaskU128::Flag2, 0b10);
    }
}
