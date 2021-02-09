use super::*;

const EMPTY_HASH: &str = "786a02f742015903c6c6fd852552d272912f4740e15847618a86e217f71f5419\
                          d25e1031afee585313896444934eb04b903a685b1448b755d56f701afe9be2ce";
const ABC_HASH: &str = "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d1\
                        7d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923";
const ONE_BLOCK_HASH: &str = "865939e120e6805438478841afb739ae4250cf372653078a065cdcfffca4caf7\
                              98e6d462b65d658fc165782640eded70963449ae1500fb0f24981d7727e22c41";
const THOUSAND_HASH: &str = "1ee4e51ecab5210a518f26150e882627ec839967f19d763e1508b12cfefed148\
                             58f6a1c9d1f969bc224dc9440f5a6955277e755b9c513f9ba4421c5e50c8d787";

#[test]
fn test_update_state() {
    let io = &[
        (&b""[..], EMPTY_HASH),
        (&b"abc"[..], ABC_HASH),
        (&[0; BLOCKBYTES], ONE_BLOCK_HASH),
        (&[0; 1000], THOUSAND_HASH),
    ];
    // Test each input all at once.
    for &(input, output) in io {
        let hash = blake2b(input);
        assert_eq!(&hash.to_hex(), output, "hash mismatch");
    }
    // Now in two chunks. This is especially important for the ONE_BLOCK case, because it would be
    // a mistake for update() to call compress, even though the buffer is full.
    for &(input, output) in io {
        let mut state = State::new();
        let split = input.len() / 2;
        state.update(&input[..split]);
        assert_eq!(split as Count, state.count());
        state.update(&input[split..]);
        assert_eq!(input.len() as Count, state.count());
        let hash = state.finalize();
        assert_eq!(&hash.to_hex(), output, "hash mismatch");
    }
    // Now one byte at a time.
    for &(input, output) in io {
        let mut state = State::new();
        let mut count = 0;
        for &b in input {
            state.update(&[b]);
            count += 1;
            assert_eq!(count, state.count());
        }
        let hash = state.finalize();
        assert_eq!(&hash.to_hex(), output, "hash mismatch");
    }
}

#[test]
fn test_multiple_finalizes() {
    let mut state = State::new();
    assert_eq!(&state.finalize().to_hex(), EMPTY_HASH, "hash mismatch");
    assert_eq!(&state.finalize().to_hex(), EMPTY_HASH, "hash mismatch");
    assert_eq!(&state.finalize().to_hex(), EMPTY_HASH, "hash mismatch");
    state.update(b"abc");
    assert_eq!(&state.finalize().to_hex(), ABC_HASH, "hash mismatch");
    assert_eq!(&state.finalize().to_hex(), ABC_HASH, "hash mismatch");
    assert_eq!(&state.finalize().to_hex(), ABC_HASH, "hash mismatch");
}

#[cfg(feature = "std")]
#[test]
fn test_write() {
    use std::io::prelude::*;

    let mut state = State::new();
    state.write_all(&[0; 1000]).unwrap();
    let hash = state.finalize();
    assert_eq!(&hash.to_hex(), THOUSAND_HASH, "hash mismatch");
}

// You can check this case against the equivalent Python:
//
// import hashlib
// hashlib.blake2b(
//     b'foo',
//     digest_size=18,
//     key=b"bar",
//     salt=b"bazbazbazbazbazb",
//     person=b"bing bing bing b",
//     fanout=2,
//     depth=3,
//     leaf_size=0x04050607,
//     node_offset=0x08090a0b0c0d0e0f,
//     node_depth=16,
//     inner_size=17,
//     last_node=True,
// ).hexdigest()
#[test]
fn test_all_parameters() {
    let mut params = Params::new();
    params
        .hash_length(18)
        // Make sure a shorter key properly overwrites a longer one.
        .key(b"not the real key")
        .key(b"bar")
        .salt(b"bazbazbazbazbazb")
        .personal(b"bing bing bing b")
        .fanout(2)
        .max_depth(3)
        .max_leaf_length(0x04050607)
        .node_offset(0x08090a0b0c0d0e0f)
        .node_depth(16)
        .inner_hash_length(17)
        .last_node(true);

    // Check the State API.
    assert_eq!(
        "ec0f59cb65f92e7fcca1280ba859a6925ded",
        &params.to_state().update(b"foo").finalize().to_hex()
    );

    // Check the all-at-once API.
    assert_eq!(
        "ec0f59cb65f92e7fcca1280ba859a6925ded",
        &params.hash(b"foo").to_hex()
    );
}

#[test]
fn test_all_parameters_blake2bp() {
    let mut params = blake2bp::Params::new();
    params
        .hash_length(18)
        // Make sure a shorter key properly overwrites a longer one.
        .key(b"not the real key")
        .key(b"bar");

    // Check the State API.
    assert_eq!(
        "8c54e888a8a01c63da6585c058fe54ea81df",
        &params.to_state().update(b"foo").finalize().to_hex()
    );

    // Check the all-at-once API.
    assert_eq!(
        "8c54e888a8a01c63da6585c058fe54ea81df",
        &params.hash(b"foo").to_hex()
    );
}

#[test]
#[should_panic]
fn test_short_hash_length_panics() {
    Params::new().hash_length(0);
}

#[test]
#[should_panic]
fn test_long_hash_length_panics() {
    Params::new().hash_length(OUTBYTES + 1);
}

#[test]
#[should_panic]
fn test_long_key_panics() {
    Params::new().key(&[0; KEYBYTES + 1]);
}

#[test]
#[should_panic]
fn test_long_salt_panics() {
    Params::new().salt(&[0; SALTBYTES + 1]);
}

#[test]
#[should_panic]
fn test_long_personal_panics() {
    Params::new().personal(&[0; PERSONALBYTES + 1]);
}

#[test]
fn test_zero_max_depth_supported() {
    Params::new().max_depth(0);
}

#[test]
#[should_panic]
fn test_long_inner_hash_length_panics() {
    Params::new().inner_hash_length(OUTBYTES + 1);
}

#[test]
#[should_panic]
fn test_blake2bp_short_hash_length_panics() {
    blake2bp::Params::new().hash_length(0);
}

#[test]
#[should_panic]
fn test_blake2bp_long_hash_length_panics() {
    blake2bp::Params::new().hash_length(OUTBYTES + 1);
}

#[test]
#[should_panic]
fn test_blake2bp_long_key_panics() {
    blake2bp::Params::new().key(&[0; KEYBYTES + 1]);
}
