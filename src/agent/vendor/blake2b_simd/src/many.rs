//! Interfaces for hashing multiple inputs at once, using SIMD more
//! efficiently.
//!
//! The throughput of these interfaces is comparable to BLAKE2bp, about twice
//! the throughput of regular BLAKE2b when AVX2 is available.
//!
//! These interfaces can accept any number of inputs, and the implementation
//! does its best to parallelize them. In general, the more inputs you can pass
//! in at once the better. If you need to batch your inputs in smaller groups,
//! see the [`degree`](fn.degree.html) function for a good batch size.
//!
//! The implementation keeps working in parallel even when inputs are of
//! different lengths, by managing a working set of jobs whose input isn't yet
//! exhausted. However, if one or two inputs are much longer than the others,
//! and they're encountered only at the end, there might not be any remaining
//! work to parallelize them with. In this case, sorting the inputs
//! longest-first can improve parallelism.
//!
//! # Example
//!
//! ```
//! use blake2b_simd::{blake2b, State, many::update_many};
//!
//! let mut states = [
//!     State::new(),
//!     State::new(),
//!     State::new(),
//!     State::new(),
//! ];
//!
//! let inputs = [
//!     &b"foo"[..],
//!     &b"bar"[..],
//!     &b"baz"[..],
//!     &b"bing"[..],
//! ];
//!
//! update_many(states.iter_mut().zip(inputs.iter()));
//!
//! for (state, input) in states.iter_mut().zip(inputs.iter()) {
//!     assert_eq!(blake2b(input), state.finalize());
//! }
//! ```

use crate::guts::{self, Finalize, Implementation, Job, LastNode, Stride};
use crate::state_words_to_bytes;
use crate::Count;
use crate::Hash;
use crate::Params;
use crate::State;
use crate::Word;
use crate::BLOCKBYTES;
use arrayref::array_mut_ref;
use arrayvec::ArrayVec;
use core::fmt;

/// The largest possible value of [`degree`](fn.degree.html) on the target
/// platform.
///
/// Note that this constant reflects the parallelism degree supported by this
/// crate, so it will change over time as support is added or removed. For
/// example, when Rust stabilizes AVX-512 support and this crate adds an
/// AVX-512 implementation, this constant will double on x86 targets. If that
/// implementation is an optional feature (e.g. because it's nightly-only), the
/// value of this constant will depend on that optional feature also.
pub const MAX_DEGREE: usize = guts::MAX_DEGREE;

/// The parallelism degree of the implementation, detected at runtime. If you
/// hash your inputs in small batches, making the batch size a multiple of
/// `degree` will generally give good performance.
///
/// For example, an x86 processor that supports AVX2 can compute four BLAKE2b
/// hashes in parallel, so `degree` returns 4 on that machine. If you call
/// [`hash_many`] with only three inputs, that's not enough to use the AVX2
/// implementation, and your average throughput will be lower. Likewise if you
/// call it with five inputs of equal length, the first four will be hashed in
/// parallel with AVX2, but the last one will have to be hashed by itself, and
/// again your average throughput will be lower.
///
/// As noted in the module level docs, performance is more complicated if your
/// inputs are of different lengths. When parallelizing long and short inputs
/// together, the longer ones will have bytes left over, and the implementation
/// will try to parallelize those leftover bytes with subsequent inputs. The
/// more inputs available in that case, the more the implementation will be
/// able to parallelize.
///
/// If you need a constant batch size, for example to collect inputs in an
/// array, see [`MAX_DEGREE`].
///
/// [`hash_many`]: fn.hash_many.html
/// [`MAX_DEGREE`]: constant.MAX_DEGREE.html
pub fn degree() -> usize {
    guts::Implementation::detect().degree()
}

type JobsVec<'a, 'b> = ArrayVec<[Job<'a, 'b>; guts::MAX_DEGREE]>;

#[inline(always)]
fn fill_jobs_vec<'a, 'b>(
    jobs_iter: &mut impl Iterator<Item = Job<'a, 'b>>,
    vec: &mut JobsVec<'a, 'b>,
    target_len: usize,
) {
    while vec.len() < target_len {
        if let Some(job) = jobs_iter.next() {
            vec.push(job);
        } else {
            break;
        }
    }
}

