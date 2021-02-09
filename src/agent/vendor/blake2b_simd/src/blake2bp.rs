//! BLAKE2bp, a variant of BLAKE2b that uses SIMD more efficiently.
//!
//! The AVX2 implementation of BLAKE2bp is about twice as fast that of BLAKE2b.
//! However, note that it's a different hash function, and it gives a different
//! hash from BLAKE2b for the same input.
//!
//! # Example
//!
//! ```
//! use blake2b_simd::blake2bp;
//!
//! let hash = blake2bp::Params::new()
//!     .hash_length(16)
//!     .key(b"The Magic Words are Squeamish Ossifrage")
//!     .to_state()
//!     .update(b"foo")
//!     .update(b"bar")
//!     .update(b"baz")
//!     .finalize();
//! assert_eq!("e69c7d2c42a5ac14948772231c68c552", &hash.to_hex());
//! ```

use crate::guts::{Finalize, Implementation, Job, LastNode, Stride};
use crate::many;
use crate::Count;
use crate::Hash;
use crate::Word;
use crate::BLOCKBYTES;
use crate::KEYBYTES;
use crate::OUTBYTES;
use core::cmp;
use core::fmt;
use core::mem::size_of;

#[cfg(feature = "std")]
use std;

pub(crate) const DEGREE: usize = 4;

/// Compute the BLAKE2bp hash of a slice of bytes all at once, using default
/// parameters.
///
/// # Example
///
/// ```
/// # use blake2b_simd::blake2bp::blake2bp;
/// let expected = "8ca9ccee7946afcb686fe7556628b5ba1bf9a691da37ca58cd049354d99f3704\
///                 2c007427e5f219b9ab5063707ec6823872dee413ee014b4d02f2ebb6abb5f643";
/// let hash = blake2bp(b"foo");
/// assert_eq!(expected, &hash.to_hex());
/// ```
pub fn blake2bp(input: &[u8]) -> Hash {
    Params::new().hash(input)
}

/// A parameter builder for BLAKE2bp, just like the [`Params`](../struct.Params.html) type for
/// BLAKE2b.
///
/// This builder only supports configuring the hash length and a secret key. This matches the
/// options provided by the [reference
/// implementation](https://github.com/BLAKE2/BLAKE2/blob/320c325437539ae91091ce62efec1913cd8093c2/ref/blake2.h#L162-L165).
///
/// # Example
///
/// ```
/// use blake2b_simd::blake2bp;
/// let mut state = blake2bp::Params::new().hash_length(32).to_state();
/// ```
#[derive(Clone)]
pub struct Params {
    hash_length: u8,
    key_length: u8,
    key: [u8; KEYBYTES],
    implementation: Implementation,
}

impl Params {
    /// Equivalent to `Params::default()`.
    pub fn new() -> Self {
        Self {
            hash_length: OUTBYTES as u8,
            key_length: 0,
            key: [0; KEYBYTES],
            implementation: Implementation::detect(),
        }
    }

    fn to_words(&self) -> ([[Word; 8]; DEGREE], [Word; 8]) {
        let mut base_params = crate::Params::new();
        base_params
            .hash_length(self.hash_length as usize)
            .key(&self.key[..self.key_length as usize])
            .fanout(DEGREE as u8)
            .max_depth(2)
            .max_leaf_length(0)
            // Note that inner_hash_length is always OUTBYTES, regardless of the hash_length
            // parameter. This isn't documented in the spec, but it matches the behavior of the
            // reference implementation: https://github.com/BLAKE2/BLAKE2/blob/320c325437539ae91091ce62efec1913cd8093c2/ref/blake2bp-ref.c#L55
            .inner_hash_length(OUTBYTES);
        let leaf_words = |worker_index| {
            base_params
                .clone()
                .node_offset(worker_index)
                .node_depth(0)
                // Note that setting the last_node flag here has no effect,
                // because it isn't included in the state words.
                .to_words()
        };
        let leaf_words = [leaf_words(0), leaf_words(1), leaf_words(2), leaf_words(3)];
        let root_words = base_params
            .clone()
            .node_offset(0)
            .node_depth(1)
            // Note that setting the last_node flag here has no effect, because
            // it isn't included in the state words. Also note that because
            // we're only preserving its state words, the root node won't hash
            // any key bytes.
            .to_words();
        (leaf_words, root_words)
    }

