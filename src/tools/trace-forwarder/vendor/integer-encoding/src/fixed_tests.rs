#[cfg(test)]
mod tests {
    use crate::fixed::FixedInt;

    #[cfg(any(feature = "tokio_async", feature = "futures_async"))]
    use crate::reader::FixedIntAsyncReader;
    #[cfg(any(feature = "tokio_async", feature = "futures_async"))]
    use crate::writer::FixedIntAsyncWriter;

    use crate::reader::FixedIntReader;
    use crate::writer::FixedIntWriter;

    #[test]
    fn test_u32_enc() {
        let result = (32 as u32).encode_fixed_vec();
        assert_eq!(result, vec![32, 0, 0, 0]);
    }
    #[test]
    fn test_u16_enc() {
        let result = (256 as u16).encode_fixed_vec();
        assert_eq!(result, vec![0, 1]);
    }
    #[test]
    fn test_i16_enc() {
        let result = (-32768 as i16).encode_fixed_vec();
        assert_eq!(result, vec![0, 128]);
    }
    #[test]
    fn test_i32_enc() {
        let result = (-32767 as i32).encode_fixed_vec();
        assert_eq!(result, vec![1, 128, 255, 255]);
    }

    // This must fail to compile:
    /*
    fn test() -> &'static [u8] {
        let int = -32767 as i32;
        let result = int.encode_fixed_light();
        assert_eq!(result, &[1, 128, 255, 255]);
        result
    }
    */

    #[test]
    fn test_i32_enc_light() {
        let int = -32767 as i32;
        let result = int.encode_fixed_light();
        assert_eq!(result, &[1, 128, 255, 255]);
    }
    #[test]
    fn test_all_identity() {
        let a: u16 = 17;
        let b: u32 = 17;
        let c: u64 = 17;
        let d: i16 = -17;
        let e: i32 = -17;
        let f: i64 = -17;

        assert_eq!(a, FixedInt::decode_fixed_vec(&a.encode_fixed_vec()));
        assert_eq!(b, FixedInt::decode_fixed_vec(&b.encode_fixed_vec()));
        assert_eq!(c, FixedInt::decode_fixed_vec(&c.encode_fixed_vec()));
        assert_eq!(d, FixedInt::decode_fixed_vec(&d.encode_fixed_vec()));
        assert_eq!(e, FixedInt::decode_fixed_vec(&e.encode_fixed_vec()));
        assert_eq!(f, FixedInt::decode_fixed_vec(&f.encode_fixed_vec()));

        assert_eq!(a, FixedInt::decode_fixed(&a.encode_fixed_light()));
        assert_eq!(b, FixedInt::decode_fixed(&b.encode_fixed_light()));
        assert_eq!(c, FixedInt::decode_fixed(&c.encode_fixed_light()));
        assert_eq!(d, FixedInt::decode_fixed(&d.encode_fixed_light()));
        assert_eq!(e, FixedInt::decode_fixed(&e.encode_fixed_light()));
        assert_eq!(f, FixedInt::decode_fixed(&f.encode_fixed_light()));
    }

    #[test]
    fn test_reader_writer() {
        let mut buf = Vec::with_capacity(128);

        let i1: u32 = 123;
        let i2: u32 = 124;
        let i3: u32 = 125;

        assert!(buf.write_fixedint(i1).is_ok());
        assert!(buf.write_fixedint(i2).is_ok());
        assert!(buf.write_fixedint(i3).is_ok());

        assert_eq!(3 * 4, buf.len());

        let mut reader: &[u8] = buf.as_ref();

        let i1_res = reader.read_fixedint().unwrap();
        let i2_res = reader.read_fixedint().unwrap();
        let i3_res = reader.read_fixedint().unwrap();

        assert_eq!(i1, i1_res);
        assert_eq!(i2, i2_res);
        assert_eq!(i3, i3_res);

        assert!(reader.read_fixedint::<u32>().is_err());
    }

    #[should_panic]
    #[test]
    fn test_invalid_decode_size() {
        assert_eq!(33, u64::decode_fixed(&[1, 0, 0, 0, 0, 1]));
    }
    #[should_panic]
    #[test]
    fn test_invalid_encode_size() {
        let mut buf = [0 as u8; 4];
        (11 as u64).encode_fixed(&mut buf);
    }

    #[cfg(any(feature = "tokio_async", feature = "futures_async"))]
    #[tokio::test]
    async fn test_async_reader() {
        let mut buf = Vec::with_capacity(128);

        let i1: u32 = 1;
        let i2: u32 = 65532;
        let i3: u32 = 4200123456;
        let i4: i64 = i3 as i64 * 1000;
        let i5: i32 = -32456;

        buf.write_fixedint_async(i1).await.unwrap();
        buf.write_fixedint_async(i2).await.unwrap();
        buf.write_fixedint_async(i3).await.unwrap();
        buf.write_fixedint_async(i4).await.unwrap();
        buf.write_fixedint_async(i5).await.unwrap();

        let mut reader: &[u8] = buf.as_ref();

        assert_eq!(i1, reader.read_fixedint_async().await.unwrap());
        assert_eq!(i2, reader.read_fixedint_async().await.unwrap());
        assert_eq!(i3, reader.read_fixedint_async().await.unwrap());
        assert_eq!(i4, reader.read_fixedint_async().await.unwrap());
        assert_eq!(i5, reader.read_fixedint_async().await.unwrap());
        assert!(reader.read_fixedint_async::<u32>().await.is_err());
    }
}