#[inline(always)]
fn evict_finished<'a, 'b>(vec: &mut JobsVec<'a, 'b>, num_jobs: usize) {
    // Iterate backwards so that removal doesn't cause an out-of-bounds panic.
    for i in (0..num_jobs).rev() {
        // Note that is_empty() is only valid because we know all these jobs
        // have been run at least once. Otherwise we could confuse the empty
        // input for a finished job, which would be incorrect.
        //
        // Avoid a panic branch here in release mode.
        debug_assert!(vec.len() > i);
        if vec.len() > i && vec[i].input.is_empty() {
            // Note that calling pop_at() repeatedly has some overhead, because
            // later elements need to be shifted up. However, the JobsVec is
            // small, and this approach guarantees that jobs are encountered in
            // order.
            vec.pop_at(i);
        }
    }
}

pub(crate) fn compress_many<'a, 'b, I>(
    jobs: I,
    imp: Implementation,
    finalize: Finalize,
    stride: Stride,
) where
    I: IntoIterator<Item = Job<'a, 'b>>,
{
    // Fuse is important for correctness, since each of these blocks tries to
    // advance the iterator, even if a previous block emptied it.
    let mut jobs_iter = jobs.into_iter().fuse();
    let mut jobs_vec = JobsVec::new();

    if imp.degree() >= 4 {
        loop {
            fill_jobs_vec(&mut jobs_iter, &mut jobs_vec, 4);
            if jobs_vec.len() < 4 {
                break;
            }
            let jobs_array = array_mut_ref!(jobs_vec, 0, 4);
            imp.compress4_loop(jobs_array, finalize, stride);
            evict_finished(&mut jobs_vec, 4);
        }
    }

    if imp.degree() >= 2 {
        loop {
            fill_jobs_vec(&mut jobs_iter, &mut jobs_vec, 2);
            if jobs_vec.len() < 2 {
                break;
            }
            let jobs_array = array_mut_ref!(jobs_vec, 0, 2);
            imp.compress2_loop(jobs_array, finalize, stride);
            evict_finished(&mut jobs_vec, 2);
        }
    }

    for job in jobs_vec.into_iter().chain(jobs_iter) {
        let Job {
            input,
            words,
            count,
            last_node,
        } = job;
        imp.compress1_loop(input, words, count, last_node, finalize, stride);
    }
}

/// Update any number of `State` objects at once.
///
/// # Example
///
/// ```
/// use blake2b_simd::{blake2b, State, many::update_many};
///
/// let mut states = [
///     State::new(),
///     State::new(),
///     State::new(),
///     State::new(),
/// ];
///
/// let inputs = [
///     &b"foo"[..],
///     &b"bar"[..],
///     &b"baz"[..],
///     &b"bing"[..],
/// ];
///
/// update_many(states.iter_mut().zip(inputs.iter()));
///
/// for (state, input) in states.iter_mut().zip(inputs.iter()) {
///     assert_eq!(blake2b(input), state.finalize());
/// }
/// ```
pub fn update_many<'a, 'b, I, T>(pairs: I)
where
    I: IntoIterator<Item = (&'a mut State, &'b T)>,
    T: 'b + AsRef<[u8]> + ?Sized,
{
    // Get the guts::Implementation from the first state, if any.
    let mut peekable_pairs = pairs.into_iter().peekable();
    let implementation = if let Some((state, _)) = peekable_pairs.peek() {
        state.implementation
    } else {
        // No work items, just short circuit.
        return;
    };

    // Adapt the pairs iterator into a Jobs iterator, but skip over the Jobs
    // where there's not actually any work to do (e.g. because there's not much
    // input and it's all just going in the State buffer).
    let jobs = peekable_pairs.flat_map(|(state, input_t)| {
        let mut input = input_t.as_ref();
        // For each pair, if the State has some input in its buffer, try to
        // finish that buffer. If there wasn't enough input to do that --
        // or if the input was empty to begin with -- skip this pair.
        state.compress_buffer_if_possible(&mut input);
        if input.is_empty() {
            return None;
        }
        // Now we know the buffer is empty and there's more input. Make sure we
        // buffer the final block, because update() doesn't finalize.
        let mut last_block_start = input.len() - 1;
        last_block_start -= last_block_start % BLOCKBYTES;
        let (blocks, last_block) = input.split_at(last_block_start);
        state.buf[..last_block.len()].copy_from_slice(last_block);
        state.buflen = last_block.len() as u8;
        // Finally, if the full blocks slice is non-empty, prepare that job for
        // compression, and bump the State count.
        if blocks.is_empty() {
            None
        } else {
            let count = state.count;
            state.count = state.count.wrapping_add(blocks.len() as Count);
            Some(Job {
                input: blocks,
                words: &mut state.words,
                count,
                last_node: state.last_node,
            })
        }
    });

    // Run all the Jobs in the iterator.
    compress_many(jobs, implementation, Finalize::No, Stride::Serial);
}

