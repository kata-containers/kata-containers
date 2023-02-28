use generic_array::{typenum::Unsigned, GenericArray};
use sha2::{digest::OutputSizeUser, Digest};
use std::{fs::File, io, io::Seek, os::unix::fs::FileExt};
use zerocopy::byteorder::{LE, U32, U64};
use zerocopy::AsBytes;

#[derive(Default, zerocopy::AsBytes, zerocopy::FromBytes, zerocopy::Unaligned)]
#[repr(C)]
pub struct SuperBlock {
    pub data_block_size: U32<LE>,
    pub hash_block_size: U32<LE>,
    pub data_block_count: U64<LE>,
}

#[derive(Clone)]
struct Level {
    next_index: usize,
    file_offset: u64,
    data: Vec<u8>,
}

pub struct Verity<'a, T: Digest + Clone> {
    levels: Vec<Level>,
    seeded: T,
    data_block_size: usize,
    hash_block_size: usize,
    block_remaining_count: u64,
    writer: Option<&'a File>,
    super_block: SuperBlock,
}

impl<'a, T: Digest + Clone> Verity<'a, T> {
    const HASH_SIZE: usize = T::OutputSize::USIZE;

    /// Creates a new `Verity` instance.
    pub fn new(
        data_size: u64,
        data_block_size: usize,
        hash_block_size: usize,
        salt: &[u8],
        write: Option<(&'a File, u64)>,
    ) -> io::Result<Self> {
        let level_count = {
            let mut max_size = data_block_size as u64;
            let mut count = 0usize;

            while max_size < data_size {
                count += 1;
                max_size *= (hash_block_size / Self::HASH_SIZE) as u64;
            }
            count
        };

        let mut data = Vec::new();
        data.resize(hash_block_size, 0);

        let mut levels = Vec::new();
        levels.resize(
            level_count,
            Level {
                next_index: 0,
                file_offset: 0,
                data,
            },
        );

        let (writer, mut offset) = match write {
            Some((w, offset)) => (Some(w), offset),
            None => (None, 0),
        };

        for (i, l) in levels.iter_mut().enumerate() {
            let entry_size = (data_block_size as u64)
                * ((hash_block_size / Self::HASH_SIZE) as u64).pow(level_count as u32 - i as u32);
            let count = (data_size + entry_size - 1) / entry_size;
            l.file_offset = offset;
            offset += hash_block_size as u64 * count;
        }

        let block_count = data_size / (data_block_size as u64);
        Ok(Self {
            levels,
            seeded: T::new_with_prefix(salt),
            data_block_size,
            block_remaining_count: block_count,
            hash_block_size,
            writer,
            super_block: SuperBlock {
                data_block_size: (data_block_size as u32).into(),
                hash_block_size: (hash_block_size as u32).into(),
                data_block_count: block_count.into(),
            },
        })
    }

    /// Determines if more blocks are expected.
    ///
    /// This is based on file size specified when this instance was created.
    fn more_blocks(&self) -> bool {
        self.block_remaining_count > 0
    }

    /// Adds the given hash to the level.
    ///
    /// Returns `true` is the level is now full; `false` is there is still room for more hashes.
    fn add_hash(&mut self, l: usize, hash: &[u8]) -> bool {
        let level = &mut self.levels[l];
        level.data[level.next_index * Self::HASH_SIZE..][..Self::HASH_SIZE].copy_from_slice(hash);
        level.next_index += 1;
        level.next_index >= self.hash_block_size / Self::HASH_SIZE
    }

    /// Finalises the level despite potentially not having filled it.
    ///
    /// It zeroes out the remaining bytes of the level so that its hash can be calculated
    /// consistently.
    fn finalize_level(&mut self, l: usize) {
        let level = &mut self.levels[l];
        for b in &mut level.data[level.next_index * Self::HASH_SIZE..] {
            *b = 0;
        }
        level.next_index = 0;
    }

    fn uplevel(&mut self, l: usize) -> io::Result<bool> {
        self.finalize_level(l);
        if let Some(w) = self.writer {
            w.write_all_at(&self.levels[l].data, self.levels[l].file_offset)?;
        }
        self.levels[l].file_offset += self.hash_block_size as u64;
        let h = self.digest(&self.levels[l].data);
        Ok(self.add_hash(l - 1, h.as_slice()))
    }

    fn digest(&self, block: &[u8]) -> GenericArray<u8, T::OutputSize> {
        let mut hasher = self.seeded.clone();
        hasher.update(block);
        hasher.finalize()
    }

    fn add_block(&mut self, b: &[u8]) -> io::Result<()> {
        if self.block_remaining_count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected block",
            ));
        }

        self.block_remaining_count -= 1;

        let count = self.levels.len();
        let hash = self.digest(b);
        if self.add_hash(count - 1, hash.as_slice()) {
            // Go up the levels as far as it can.
            for l in (1..count).rev() {
                if !self.uplevel(l)? {
                    break;
                }
            }
        }
        Ok(())
    }

    fn finalize(mut self, write_superblock: bool) -> io::Result<GenericArray<u8, T::OutputSize>> {
        let len = self.levels.len();
        for mut l in (1..len).rev() {
            if self.levels[l].next_index != 0 {
                while l > 0 {
                    self.uplevel(l)?;
                    l -= 1;
                }
                break;
            }
        }

        self.finalize_level(0);
        if let Some(w) = self.writer {
            w.write_all_at(&self.levels[0].data, self.levels[0].file_offset)?;
            self.levels[0].file_offset += self.hash_block_size as u64;

            if write_superblock {
                w.write_all_at(
                    self.super_block.as_bytes(),
                    self.levels[len - 1].file_offset + 4096 - 512,
                )?;

                // TODO: Align to the hash_block_size...
                // Align to 4096 bytes.
                w.write_all_at(&[0u8], self.levels[len - 1].file_offset + 4095)?;
            }
        }

        Ok(self.digest(&self.levels[0].data))
    }
}

pub fn traverse_file<T: Digest + Clone>(
    reader: &File,
    mut read_offset: u64,
    write_superblock: bool,
    mut verity: Verity<'_, T>,
) -> io::Result<GenericArray<u8, T::OutputSize>> {
    let mut buf = Vec::new();
    buf.resize(verity.data_block_size, 0);
    while verity.more_blocks() {
        reader.read_exact_at(&mut buf, read_offset)?;
        verity.add_block(&buf)?;
        read_offset += verity.data_block_size as u64;
    }
    verity.finalize(write_superblock)
}

pub fn append_tree<T: Digest + Clone>(
    file: &mut File,
) -> io::Result<GenericArray<u8, T::OutputSize>> {
    let file_size = file.seek(io::SeekFrom::End(0))?;
    file.rewind()?;
    let mut salt = Vec::new();
    salt.resize(<T as OutputSizeUser>::OutputSize::USIZE, 0);
    let verity = Verity::<T>::new(file_size, 4096, 4096, &salt, Some((file, file_size)))?;
    traverse_file(file, 0, true, verity)
}