    /// Hash an input all at once with these parameters.
    pub fn hash(&self, input: &[u8]) -> Hash {
        // If there's a key, just fall back to using the State.
        if self.key_length > 0 {
            return self.to_state().update(input).finalize();
        }
        let (mut leaf_words, mut root_words) = self.to_words();
        // Hash each leaf in parallel.
        let jobs = leaf_words.iter_mut().enumerate().map(|(i, words)| {
            let input_start = cmp::min(input.len(), i * BLOCKBYTES);
            Job {
                input: &input[input_start..],
                words,
                count: 0,
                last_node: if i == DEGREE - 1 {
                    LastNode::Yes
                } else {
                    LastNode::No
                },
            }
        });
        many::compress_many(jobs, self.implementation, Finalize::Yes, Stride::Parallel);
        // Hash each leaf into the root.
        finalize_root_words(
            &leaf_words,
            &mut root_words,
            self.hash_length,
            self.implementation,
        )
    }

    /// Construct a BLAKE2bp `State` object based on these parameters.
    pub fn to_state(&self) -> State {
        State::with_params(self)
    }

    /// Set the length of the final hash, from 1 to `OUTBYTES` (64). Apart from controlling the
    /// length of the final `Hash`, this is also associated data, and changing it will result in a
    /// totally different hash.
    pub fn hash_length(&mut self, length: usize) -> &mut Self {
        assert!(
            1 <= length && length <= OUTBYTES,
            "Bad hash length: {}",
            length
        );
        self.hash_length = length as u8;
        self
    }

    /// Use a secret key, so that BLAKE2bp acts as a MAC. The maximum key length is `KEYBYTES`
    /// (64). An empty key is equivalent to having no key at all.
    pub fn key(&mut self, key: &[u8]) -> &mut Self {
        assert!(key.len() <= KEYBYTES, "Bad key length: {}", key.len());
        self.key_length = key.len() as u8;
        self.key = [0; KEYBYTES];
        self.key[..key.len()].copy_from_slice(key);
        self
    }
}

impl Default for Params {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Params {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Params {{ hash_length: {}, key_length: {} }}",
            self.hash_length,
            // NB: Don't print the key itself. Debug shouldn't leak secrets.
            self.key_length,
        )
    }
}

/// An incremental hasher for BLAKE2bp, just like the [`State`](../struct.State.html) type for
/// BLAKE2b.
///
/// # Example
///
/// ```
/// use blake2b_simd::blake2bp;
///
/// let mut state = blake2bp::State::new();
/// state.update(b"foo");
/// state.update(b"bar");
/// let hash = state.finalize();
///
/// let expected = "e654427b6ef02949471712263e59071abbb6aa94855674c1daeed6cfaf127c33\
///                 dfa3205f7f7f71e4f0673d25fa82a368488911f446bccd323af3ab03f53e56e5";
/// assert_eq!(expected, &hash.to_hex());
/// ```
#[derive(Clone)]
pub struct State {
    leaf_words: [[Word; 8]; DEGREE],
    root_words: [Word; 8],
    // Note that this buffer is twice as large as what compress4 needs. That guarantees that we
    // have enough input when we compress to know we don't need to finalize any of the leaves.
    buf: [u8; 2 * DEGREE * BLOCKBYTES],
    buf_len: u16,
    // Note that this is the *per-leaf* count.
    count: Count,
    hash_length: u8,
    implementation: Implementation,
    is_keyed: bool,
}

