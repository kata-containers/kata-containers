const CHACHA_TAU: &[u8] = b"expand 32-byte k";

fn chacha_quarter_round(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
	state[a] = state[a].wrapping_add(state[b]);
	state[d] ^= state[a];
	state[d] = state[d].rotate_left(16);

	state[c] = state[c].wrapping_add(state[d]);
	state[b] ^= state[c];
	state[b] = state[b].rotate_left(12);

	state[a] = state[a].wrapping_add(state[b]);
	state[d] ^= state[a];
	state[d] = state[d].rotate_left(8);

	state[c] = state[c].wrapping_add(state[d]);
	state[b] ^= state[c];
	state[b] = state[b].rotate_left(7);
}

const fn chacha_pack(unpacked: &[u8], idx: usize) -> u32 {
	(unpacked[idx] as u32)
		| ((unpacked[idx + 1] as u32) << 8)
		| ((unpacked[idx + 2] as u32) << 16)
		| ((unpacked[idx + 3] as u32) << 24)
}

/// Do one ChaCha round on the input data.
pub fn chacha_block<const ROUNDS: u8>(input: [u32; 16]) -> [u32; 16] {
	let mut x = input;
	assert_eq!(ROUNDS % 2, 0, "ChaCha rounds must be divisble by 2!");
	for _ in (0..ROUNDS).step_by(2) {
		// Odd rounds
		chacha_quarter_round(&mut x, 0, 4, 8, 12);
		chacha_quarter_round(&mut x, 1, 5, 9, 13);
		chacha_quarter_round(&mut x, 2, 6, 10, 14);
		chacha_quarter_round(&mut x, 3, 7, 11, 15);
		// Even rounds
		chacha_quarter_round(&mut x, 0, 5, 10, 15);
		chacha_quarter_round(&mut x, 1, 6, 11, 12);
		chacha_quarter_round(&mut x, 2, 7, 8, 13);
		chacha_quarter_round(&mut x, 3, 4, 9, 14);
	}
	x.iter_mut()
		.zip(input.iter())
		.for_each(|(l, r)| *l = l.wrapping_add(*r));
	x
}

/// Initialize the ChaCha internal state, with a 256-bit key and 64-bit nonce.
pub const fn chacha_init(key: [u8; 32], nonce: [u8; 8]) -> [u32; 16] {
	let mut state = [0u32; 16];
	state[0] = chacha_pack(CHACHA_TAU, 0);
	state[1] = chacha_pack(CHACHA_TAU, 4);
	state[2] = chacha_pack(CHACHA_TAU, 8);
	state[3] = chacha_pack(CHACHA_TAU, 12);

	state[4] = chacha_pack(&key, 0);
	state[5] = chacha_pack(&key, 4);
	state[6] = chacha_pack(&key, 8);
	state[7] = chacha_pack(&key, 12);
	state[8] = chacha_pack(&key, 16);
	state[9] = chacha_pack(&key, 20);
	state[10] = chacha_pack(&key, 24);
	state[11] = chacha_pack(&key, 28);

	// 64-bit counter
	state[12] = 0;
	state[13] = 0;
	// Nonce
	state[14] = chacha_pack(&nonce, 0);
	state[15] = chacha_pack(&nonce, 4);
	state
}

/// Increment the 64-bit counter of the internal ChaCha20 state by 1.
/// Returns `false` if it overflows, `true` otherwise.
pub fn chacha_increment_counter(state: &mut [u32; 16]) -> bool {
	let counter = ((state[13] as u64) << 32) | (state[12] as u64);
	match counter.checked_add(1) {
		Some(new_counter) => {
			state[12] = (new_counter & 0xFFFFFFFF) as u32;
			state[13] = ((counter >> 32) & 0xFFFFFFFF) as u32;
			true
		}
		None => false,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::convert::TryInto;

	macro_rules! ietf_test_vector {
		($key_hex: tt, $nonce_hex: tt, $keystream_hex: tt) => {
			let key: [u8; 32] = hex::decode($key_hex).unwrap().try_into().unwrap();
			let nonce: [u8; 8] = hex::decode($nonce_hex).unwrap().try_into().unwrap();
			let expected_keystream: Vec<u8> = hex::decode($keystream_hex).unwrap();

			let mut state = chacha_init(key, nonce);
			let mut keystream: Vec<u8> = Vec::with_capacity(expected_keystream.len());

			while expected_keystream.len() > keystream.len() {
				chacha_block::<20>(state)
					.iter()
					.for_each(|packed| keystream.extend_from_slice(&packed.to_le_bytes()));
				chacha_increment_counter(&mut state);
			}
			keystream.resize(expected_keystream.len(), 0);

			assert_eq!(keystream, expected_keystream);
		};
	}

	#[test]
	fn test_ietf_chacha20_test_vectors() {
		ietf_test_vector!(
			"0000000000000000000000000000000000000000000000000000000000000000",
			"0000000000000000",
			"76b8e0ada0f13d90405d6ae55386bd28bdd219b8a08ded1aa836efcc8b770dc7da41597c5157488d7724e03fb8d84a376a43b8f41518a11cc387b669b2ee6586"
		);

		ietf_test_vector!(
			"0000000000000000000000000000000000000000000000000000000000000001",
			"0000000000000000",
			"4540f05a9f1fb296d7736e7b208e3c96eb4fe1834688d2604f450952ed432d41bbe2a0b6ea7566d2a5d1e7e20d42af2c53d792b1c43fea817e9ad275ae546963"
		);

		ietf_test_vector!(
			"0000000000000000000000000000000000000000000000000000000000000000",
			"0000000000000001",
			"de9cba7bf3d69ef5e786dc63973f653a0b49e015adbff7134fcb7df137821031e85a050278a7084527214f73efc7fa5b5277062eb7a0433e445f41e3"
		);

		ietf_test_vector!(
			"0000000000000000000000000000000000000000000000000000000000000000",
			"0100000000000000",
			"ef3fdfd6c61578fbf5cf35bd3dd33b8009631634d21e42ac33960bd138e50d32111e4caf237ee53ca8ad6426194a88545ddc497a0b466e7d6bbdb0041b2f586b"
		);

		ietf_test_vector!(
			"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
			"0001020304050607",
			"f798a189f195e66982105ffb640bb7757f579da31602fc93ec01ac56f85ac3c134a4547b733b46413042c9440049176905d3be59ea1c53f15916155c2be8241a38008b9a26bc35941e2444177c8ade6689de95264986d95889fb60e84629c9bd9a5acb1cc118be563eb9b3a4a472f82e09a7e778492b562ef7130e88dfe031c79db9d4f7c7a899151b9a475032b63fc385245fe054e3dd5a97a5f576fe064025d3ce042c566ab2c507b138db853e3d6959660996546cc9c4a6eafdc777c040d70eaf46f76dad3979e5c5360c3317166a1c894c94a371876a94df7628fe4eaaf2ccb27d5aaae0ad7ad0f9d4b6ad3b54098746d4524d38407a6deb3ab78fab78c9"
		);
	}
}
