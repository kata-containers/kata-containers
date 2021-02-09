// Copyright (c) 2017 Martijn Rijkeboer <mrr@sru-systems.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::block::Block;
use crate::common;
use crate::context::Context;
use crate::memory::Memory;
use crate::variant::Variant;
use crate::version::Version;
use blake2b_simd::Params;
#[cfg(feature = "crossbeam-utils")]
use crossbeam_utils::thread::scope;

/// Position of the block currently being operated on.
#[derive(Clone, Debug)]
struct Position {
    pass: u32,
    lane: u32,
    slice: u32,
    index: u32,
}

/// Initializes the memory.
pub fn initialize(context: &Context, memory: &mut Memory) {
    fill_first_blocks(context, memory, &mut h0(context));
}

/// Fills all the memory blocks.
pub fn fill_memory_blocks(context: &Context, memory: &mut Memory) {
    if context.config.uses_sequential() {
        fill_memory_blocks_st(context, memory);
    } else {
        fill_memory_blocks_mt(context, memory);
    }
}

/// Calculates the final hash and returns it.
pub fn finalize(context: &Context, memory: &Memory) -> Vec<u8> {
    let mut blockhash = memory[context.lane_length - 1].clone();
    for l in 1..context.config.lanes {
        let last_block_in_lane = l * context.lane_length + (context.lane_length - 1);
        blockhash ^= &memory[last_block_in_lane];
    }

    let mut hash = vec![0u8; context.config.hash_length as usize];
    hprime(hash.as_mut_slice(), blockhash.as_u8());
    hash
}

fn blake2b(out: &mut [u8], input: &[&[u8]]) {
    let mut blake = Params::new().hash_length(out.len()).to_state();
    for slice in input {
        blake.update(slice);
    }
    out.copy_from_slice(blake.finalize().as_bytes());
}

fn f_bla_mka(x: u64, y: u64) -> u64 {
    let m = 0xFFFF_FFFFu64;
    let xy = (x & m) * (y & m);
    x.wrapping_add(y.wrapping_add(xy.wrapping_add(xy)))
}

