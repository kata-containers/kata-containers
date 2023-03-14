use cipher::block::{BlockCipher, NewBlockCipher};
use cipher::generic_array::GenericArray;
use hex_literal::hex;
use twofish::Twofish;

macro_rules! new_test {
    (
        $name:ident, $key_len:expr,
        $r1:expr, $r2:expr, $r3:expr,
        $r4:expr, $r5:expr, $r48:expr,
    ) => {
        #[test]
        fn $name() {
            let mut key = [0u8; $key_len];
            let mut plain = GenericArray::default();
            let mut cipher;

            for i in 1..50 {
                let twofish = Twofish::new_varkey(&key).unwrap();

                let mut buf = plain.clone();
                twofish.encrypt_block(&mut buf);
                cipher = buf.clone();
                twofish.decrypt_block(&mut buf);
                assert_eq!(plain, buf);

                let correct = match i {
                    1 => Some(hex!($r1)),
                    2 => Some(hex!($r2)),
                    3 => Some(hex!($r3)),
                    4 => Some(hex!($r4)),
                    5 => Some(hex!($r5)),
                    48 => Some(hex!($r48)),
                    _ => None,
                };

                correct.map(|v| assert_eq!(&cipher[..], v, "i = {}", i));
                let (l, r) = key.split_at_mut(16);
                r.copy_from_slice(&l[..$key_len - 16]);
                l.copy_from_slice(&plain);
                plain = cipher;
            }
        }
    };
}

new_test!(
    encrypt_ecb128,
    16,
    "9F589F5CF6122C32B6BFEC2F2AE8C35A",
    "D491DB16E7B1C39E86CB086B789F5419",
    "019F9809DE1711858FAAC3A3BA20FBC3",
    "6363977DE839486297E661C6C9D668EB",
    "816D5BD0FAE35342BF2A7412C246F752",
    "6B459286F3FFD28D49F15B1581B08E42",
);

new_test!(
    encrypt_ecb192,
    24,
    "EFA71F788965BD4453F860178FC19101",
    "88B2B2706B105E36B446BB6D731A1E88",
    "39DA69D6BA4997D585B6DC073CA341B2",
    "182B02D81497EA45F9DAACDC29193A65",
    "7AFF7A70CA2FF28AC31DD8AE5DAAAB63",
    "F0AB73301125FA21EF70BE5385FB76B6",
);

new_test!(
    encrypt_ecb256,
    32,
    "57FF739D4DC92C1BD7FC01700CC8216F",
    "D43BB7556EA32E46F2A282B7D45B4E0D",
    "90AFE91BB288544F2C32DC239B2635E6",
    "6CB4561C40BF0A9705931CB6D408E7FA",
    "3059D6D61753B958D92F4781C8640E58",
    "431058F4DBC7F734DA4F02F04CC4F459",
);
