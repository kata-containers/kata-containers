#[cfg(feature = "digest-trait")]
mod sha1_oracle {
    use sha1::Sha1;
    use digest::Digest;
    use sha1collisiondetection::Sha1CD;

    fn initialize() -> digest::Output<Sha1CD> {
        let mut d = digest::Output::<Sha1CD>::default();
        getrandom::getrandom(&mut d[..]).unwrap();
        d
    }

    #[test]
    fn short() {
        /// How often to run the test.
        const N: usize = 100_000;

        let mut d = initialize();
        for _ in 0..N {
            let a = Sha1::digest(&d);
            let b = Sha1CD::digest(&d);
            assert_eq!(a, b);
            d = a;
        }
    }

    #[test]
    fn long() {
        /// How often to run the test.  Careful, quadratic runtime.
        const N: usize = 1_000;
        /// SHA1 divides input into blocks.
        const BS: usize = 64;

        let d = initialize();
        let mut buf = Vec::with_capacity(20 * N);
        buf.extend_from_slice(&d[..]);
        for i in 0..N {
            let input = if buf.len() > 2 * BS {
                // Exercise padding.
                &buf[..buf.len() - (i % BS)]
            } else {
                &buf[..]
            };
            let a = Sha1::digest(input);
            let b = Sha1CD::digest(input);
            assert_eq!(a, b);
            buf.extend_from_slice(&a[..]);
        }
    }
}
