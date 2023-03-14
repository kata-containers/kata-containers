//!64 bit version of xxhash algorithm
//!
//!Written using C implementation as reference.

use core::{ptr, slice};

use crate::xxh64_common::*;

#[inline(always)]
fn read_32le_unaligned(data: *const u8) -> u32 {
    debug_assert!(!data.is_null());

    unsafe {
        ptr::read_unaligned(data as *const u32).to_le()
    }
}

#[inline(always)]
fn read_32le_aligned(data: *const u8) -> u32 {
    debug_assert!(!data.is_null());

    unsafe {
        ptr::read(data as *const u32).to_le()
    }
}

#[inline(always)]
fn read_32le_is_align(data: *const u8, is_aligned: bool) -> u32 {
    match is_aligned {
        true => read_32le_aligned(data),
        false => read_32le_unaligned(data),
    }
}

#[inline(always)]
fn read_64le_unaligned(data: *const u8) -> u64 {
    debug_assert!(!data.is_null());

    unsafe {
        ptr::read_unaligned(data as *const u64).to_le()
    }
}

#[inline(always)]
fn read_64le_aligned(data: *const u8) -> u64 {
    debug_assert!(!data.is_null());

    unsafe {
        ptr::read(data as *const u64).to_le()
    }
}

#[inline(always)]
fn read_64le_is_align(data: *const u8, is_aligned: bool) -> u64 {
    match is_aligned {
        true => read_64le_aligned(data),
        false => read_64le_unaligned(data),
    }
}

fn finalize(mut input: u64, mut data: &[u8], is_aligned: bool) -> u64 {
    while data.len() >= 8 {
        input ^= round(0, read_64le_is_align(data.as_ptr(), is_aligned));
        data = &data[8..];
        input = input.rotate_left(27).wrapping_mul(PRIME_1).wrapping_add(PRIME_4)
    }

    if data.len() >= 4 {
        input ^= (read_32le_is_align(data.as_ptr(), is_aligned) as u64).wrapping_mul(PRIME_1);
        data = &data[4..];
        input = input.rotate_left(23).wrapping_mul(PRIME_2).wrapping_add(PRIME_3);
    }

    for byte in data.iter() {
        input ^= (*byte as u64).wrapping_mul(PRIME_5);
        input = input.rotate_left(11).wrapping_mul(PRIME_1);
    }

    avalanche(input)
}

///Returns hash for the provided input.
pub fn xxh64(mut input: &[u8], seed: u64) -> u64 {
    let input_len = input.len() as u64;
    let mut result;

    if input.len() >= CHUNK_SIZE {
        let mut v1 = seed.wrapping_add(PRIME_1).wrapping_add(PRIME_2);
        let mut v2 = seed.wrapping_add(PRIME_2);
        let mut v3 = seed;
        let mut v4 = seed.wrapping_sub(PRIME_1);

        loop {
            v1 = round(v1, read_64le_unaligned(input.as_ptr()));
            input = &input[8..];
            v2 = round(v2, read_64le_unaligned(input.as_ptr()));
            input = &input[8..];
            v3 = round(v3, read_64le_unaligned(input.as_ptr()));
            input = &input[8..];
            v4 = round(v4, read_64le_unaligned(input.as_ptr()));
            input = &input[8..];

            if input.len() < CHUNK_SIZE {
                break;
            }
        }

        result = v1.rotate_left(1).wrapping_add(v2.rotate_left(7))
                                  .wrapping_add(v3.rotate_left(12))
                                  .wrapping_add(v4.rotate_left(18));

        result = merge_round(result, v1);
        result = merge_round(result, v2);
        result = merge_round(result, v3);
        result = merge_round(result, v4);
    } else {
        result = seed.wrapping_add(PRIME_5)
    }

    result = result.wrapping_add(input_len);

    finalize(result, input, false)
}

///XXH64 Streaming algorithm
pub struct Xxh64 {
    total_len: u64,
    v1: u64,
    v2: u64,
    v3: u64,
    v4: u64,
    mem: [u64; 4],
    mem_size: u64,
}

impl Xxh64 {
    #[inline]
    ///Creates new state with provided seed.
    pub const fn new(seed: u64) -> Self {
        Self {
            total_len: 0,
            v1: seed.wrapping_add(PRIME_1).wrapping_add(PRIME_2),
            v2: seed.wrapping_add(PRIME_2),
            v3: seed,
            v4: seed.wrapping_sub(PRIME_1),
            mem: [0, 0, 0, 0],
            mem_size: 0,
        }
    }