impl State {
    /// Equivalent to `State::default()` or `Params::default().to_state()`.
    pub fn new() -> Self {
        Self::with_params(&Params::default())
    }

    fn with_params(params: &Params) -> Self {
        let (leaf_words, root_words) = params.to_words();

        // If a key is set, initalize the buffer to contain the key bytes. Note
        // that only the leaves hash key bytes. The root doesn't, even though
        // the key length it still set in its parameters. Again this isn't
        // documented in the spec, but it matches the behavior of the reference
        // implementation:
        // https://github.com/BLAKE2/BLAKE2/blob/320c325437539ae91091ce62efec1913cd8093c2/ref/blake2bp-ref.c#L128
        // This particular behavior (though not the inner hash length behavior
        // above) is also corroborated by the official test vectors; see
        // tests/vector_tests.rs.
        let mut buf = [0; 2 * DEGREE * BLOCKBYTES];
        let mut buf_len = 0;
        if params.key_length > 0 {
            for i in 0..DEGREE {
                let keybytes = &params.key[..params.key_length as usize];
                buf[i * BLOCKBYTES..][..keybytes.len()].copy_from_slice(keybytes);
                buf_len = BLOCKBYTES * DEGREE;
            }
        }

        Self {
            leaf_words,
            root_words,
            buf,
            buf_len: buf_len as u16,
            count: 0, // count gets updated in self.compress()
            hash_length: params.hash_length,
            implementation: params.implementation,
            is_keyed: params.key_length > 0,
        }
    }

    fn fill_buf(&mut self, input: &mut &[u8]) {
        let take = cmp::min(self.buf.len() - self.buf_len as usize, input.len());
        self.buf[self.buf_len as usize..][..take].copy_from_slice(&input[..take]);
        self.buf_len += take as u16;
        *input = &input[take..];
    }

    fn compress_to_leaves(
        leaves: &mut [[Word; 8]; DEGREE],
        input: &[u8],
        count: &mut Count,
        implementation: Implementation,
    ) {
        // Input is assumed to be an even number of blocks for each leaf. Since
        // we're not finilizing, debug asserts will fire otherwise.
        let jobs = leaves.iter_mut().enumerate().map(|(i, words)| {
            Job {
                input: &input[i * BLOCKBYTES..],
                words,
                count: *count,
                last_node: LastNode::No, // irrelevant when not finalizing
            }
        });
        many::compress_many(jobs, implementation, Finalize::No, Stride::Parallel);
        // Note that count is the bytes input *per-leaf*.
        *count = count.wrapping_add((input.len() / DEGREE) as Count);
    }

    /// Add input to the hash. You can call `update` any number of times.
    pub fn update(&mut self, mut input: &[u8]) -> &mut Self {
        // If we have a partial buffer, try to complete it. If we complete it and there's more
        // input waiting, we need to compress to make more room. However, because we need to be
        // sure that *none* of the leaves would need to be finalized as part of this round of
        // compression, we need to buffer more than we would for BLAKE2b.
        if self.buf_len > 0 {
            self.fill_buf(&mut input);
            // The buffer is large enough for two compressions. If we've filled
            // the buffer and there's still more input coming, then we have to
            // do at least one compression. If there's enough input still
            // coming that all the leaves are guaranteed to get more, do both
            // compressions in the buffer. Otherwise, do just one and shift the
            // back half of the buffer to the front.
            if !input.is_empty() {
                if input.len() > (DEGREE - 1) * BLOCKBYTES {
                    // Enough input coming to do both compressions.
                    Self::compress_to_leaves(
                        &mut self.leaf_words,
                        &self.buf,
                        &mut self.count,
                        self.implementation,
                    );
                    self.buf_len = 0;
                } else {
                    // Only enough input coming for one compression.
                    Self::compress_to_leaves(
                        &mut self.leaf_words,
                        &self.buf[..DEGREE * BLOCKBYTES],
                        &mut self.count,
                        self.implementation,
                    );
                    self.buf_len = (DEGREE * BLOCKBYTES) as u16;
                    let (buf_front, buf_back) = self.buf.split_at_mut(DEGREE * BLOCKBYTES);
                    buf_front.copy_from_slice(buf_back);
                }
            }
        }

        // Now we directly compress as much input as possible, without copying
        // it into the buffer. We need to make sure we buffer at least one byte
        // for each of the leaves, so that we know we don't need to finalize
        // them.
        let needed_tail = (DEGREE - 1) * BLOCKBYTES + 1;
        let mut bulk_bytes = input.len().saturating_sub(needed_tail);
        bulk_bytes -= bulk_bytes % (DEGREE * BLOCKBYTES);
        if bulk_bytes > 0 {
            Self::compress_to_leaves(
                &mut self.leaf_words,
                &input[..bulk_bytes],
                &mut self.count,
                self.implementation,
            );
            input = &input[bulk_bytes..];
        }

        // Buffer any remaining input, to be either compressed or finalized in
        // a subsequent call.
        self.fill_buf(&mut input);
        debug_assert_eq!(0, input.len());
        self
    }