fn fill_block(prev_block: &Block, ref_block: &Block, next_block: &mut Block, with_xor: bool) {
    let mut block_r = ref_block.clone();
    block_r ^= prev_block;
    let mut block_tmp = block_r.clone();

    // Now block_r = ref_block + prev_block and block_tmp = ref_block + prev_block
    if with_xor {
        // Saving the next block contents for XOR over
        block_tmp ^= next_block;
        // Now block_r = ref_block + prev_block and
        // block_tmp = ref_block + prev_block + next_block
    }

    // Apply Blake2 on columns of 64-bit words: (0,1,...,15) , then
    // (16,17,..31)... finally (112,113,...127)
    for i in 0..8 {
        let mut v0 = block_r[16 * i];
        let mut v1 = block_r[16 * i + 1];
        let mut v2 = block_r[16 * i + 2];
        let mut v3 = block_r[16 * i + 3];
        let mut v4 = block_r[16 * i + 4];
        let mut v5 = block_r[16 * i + 5];
        let mut v6 = block_r[16 * i + 6];
        let mut v7 = block_r[16 * i + 7];
        let mut v8 = block_r[16 * i + 8];
        let mut v9 = block_r[16 * i + 9];
        let mut v10 = block_r[16 * i + 10];
        let mut v11 = block_r[16 * i + 11];
        let mut v12 = block_r[16 * i + 12];
        let mut v13 = block_r[16 * i + 13];
        let mut v14 = block_r[16 * i + 14];
        let mut v15 = block_r[16 * i + 15];

        p(
            &mut v0, &mut v1, &mut v2, &mut v3, &mut v4, &mut v5, &mut v6, &mut v7, &mut v8,
            &mut v9, &mut v10, &mut v11, &mut v12, &mut v13, &mut v14, &mut v15,
        );

        block_r[16 * i] = v0;
        block_r[16 * i + 1] = v1;
        block_r[16 * i + 2] = v2;
        block_r[16 * i + 3] = v3;
        block_r[16 * i + 4] = v4;
        block_r[16 * i + 5] = v5;
        block_r[16 * i + 6] = v6;
        block_r[16 * i + 7] = v7;
        block_r[16 * i + 8] = v8;
        block_r[16 * i + 9] = v9;
        block_r[16 * i + 10] = v10;
        block_r[16 * i + 11] = v11;
        block_r[16 * i + 12] = v12;
        block_r[16 * i + 13] = v13;
        block_r[16 * i + 14] = v14;
        block_r[16 * i + 15] = v15;
    }

    // Apply Blake2 on rows of 64-bit words: (0,1,16,17,...112,113), then
    // (2,3,18,19,...,114,115).. finally (14,15,30,31,...,126,127)
    for i in 0..8 {
        let mut v0 = block_r[2 * i];
        let mut v1 = block_r[2 * i + 1];
        let mut v2 = block_r[2 * i + 16];
        let mut v3 = block_r[2 * i + 17];
        let mut v4 = block_r[2 * i + 32];
        let mut v5 = block_r[2 * i + 33];
        let mut v6 = block_r[2 * i + 48];
        let mut v7 = block_r[2 * i + 49];
        let mut v8 = block_r[2 * i + 64];
        let mut v9 = block_r[2 * i + 65];
        let mut v10 = block_r[2 * i + 80];
        let mut v11 = block_r[2 * i + 81];
        let mut v12 = block_r[2 * i + 96];
        let mut v13 = block_r[2 * i + 97];
        let mut v14 = block_r[2 * i + 112];
        let mut v15 = block_r[2 * i + 113];

        p(
            &mut v0, &mut v1, &mut v2, &mut v3, &mut v4, &mut v5, &mut v6, &mut v7, &mut v8,
            &mut v9, &mut v10, &mut v11, &mut v12, &mut v13, &mut v14, &mut v15,
        );

        block_r[2 * i] = v0;
        block_r[2 * i + 1] = v1;
        block_r[2 * i + 16] = v2;
        block_r[2 * i + 17] = v3;
        block_r[2 * i + 32] = v4;
        block_r[2 * i + 33] = v5;
        block_r[2 * i + 48] = v6;
        block_r[2 * i + 49] = v7;
        block_r[2 * i + 64] = v8;
        block_r[2 * i + 65] = v9;
        block_r[2 * i + 80] = v10;
        block_r[2 * i + 81] = v11;
        block_r[2 * i + 96] = v12;
        block_r[2 * i + 97] = v13;
        block_r[2 * i + 112] = v14;
        block_r[2 * i + 113] = v15;
    }

    block_tmp.copy_to(next_block);
    *next_block ^= &block_r;
}

fn fill_first_blocks(context: &Context, memory: &mut Memory, h0: &mut [u8]) {
    for lane in 0..context.config.lanes {
        let start = common::PREHASH_DIGEST_LENGTH;
        // H'(H0||0||i)
        h0[start..(start + 4)].clone_from_slice(&u32::to_le_bytes(0));
        h0[(start + 4)..(start + 8)].clone_from_slice(&u32::to_le_bytes(lane));
        hprime(memory[(lane, 0)].as_u8_mut(), &h0);

        // H'(H0||1||i)
        h0[start..(start + 4)].clone_from_slice(&u32::to_le_bytes(1));
        hprime(memory[(lane, 1)].as_u8_mut(), &h0);
    }
}

#[cfg(feature = "crossbeam-utils")]
fn fill_memory_blocks_mt(context: &Context, memory: &mut Memory) {
    for p in 0..context.config.time_cost {
        for s in 0..common::SYNC_POINTS {
            let _ = scope(|scoped| {
                for (l, mem) in (0..context.config.lanes).zip(memory.as_lanes_mut()) {
                    let position = Position {
                        pass: p,
                        lane: l,
                        slice: s,
                        index: 0,
                    };
                    scoped.spawn(move |_| {
                        fill_segment(context, &position, mem);
                    });
                }
            });
        }
    }
}

#[cfg(not(feature = "crossbeam-utils"))]
fn fill_memory_blocks_mt(_: &Context, _: &mut Memory) {
    unimplemented!()
}

fn fill_memory_blocks_st(context: &Context, memory: &mut Memory) {
    for p in 0..context.config.time_cost {
        for s in 0..common::SYNC_POINTS {
            for l in 0..context.config.lanes {
                let position = Position {
                    pass: p,
                    lane: l,
                    slice: s,
                    index: 0,
                };
                fill_segment(context, &position, memory);
            }
        }
    }
}