    ///Adds chunk of data to hash.
    pub fn update(&mut self, mut input: &[u8]) {
        self.total_len = self.total_len.wrapping_add(input.len() as u64);

        if (self.mem_size as usize + input.len()) < CHUNK_SIZE {
            unsafe {
                ptr::copy_nonoverlapping(input.as_ptr(), (self.mem.as_mut_ptr() as *mut u8).add(self.mem_size as usize), input.len())
            }
            self.mem_size += input.len() as u64;
            return
        }

        if self.mem_size > 0 {
            //previous if can fail only when we do not have enough space in buffer for input.
            //hence fill_len >= input.len()
            let fill_len = CHUNK_SIZE - self.mem_size as usize;

            unsafe {
                ptr::copy_nonoverlapping(input.as_ptr(), (self.mem.as_mut_ptr() as *mut u8).add(self.mem_size as usize), fill_len)
            }

            self.v1 = round(self.v1, self.mem[0].to_le());
            self.v2 = round(self.v2, self.mem[1].to_le());
            self.v3 = round(self.v3, self.mem[2].to_le());
            self.v4 = round(self.v4, self.mem[3].to_le());

            input = &input[fill_len..];
            self.mem_size = 0;
        }

        if input.len() >= CHUNK_SIZE {
            //In general this loop is not that long running on small input
            //So it is questionable whether we want to allocate local vars here.
            //Streaming version is likely to be used with relatively small chunks anyway.
            loop {
                self.v1 = round(self.v1, read_64le_unaligned(input.as_ptr()));
                input = &input[8..];
                self.v2 = round(self.v2, read_64le_unaligned(input.as_ptr()));
                input = &input[8..];
                self.v3 = round(self.v3, read_64le_unaligned(input.as_ptr()));
                input = &input[8..];
                self.v4 = round(self.v4, read_64le_unaligned(input.as_ptr()));
                input = &input[8..];

                if input.len() < CHUNK_SIZE {
                    break;
                }
            }
        }

        if input.len() > 0 {
            unsafe {
                ptr::copy_nonoverlapping(input.as_ptr(), self.mem.as_mut_ptr() as *mut u8, input.len())
            }
            self.mem_size = input.len() as u64;
        }
    }

    ///Finalize hashing.
    pub fn digest(&self) -> u64 {
        let mut result;

        if self.total_len >= CHUNK_SIZE as u64 {
            result = self.v1.rotate_left(1).wrapping_add(self.v2.rotate_left(7))
                                           .wrapping_add(self.v3.rotate_left(12))
                                           .wrapping_add(self.v4.rotate_left(18));

            result = merge_round(result, self.v1);
            result = merge_round(result, self.v2);
            result = merge_round(result, self.v3);
            result = merge_round(result, self.v4);
        } else {
            result = self.v3.wrapping_add(PRIME_5)
        }

        result = result.wrapping_add(self.total_len);

        let input = unsafe {
            slice::from_raw_parts(self.mem.as_ptr() as *const u8, self.mem_size as usize)
        };

        finalize(result, input, true)
    }

    #[inline]
    ///Resets state with provided seed.
    pub fn reset(&mut self, seed: u64) {
        self.total_len = 0;
        self.v1 = seed.wrapping_add(PRIME_1).wrapping_add(PRIME_2);
        self.v2 = seed.wrapping_add(PRIME_2);
        self.v3 = seed;
        self.v4 = seed.wrapping_sub(PRIME_1);
        self.mem_size = 0;
    }
}

impl core::hash::Hasher for Xxh64 {
    #[inline(always)]
    fn finish(&self) -> u64 {
        self.digest()
    }

    #[inline(always)]
    fn write(&mut self, input: &[u8]) {
        self.update(input)
    }
}

impl Default for Xxh64 {
    #[inline(always)]
    fn default() -> Self {
        Xxh64Builder::new(0).build()
    }
}

#[derive(Clone, Copy, Default)]
///Hash builder for `Xxh64`
pub struct Xxh64Builder {
    seed: u64
}

impl Xxh64Builder {
    #[inline(always)]
    ///Creates builder with provided `seed`
    pub const fn new(seed: u64) -> Self {
        Self {
            seed
        }
    }

    #[inline(always)]
    ///Creates hasher.
    pub const fn build(self) -> Xxh64 {
        Xxh64::new(self.seed)
    }
}

impl core::hash::BuildHasher for Xxh64Builder {
    type Hasher = Xxh64;

    #[inline(always)]
    fn build_hasher(&self) -> Self::Hasher {
        self.build()
    }
}