    /// Finalize the state and return a `Hash`. This method is idempotent, and calling it multiple
    /// times will give the same result. It's also possible to `update` with more input in between.
    pub fn finalize(&self) -> Hash {
        // Hash whatever's remaining in the buffer and finalize the leaves.
        let buf_len = self.buf_len as usize;
        let mut leaves_copy = self.leaf_words;
        let jobs = leaves_copy
            .iter_mut()
            .enumerate()
            .map(|(leaf_index, leaf_words)| {
                let input = &self.buf[cmp::min(leaf_index * BLOCKBYTES, buf_len)..buf_len];
                Job {
                    input,
                    words: leaf_words,
                    count: self.count,
                    last_node: if leaf_index == DEGREE - 1 {
                        LastNode::Yes
                    } else {
                        LastNode::No
                    },
                }
            });
        many::compress_many(jobs, self.implementation, Finalize::Yes, Stride::Parallel);

        // Concatenate each leaf into the root and hash that.
        let mut root_words_copy = self.root_words;
        finalize_root_words(
            &leaves_copy,
            &mut root_words_copy,
            self.hash_length,
            self.implementation,
        )
    }

    /// Return the total number of bytes input so far.
    ///
    /// Note that `count` doesn't include the bytes of the key block, if any.
    /// It's exactly the total number of input bytes fed to `update`.
    pub fn count(&self) -> Count {
        // Remember that self.count is *per-leaf*.
        let mut ret = self
            .count
            .wrapping_mul(DEGREE as Count)
            .wrapping_add(self.buf_len as Count);
        if self.is_keyed {
            ret -= (DEGREE * BLOCKBYTES) as Count;
        }
        ret
    }
}

#[cfg(feature = "std")]
impl std::io::Write for State {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.update(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "State {{ count: {}, hash_length: {} }}",
            self.count(),
            self.hash_length,
        )
    }
}

impl Default for State {
    fn default() -> Self {
        Self::with_params(&Params::default())
    }
}

