//!32 bit version of xxhash algorithm
//!
//!Written using C implementation as reference.

use core::{ptr, slice};

use crate::xxh32_common::*;

#[inline(always)]
fn read_le_unaligned(data: *const u8) -> u32 {
    debug_assert!(!data.is_null());

    unsafe {
        ptr::read_unaligned(data as *const u32).to_le()
    }
}

#[inline(always)]
fn read_le_aligned(data: *const u8) -> u32 {
    debug_assert!(!data.is_null());

    unsafe {
        ptr::read(data as *const u32).to_le()
    }
}

#[inline(always)]
fn read_le_is_align(data: *const u8, is_aligned: bool) -> u32 {
    match is_aligned {
        true => read_le_aligned(data),
        false => read_le_unaligned(data)
    }
}

fn finalize(mut input: u32, mut data: &[u8], is_aligned: bool) -> u32 {
    while data.len() >= 4 {
        input = input.wrapping_add(
            read_le_is_align(data.as_ptr(), is_aligned).wrapping_mul(PRIME_3)
        );
        data = &data[4..];
        input = input.rotate_left(17).wrapping_mul(PRIME_4);
    }

    for byte in data.iter() {
        input = input.wrapping_add((*byte as u32).wrapping_mul(PRIME_5));
        input = input.rotate_left(11).wrapping_mul(PRIME_1);
    }

    avalanche(input)
}

///Returns hash for the provided input
pub fn xxh32(mut input: &[u8], seed: u32) -> u32 {
    let mut result = input.len() as u32;

    if input.len() >= CHUNK_SIZE {
        let mut v1 = seed.wrapping_add(PRIME_1).wrapping_add(PRIME_2);
        let mut v2 = seed.wrapping_add(PRIME_2);
        let mut v3 = seed;
        let mut v4 = seed.wrapping_sub(PRIME_1);

        loop {
            v1 = round(v1, read_le_unaligned(input.as_ptr()));
            input = &input[4..];
            v2 = round(v2, read_le_unaligned(input.as_ptr()));
            input = &input[4..];
            v3 = round(v3, read_le_unaligned(input.as_ptr()));
            input = &input[4..];
            v4 = round(v4, read_le_unaligned(input.as_ptr()));
            input = &input[4..];

            if input.len() < CHUNK_SIZE {
                break;
            }
        }

        result = result.wrapping_add(
            v1.rotate_left(1).wrapping_add(
                v2.rotate_left(7).wrapping_add(
                    v3.rotate_left(12).wrapping_add(
                        v4.rotate_left(18)
                    )
                )
            )
        );
    } else {
        result = result.wrapping_add(seed.wrapping_add(PRIME_5));
    }

    return finalize(result, input, false);
}

///XXH32 Streaming algorithm
pub struct Xxh32 {
    total_len: u32,
    is_large_len: bool,
    v1: u32,
    v2: u32,
    v3: u32,
    v4: u32,
    mem: [u32; 4],
    mem_size: u32,
}

impl Xxh32 {
    #[inline]
    ///Creates new hasher with specified seed.
    pub const fn new(seed: u32) -> Self {
        Self {
            total_len: 0,
            is_large_len: false,
            v1: seed.wrapping_add(PRIME_1).wrapping_add(PRIME_2),
            v2: seed.wrapping_add(PRIME_2),
            v3: seed,
            v4: seed.wrapping_sub(PRIME_1),
            mem: [0, 0, 0, 0],
            mem_size: 0,
        }
    }

    ///Hashes provided input.
    pub fn update(&mut self, mut input: &[u8]) {
        self.total_len = self.total_len.wrapping_add(input.len() as u32);
        self.is_large_len |= (input.len() as u32 >= CHUNK_SIZE as u32) | (self.total_len >= CHUNK_SIZE as u32);

        if (self.mem_size + input.len() as u32) < CHUNK_SIZE as u32 {
            unsafe {
                ptr::copy_nonoverlapping(input.as_ptr(), (self.mem.as_mut_ptr() as *mut u8).offset(self.mem_size as isize), input.len())
            }
            self.mem_size += input.len() as u32;
            return
        }

        if self.mem_size > 0 {
            //previous if can fail only when we do not have enough space in buffer for input.
            //hence fill_len >= input.len()
            let fill_len = CHUNK_SIZE - self.mem_size as usize;

            unsafe {
                ptr::copy_nonoverlapping(input.as_ptr(), (self.mem.as_mut_ptr() as *mut u8).offset(self.mem_size as isize), fill_len)
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
                self.v1 = round(self.v1, read_le_unaligned(input.as_ptr()));
                input = &input[4..];
                self.v2 = round(self.v2, read_le_unaligned(input.as_ptr()));
                input = &input[4..];
                self.v3 = round(self.v3, read_le_unaligned(input.as_ptr()));
                input = &input[4..];
                self.v4 = round(self.v4, read_le_unaligned(input.as_ptr()));
                input = &input[4..];

                if input.len() < CHUNK_SIZE {
                    break;
                }
            }
        }

        if input.len() > 0 {
            unsafe {
                ptr::copy_nonoverlapping(input.as_ptr(), self.mem.as_mut_ptr() as *mut u8, input.len())
            }
            self.mem_size = input.len() as u32;
        }
    }

    ///Finalize hashing.
    pub fn digest(&self) -> u32 {
        let mut result = self.total_len;

        if self.is_large_len {
            result = result.wrapping_add(
                self.v1.rotate_left(1).wrapping_add(
                    self.v2.rotate_left(7).wrapping_add(
                        self.v3.rotate_left(12).wrapping_add(
                            self.v4.rotate_left(18)
                        )
                    )
                )
            );
        } else {
            result = result.wrapping_add(self.v3.wrapping_add(PRIME_5));
        }

        let input = unsafe {
            slice::from_raw_parts(self.mem.as_ptr() as *const u8, self.mem_size as usize)
        };

        return finalize(result, input, true);
    }

    #[inline]
    ///Resets the state with specified seed.
    pub fn reset(&mut self, seed: u32) {
        self.total_len = 0;
        self.is_large_len = false;
        self.v1 = seed.wrapping_add(PRIME_1).wrapping_add(PRIME_2);
        self.v2 = seed.wrapping_add(PRIME_2);
        self.v3 = seed;
        self.v4 = seed.wrapping_sub(PRIME_1);
        self.mem_size = 0;
    }
}