/// A job for the [`hash_many`] function. After calling [`hash_many`] on a
/// collection of `HashManyJob` objects, you can call [`to_hash`] on each job
/// to get the result.
///
/// [`hash_many`]: fn.hash_many.html
/// [`to_hash`]: struct.HashManyJob.html#method.to_hash
#[derive(Clone)]
pub struct HashManyJob<'a> {
    words: [Word; 8],
    count: Count,
    last_node: LastNode,
    hash_length: u8,
    input: &'a [u8],
    finished: bool,
    implementation: guts::Implementation,
}

impl<'a> HashManyJob<'a> {
    /// Construct a new `HashManyJob` from a set of hashing parameters and an
    /// input.
    #[inline]
    pub fn new(params: &Params, input: &'a [u8]) -> Self {
        let mut words = params.to_words();
        let mut count = 0;
        let mut finished = false;
        // If we have key bytes, compress them into the state words. If there's
        // no additional input, this compression needs to finalize and set
        // finished=true.
        if params.key_length > 0 {
            let mut finalization = Finalize::No;
            if input.is_empty() {
                finalization = Finalize::Yes;
                finished = true;
            }
            params.implementation.compress1_loop(
                &params.key_block,
                &mut words,
                0,
                params.last_node,
                finalization,
                Stride::Serial,
            );
            count = BLOCKBYTES as Count;
        }
        Self {
            words,
            count,
            last_node: params.last_node,
            hash_length: params.hash_length,
            input,
            finished,
            implementation: params.implementation,
        }
    }

    /// Get the hash from a finished job. If you call this before calling
    /// [`hash_many`], it will panic in debug mode.
    ///
    /// [`hash_many`]: fn.hash_many.html
    #[inline]
    pub fn to_hash(&self) -> Hash {
        debug_assert!(self.finished, "job hasn't been run yet");
        Hash {
            bytes: state_words_to_bytes(&self.words),
            len: self.hash_length,
        }
    }
}

impl<'a> fmt::Debug for HashManyJob<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // NB: Don't print the words. Leaking them would allow length extension.
        write!(
            f,
            "HashManyJob {{ count: {}, hash_length: {}, last_node: {}, input_len: {} }}",
            self.count,
            self.hash_length,
            self.last_node.yes(),
            self.input.len(),
        )
    }
}