// Compress each of the four finalized hashes into the root words as input,
// using two compressions. Note that even if a future version of this
// implementation supports the hash_length parameter and sets it as associated
// data for all nodes, this step must still use the untruncated output of each
// leaf. Note also that, as mentioned above, the root node doesn't hash any key
// bytes.
fn finalize_root_words(
    leaf_words: &[[Word; 8]; DEGREE],
    root_words: &mut [Word; 8],
    hash_length: u8,
    imp: Implementation,
) -> Hash {
    debug_assert_eq!(OUTBYTES, 8 * size_of::<Word>());
    let mut block = [0; DEGREE * OUTBYTES];
    for (word, chunk) in leaf_words
        .iter()
        .flat_map(|words| words.iter())
        .zip(block.chunks_exact_mut(size_of::<Word>()))
    {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    imp.compress1_loop(
        &block,
        root_words,
        0,
        LastNode::Yes,
        Finalize::Yes,
        Stride::Serial,
    );
    Hash {
        bytes: crate::state_words_to_bytes(&root_words),
        len: hash_length,
    }
}

pub(crate) fn force_portable(params: &mut Params) {
    params.implementation = Implementation::portable();
}

#[cfg(test)]
pub(crate) mod test {
    use super::*;
    use crate::paint_test_input;

    // This is a simple reference implementation without the complicated buffering or parameter
    // support of the real implementation. We need this because the official test vectors don't
    // include any inputs large enough to exercise all the branches in the buffering logic.
    fn blake2bp_reference(input: &[u8]) -> Hash {
        let mut leaves = arrayvec::ArrayVec::<[_; DEGREE]>::new();
        for leaf_index in 0..DEGREE {
            leaves.push(
                crate::Params::new()
                    .fanout(DEGREE as u8)
                    .max_depth(2)
                    .node_offset(leaf_index as u64)
                    .inner_hash_length(OUTBYTES)
                    .to_state(),
            );
        }
        leaves[DEGREE - 1].set_last_node(true);
        for (i, chunk) in input.chunks(BLOCKBYTES).enumerate() {
            leaves[i % DEGREE].update(chunk);
        }
        let mut root = crate::Params::new()
            .fanout(DEGREE as u8)
            .max_depth(2)
            .node_depth(1)
            .inner_hash_length(OUTBYTES)
            .last_node(true)
            .to_state();
        for leaf in &mut leaves {
            root.update(leaf.finalize().as_bytes());
        }
        root.finalize()
    }

    #[test]
    fn test_against_reference() {
        let mut buf = [0; 21 * BLOCKBYTES];
        paint_test_input(&mut buf);
        // - 8 blocks is just enought to fill the double buffer.
        // - 9 blocks triggers the "perform one compression on the double buffer" case.
        // - 11 blocks is the largest input where only one compression may be performed, on the
        //   first half of the buffer, because there's not enough input to avoid needing to
        //   finalize the second half.
        // - 12 blocks triggers the "perform both compressions in the double buffer" case.
        // - 15 blocks is the largest input where, after compressing 8 blocks from the buffer,
        //   there's not enough input to hash directly from memory.
        // - 16 blocks triggers "after emptying the buffer, hash directly from memory".
        for num_blocks in 0..=20 {
            for &extra in &[0, 1, BLOCKBYTES - 1] {
                for &portable in &[false, true] {
                    // eprintln!("\ncase -----");
                    // dbg!(num_blocks);
                    // dbg!(extra);
                    // dbg!(portable);

                    // First hash the input all at once, as a sanity check.
                    let mut params = Params::new();
                    if portable {
                        force_portable(&mut params);
                    }
                    let input = &buf[..num_blocks * BLOCKBYTES + extra];
                    let expected = blake2bp_reference(&input);
                    let mut state = params.to_state();
                    let found = state.update(input).finalize();
                    assert_eq!(expected, found);

                    // Then, do it again, but buffer 1 byte of input first. That causes the buffering
                    // branch to trigger.
                    let mut state = params.to_state();
                    let maybe_one = cmp::min(1, input.len());
                    state.update(&input[..maybe_one]);
                    assert_eq!(maybe_one as Count, state.count());
                    // Do a throwaway finalize here to check for idempotency.
                    state.finalize();
                    state.update(&input[maybe_one..]);
                    assert_eq!(input.len() as Count, state.count());
                    let found = state.finalize();
                    assert_eq!(expected, found);

                    // Finally, do it again with the all-at-once interface.
                    assert_eq!(expected, blake2bp(input));
                }
            }
        }
    }
}
