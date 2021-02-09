#[macro_use]
extern crate byteordered;

use byteordered::{ByteOrdered, Endianness};

#[test]
fn test_macro_one_read() {
    let x: &[u8] = &[1, 2, 1, 2];
    let mut c = x;
    let v = with_order!(&mut c, Endianness::le_iff(2 + 2 == 4), |data| {
        let v = data.read_u16().unwrap();
        assert_eq!(v, 513);
        v
    });
    assert_eq!(v, 513);

    let v = with_order!(&mut c, Endianness::Big, |data| {
        let v = data.read_u16().unwrap();
        assert_eq!(v, 258);
        v
    });
    assert_eq!(v, 258);
}

#[test]
fn test_macro_one_read_2() {
    let x: &[u8] = &[16, 1];
    let v = with_order!(x, Endianness::Little, |data| { data.read_u16().unwrap() });
    assert_eq!(v, 272);

    let x: &[u8] = &[1, 16];
    let v = with_order!(x, Endianness::Big, |data| { data.read_u16().unwrap() });
    assert_eq!(v, 272);
}

#[test]
fn test_macro_multi_pipe() {
    let x: &[u8] = &[1, 2, 3, 4];
    let mut sink = Vec::new();
    let mut c = x;

    with_order!(
        (&mut c, &mut sink),
        Endianness::le_iff(2 + 2 == 4),
        |input, output| {
            let v = input.read_u16().unwrap();
            output.write_u16(v + 10).unwrap();
        }
    );

    with_order!((&mut c, &mut sink), Endianness::Big, |input, output| {
        let v = input.read_u16().unwrap();
        output.write_u16(v + 100).unwrap();
    });

    assert_eq!(sink, vec![11, 2, 3, 104]);
}

#[test]
fn test_macro_byteordered() {
    let x: &[u8] = &[1, 2, 1, 2];
    let mut reader = ByteOrdered::runtime(x, Endianness::le_iff(2 + 2 == 4));
    let v = with_order!(reader.as_mut(), |data| {
        let v = data.read_u16().unwrap();
        assert_eq!(v, 513);
        v
    });

    assert_eq!(v, 513);

    let v = with_order!(reader, Endianness::Big, |data| {
        let v = data.read_u16().unwrap();
        assert_eq!(v, 258);
        v
    });
    assert_eq!(v, 258);
}