fn fill_segment(context: &Context, position: &Position, memory: &mut Memory) {
    let mut position = position.clone();
    let data_independent_addressing = (context.config.variant == Variant::Argon2i)
        || (context.config.variant == Variant::Argon2id && position.pass == 0)
            && (position.slice < (common::SYNC_POINTS / 2));
    let zero_block = Block::zero();
    let mut input_block = Block::zero();
    let mut address_block = Block::zero();

    if data_independent_addressing {
        input_block[0] = position.pass as u64;
        input_block[1] = position.lane as u64;
        input_block[2] = position.slice as u64;
        input_block[3] = context.memory_blocks as u64;
        input_block[4] = context.config.time_cost as u64;
        input_block[5] = context.config.variant.as_u64();
    }

    let mut starting_index = 0u32;

    if position.pass == 0 && position.slice == 0 {
        starting_index = 2;

        // Don't forget to generate the first block of addresses:
        if data_independent_addressing {
            next_addresses(&mut address_block, &mut input_block, &zero_block);
        }
    }

    let mut curr_offset = (position.lane * context.lane_length)
        + (position.slice * context.segment_length)
        + starting_index;

    let mut prev_offset = if curr_offset % context.lane_length == 0 {
        // Last block in this lane
        curr_offset + context.lane_length - 1
    } else {
        curr_offset - 1
    };

    let mut pseudo_rand;
    for i in starting_index..context.segment_length {
        // 1.1 Rotating prev_offset if needed
        if curr_offset % context.lane_length == 1 {
            prev_offset = curr_offset - 1;
        }

        // 1.2 Computing the index of the reference block
        // 1.2.1 Taking pseudo-random value from the previous block
        if data_independent_addressing {
            if i % common::ADDRESSES_IN_BLOCK == 0 {
                next_addresses(&mut address_block, &mut input_block, &zero_block);
            }
            pseudo_rand = address_block[(i % common::ADDRESSES_IN_BLOCK) as usize];
        } else {
            pseudo_rand = memory[(prev_offset)][0];
        }

        // 1.2.2 Computing the lane of the reference block
        // If (position.pass == 0) && (position.slice == 0): can not reference other lanes yet
        let ref_lane = if (position.pass == 0) && (position.slice == 0) {
            position.lane as u64
        } else {
            (pseudo_rand >> 32) % context.config.lanes as u64
        };

        // 1.2.3 Computing the number of possible reference block within the lane.
        position.index = i;
        let pseudo_rand_u32 = (pseudo_rand & 0xFFFF_FFFF) as u32;
        let same_lane = ref_lane == (position.lane as u64);
        let ref_index = index_alpha(context, &position, pseudo_rand_u32, same_lane);

        // 2 Creating a new block
        let index = context.lane_length as u64 * ref_lane + ref_index as u64;
        let mut curr_block = memory[curr_offset].clone();
        {
            let prev_block = &memory[prev_offset];
            let ref_block = &memory[index];
            if context.config.version == Version::Version10 || position.pass == 0 {
                fill_block(prev_block, ref_block, &mut curr_block, false);
            } else {
                fill_block(prev_block, ref_block, &mut curr_block, true);
            }
        }

        memory[curr_offset] = curr_block;
        curr_offset += 1;
        prev_offset += 1;
    }
}

fn g(a: &mut u64, b: &mut u64, c: &mut u64, d: &mut u64) {
    *a = f_bla_mka(*a, *b);
    *d = rotr64(*d ^ *a, 32);
    *c = f_bla_mka(*c, *d);
    *b = rotr64(*b ^ *c, 24);
    *a = f_bla_mka(*a, *b);
    *d = rotr64(*d ^ *a, 16);
    *c = f_bla_mka(*c, *d);
    *b = rotr64(*b ^ *c, 63);
}

fn h0(context: &Context) -> [u8; common::PREHASH_SEED_LENGTH] {
    let input = [
        &u32::to_le_bytes(context.config.lanes),
        &u32::to_le_bytes(context.config.hash_length),
        &u32::to_le_bytes(context.config.mem_cost),
        &u32::to_le_bytes(context.config.time_cost),
        &u32::to_le_bytes(context.config.version.as_u32()),
        &u32::to_le_bytes(context.config.variant.as_u32()),
        &len_as_32le(context.pwd),
        context.pwd,
        &len_as_32le(context.salt),
        context.salt,
        &len_as_32le(context.config.secret),
        context.config.secret,
        &len_as_32le(context.config.ad),
        context.config.ad,
    ];
    let mut out = [0u8; common::PREHASH_SEED_LENGTH];
    blake2b(&mut out[0..common::PREHASH_DIGEST_LENGTH], &input);
    out
}