/// Hash any number of complete inputs all at once.
///
/// This is slightly more efficient than using `update_many` with `State`
/// objects, because it doesn't need to do any buffering.
///
/// Running `hash_many` on the same `HashManyJob` object more than once has no
/// effect.
///
/// # Example
///
/// ```
/// use blake2b_simd::{blake2b, Params, many::{HashManyJob, hash_many}};
///
/// let inputs = [
///     &b"foo"[..],
///     &b"bar"[..],
///     &b"baz"[..],
///     &b"bing"[..],
/// ];
///
/// let mut params = Params::new();
/// params.hash_length(16);
///
/// let mut jobs = [
///     HashManyJob::new(&params, inputs[0]),
///     HashManyJob::new(&params, inputs[1]),
///     HashManyJob::new(&params, inputs[2]),
///     HashManyJob::new(&params, inputs[3]),
/// ];
///
/// hash_many(jobs.iter_mut());
///
/// for (input, job) in inputs.iter().zip(jobs.iter()) {
///     let expected = params.hash(input);
///     assert_eq!(expected, job.to_hash());
/// }
/// ```
pub fn hash_many<'a, 'b, I>(hash_many_jobs: I)
where
    'b: 'a,
    I: IntoIterator<Item = &'a mut HashManyJob<'b>>,
{
    // Get the guts::Implementation from the first job, if any.
    let mut peekable_jobs = hash_many_jobs.into_iter().peekable();
    let implementation = if let Some(job) = peekable_jobs.peek() {
        job.implementation
    } else {
        // No work items, just short circuit.
        return;
    };

    // In the jobs iterator, skip HashManyJobs that have already been run. This
    // is less because we actually expect callers to call hash_many twice
    // (though they're allowed to if they want), and more because
    // HashManyJob::new might need to finalize if there are key bytes but no
    // input. Tying the job lifetime to the Params reference is an alternative,
    // but I've found it too constraining in practice. We could also put key
    // bytes in every HashManyJob, but that would add unnecessary storage and
    // zeroing for all callers.
    let unfinished_jobs = peekable_jobs.into_iter().filter(|j| !j.finished);
    let jobs = unfinished_jobs.map(|j| {
        j.finished = true;
        Job {
            input: j.input,
            words: &mut j.words,
            count: j.count,
            last_node: j.last_node,
        }
    });
    compress_many(jobs, implementation, Finalize::Yes, Stride::Serial);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::guts;
    use crate::paint_test_input;
    use crate::BLOCKBYTES;
    use arrayvec::ArrayVec;

    #[test]
    fn test_degree() {
        assert!(degree() <= MAX_DEGREE);

        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        #[cfg(feature = "std")]
        {
            if is_x86_feature_detected!("avx2") {
                assert!(degree() >= 4);
            }
            if is_x86_feature_detected!("sse4.1") {
                assert!(degree() >= 2);
            }
        }
    }

    #[test]
    fn test_hash_many() {
        // Use a length of inputs that will exercise all of the power-of-two loops.
        const LEN: usize = 2 * guts::MAX_DEGREE - 1;

        // Rerun LEN inputs LEN different times, with the empty input starting in a
        // different spot each time.
        let mut input = [0; LEN * BLOCKBYTES];
        paint_test_input(&mut input);
        for start_offset in 0..LEN {
            let mut inputs: [&[u8]; LEN] = [&[]; LEN];
            for i in 0..LEN {
                let chunks = (i + start_offset) % LEN;
                inputs[i] = &input[..chunks * BLOCKBYTES];
            }

            let mut params: ArrayVec<[Params; LEN]> = ArrayVec::new();
            for i in 0..LEN {
                let mut p = Params::new();
                p.node_offset(i as u64);
                if i % 2 == 1 {
                    p.last_node(true);
                    p.key(b"foo");
                }
                params.push(p);
            }

            let mut jobs: ArrayVec<[HashManyJob; LEN]> = ArrayVec::new();
            for i in 0..LEN {
                jobs.push(HashManyJob::new(&params[i], inputs[i]));
            }

            hash_many(&mut jobs);

            // Check the outputs.
            for i in 0..LEN {
                let expected = params[i].hash(inputs[i]);
                assert_eq!(expected, jobs[i].to_hash());
            }
        }
    }

    #[test]
    fn test_update_many() {
        // Use a length of inputs that will exercise all of the power-of-two loops.
        const LEN: usize = 2 * guts::MAX_DEGREE - 1;

        // Rerun LEN inputs LEN different times, with the empty input starting in a
        // different spot each time.
        let mut input = [0; LEN * BLOCKBYTES];
        paint_test_input(&mut input);
        for start_offset in 0..LEN {
            let mut inputs: [&[u8]; LEN] = [&[]; LEN];
            for i in 0..LEN {
                let chunks = (i + start_offset) % LEN;
                inputs[i] = &input[..chunks * BLOCKBYTES];
            }

            let mut params: ArrayVec<[Params; LEN]> = ArrayVec::new();
            for i in 0..LEN {
                let mut p = Params::new();
                p.node_offset(i as u64);
                if i % 2 == 1 {
                    p.last_node(true);
                    p.key(b"foo");
                }
                params.push(p);
            }

            let mut states: ArrayVec<[State; LEN]> = ArrayVec::new();
            for i in 0..LEN {
                states.push(params[i].to_state());
            }

            // Run each input twice through, to exercise buffering.
            update_many(states.iter_mut().zip(inputs.iter()));
            update_many(states.iter_mut().zip(inputs.iter()));

            // Check the outputs.
            for i in 0..LEN {
                let mut reference_state = params[i].to_state();
                // Again, run the input twice.
                reference_state.update(inputs[i]);
                reference_state.update(inputs[i]);
                assert_eq!(reference_state.finalize(), states[i].finalize());
                assert_eq!(2 * inputs[i].len() as Count, states[i].count());
            }
        }
    }
}