fn hprime(out: &mut [u8], input: &[u8]) {
    let out_len = out.len();
    if out_len <= common::BLAKE2B_OUT_LENGTH {
        blake2b(out, &[&u32::to_le_bytes(out_len as u32), input]);
    } else {
        let ai_len = 32;
        let mut out_buffer = [0u8; common::BLAKE2B_OUT_LENGTH];
        let mut in_buffer = [0u8; common::BLAKE2B_OUT_LENGTH];
        blake2b(&mut out_buffer, &[&u32::to_le_bytes(out_len as u32), input]);
        out[0..ai_len].clone_from_slice(&out_buffer[0..ai_len]);
        let mut out_pos = ai_len;
        let mut to_produce = out_len - ai_len;

        while to_produce > common::BLAKE2B_OUT_LENGTH {
            in_buffer.clone_from_slice(&out_buffer);
            blake2b(&mut out_buffer, &[&in_buffer]);
            out[out_pos..out_pos + ai_len].clone_from_slice(&out_buffer[0..ai_len]);
            out_pos += ai_len;
            to_produce -= ai_len;
        }
        blake2b(&mut out[out_pos..out_len], &[&out_buffer]);
    }
}

fn index_alpha(context: &Context, position: &Position, pseudo_rand: u32, same_lane: bool) -> u32 {
    // Pass 0:
    // - This lane: all already finished segments plus already constructed blocks in this segment
    // - Other lanes: all already finished segments
    // Pass 1+:
    // - This lane: (SYNC_POINTS - 1) last segments plus already constructed blocks in this segment
    // - Other lanes : (SYNC_POINTS - 1) last segments
    let reference_area_size: u32 = if position.pass == 0 {
        // First pass
        if position.slice == 0 {
            // First slice
            position.index - 1
        } else if same_lane {
            // The same lane => add current segment
            position.slice * context.segment_length + position.index - 1
        } else if position.index == 0 {
            position.slice * context.segment_length - 1
        } else {
            position.slice * context.segment_length
        }
    } else {
        // Second pass
        if same_lane {
            context.lane_length - context.segment_length + position.index - 1
        } else if position.index == 0 {
            context.lane_length - context.segment_length - 1
        } else {
            context.lane_length - context.segment_length
        }
    };
    let reference_area_size = reference_area_size as u64;
    let mut relative_position = pseudo_rand as u64;
    relative_position = (relative_position * relative_position) >> 32;
    relative_position = reference_area_size - 1 - ((reference_area_size * relative_position) >> 32);

    // 1.2.5 Computing starting position
    let start_position: u32 = if position.pass != 0 {
        if position.slice == common::SYNC_POINTS - 1 {
            0u32
        } else {
            (position.slice + 1) * context.segment_length
        }
    } else {
        0u32
    };
    let start_position = start_position as u64;

    // 1.2.6. Computing absolute position
    ((start_position + relative_position) % context.lane_length as u64) as u32
}

fn len_as_32le(slice: &[u8]) -> [u8; 4] {
    u32::to_le_bytes(slice.len() as u32)
}

fn next_addresses(address_block: &mut Block, input_block: &mut Block, zero_block: &Block) {
    input_block[6] += 1;
    fill_block(zero_block, input_block, address_block, false);
    fill_block(zero_block, &address_block.clone(), address_block, false);
}

fn p(
    v0: &mut u64,
    v1: &mut u64,
    v2: &mut u64,
    v3: &mut u64,
    v4: &mut u64,
    v5: &mut u64,
    v6: &mut u64,
    v7: &mut u64,
    v8: &mut u64,
    v9: &mut u64,
    v10: &mut u64,
    v11: &mut u64,
    v12: &mut u64,
    v13: &mut u64,
    v14: &mut u64,
    v15: &mut u64,
) {
    g(v0, v4, v8, v12);
    g(v1, v5, v9, v13);
    g(v2, v6, v10, v14);
    g(v3, v7, v11, v15);
    g(v0, v5, v10, v15);
    g(v1, v6, v11, v12);
    g(v2, v7, v8, v13);
    g(v3, v4, v9, v14);
}

fn rotr64(w: u64, c: u32) -> u64 {
    (w >> c) | (w << (64 - c))
}
