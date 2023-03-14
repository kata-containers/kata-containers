// Temporarily disable
/*
#[macro_use]
extern crate assert_matches;

use std::sync::Arc;

use rafs::fs::RafsConfig;
use rafs::metadata::RafsSuper;
use rafs::mock::{MockChunkInfo, MockInode, MockSuperBlock, CHUNK_SIZE};

#[test]
fn test_user_io_amplification_lack_chunks_small_expected() {
    let mut rafs_config = RafsConfig::new();
    rafs_config.mode = "cached".to_string();
    let mut super_sb = RafsSuper::new(&rafs_config).unwrap();
    let mut rafs_super_block = MockSuperBlock::new();

    // (1)file offset +
    // (2)compress offset + (3)compress size +
    // (4)decompress offset + (5)decompress size
    // not-trailing chunks have size of 200
    let ck1 = Arc::new(MockChunkInfo::mock(0, 700, 80, 900, CHUNK_SIZE));
    let ck2 = Arc::new(MockChunkInfo::mock(
        CHUNK_SIZE as u64,
        780,
        110,
        CHUNK_SIZE as u64,
        112,
    ));
    let chunks = vec![ck1.clone(), ck2];

    rafs_super_block.inodes.insert(
        1,
        Arc::new(MockInode::mock(1, CHUNK_SIZE as u64 + 112, chunks)),
    );

    super_sb.superblock = Arc::new(rafs_super_block);
    let inode_1 = super_sb.get_inode(1, false).unwrap();

    let desc = super_sb.carry_more_until(inode_1.as_ref(), 20, ck1.as_ref(), 60);
    assert_matches!(desc, Ok(None));

    let desc = super_sb.carry_more_until(inode_1.as_ref(), 20, ck1.as_ref(), 61);
    assert_matches!(desc, Ok(None));

    let desc = super_sb.carry_more_until(inode_1.as_ref(), 20, ck1.as_ref(), 60 + 110);
    assert_matches!(desc, Ok(None));

    let desc = super_sb.carry_more_until(inode_1.as_ref(), 20, ck1.as_ref(), CHUNK_SIZE as u64 + 1);
    assert_matches!(desc.unwrap().unwrap().bi_vec.len(), 1);

    let desc =
        super_sb.carry_more_until(inode_1.as_ref(), 20, ck1.as_ref(), CHUNK_SIZE as u64 * 10);
    assert_matches!(desc.unwrap().unwrap().bi_vec.len(), 1);
}

#[test]
fn test_user_io_amplification_lack_chunks_normal_expected() {
    let mut rafs_config = RafsConfig::new();
    rafs_config.mode = "cached".to_string();
    let mut super_sb = RafsSuper::new(&rafs_config).unwrap();
    let mut rafs_super_block = MockSuperBlock::new();

    // (1)file offset +
    // (2)compress offset + (3)compress size +
    // (4)decompress offset + (5)decompress size
    // not-trailing chunks have size of 200
    let ck1 = Arc::new(MockChunkInfo::mock(0, 700, 80, 900, 100));
    let ck2 = Arc::new(MockChunkInfo::mock(100, 780, 110, 1000, 300));
    let chunks = vec![ck1.clone(), ck2];

    rafs_super_block
        .inodes
        .insert(1, Arc::new(MockInode::mock(1, 400, chunks)));

    super_sb.superblock = Arc::new(rafs_super_block);
    let inode_1 = super_sb.get_inode(1, false).unwrap();

    let desc = super_sb.carry_more_until(inode_1.as_ref(), 81, ck1.as_ref(), 60);

    assert_matches!(desc, Ok(None));
}
#[test]
fn test_user_io_amplification_large_boundary() {
    let mut rafs_config = RafsConfig::new();
    rafs_config.mode = "cached".to_string();
    let mut super_sb = RafsSuper::new(&rafs_config).unwrap();
    let mut rafs_super_block = MockSuperBlock::new();

    // (1)file offset +
    // (2)compress offset + (3)compress size +
    // (4)decompress offset + (5)decompress size
    // not-trailing chunks have size of 200
    let ck1 = Arc::new(MockChunkInfo::mock(0, 700, 80, 900, CHUNK_SIZE));
    let ck2 = Arc::new(MockChunkInfo::mock(CHUNK_SIZE as u64, 780, 110, 1000, 120));
    let tail_ck = ck2.clone();
    let chunks = vec![ck1, ck2];

    rafs_super_block.inodes.insert(
        1,
        Arc::new(MockInode::mock(1, CHUNK_SIZE as u64 + 120, chunks)),
    );

    // Next file, not chunk continuous
    let discontinuous_blob_offset = 780 + 110;
    let ck1 = Arc::new(MockChunkInfo::mock(
        0,
        discontinuous_blob_offset,
        100,
        900,
        CHUNK_SIZE,
    ));
    let ck2 = Arc::new(MockChunkInfo::mock(
        CHUNK_SIZE as u64,
        discontinuous_blob_offset + 100,
        110,
        CHUNK_SIZE as u64,
        80,
    ));
    let chunks = vec![ck1, ck2];
    rafs_super_block.inodes.insert(
        2,
        Arc::new(MockInode::mock(2, CHUNK_SIZE as u64 + 80, chunks)),
    );

    super_sb.superblock = Arc::new(rafs_super_block);
    let inode_1 = super_sb.get_inode(1, false).unwrap();

    let desc = super_sb.carry_more_until(inode_1.as_ref(), 10000, tail_ck.as_ref(), 201);

    assert_matches!(desc, Ok(Some(_)));
    let appending = desc.unwrap().unwrap();
    assert_eq!(
        appending.bi_vec[0].chunkinfo.compress_offset(),
        discontinuous_blob_offset
    );
    assert_eq!(appending.bi_vec.len(), 1);

    let desc = super_sb.carry_more_until(inode_1.as_ref(), 10000, tail_ck.as_ref(), 100 + 1);

    assert_matches!(desc, Ok(Some(_)));
    let appending = desc.unwrap().unwrap();
    assert_eq!(
        appending.bi_vec[0].chunkinfo.compress_offset(),
        discontinuous_blob_offset
    );
    assert_eq!(appending.bi_vec.len(), 1);

    let desc = super_sb.carry_more_until(inode_1.as_ref(), 10000, tail_ck.as_ref(), 110 + 100 + 1);

    // 60 is smaller than real chunk
    // assert_matches!(desc, Ok(None));

    assert_matches!(desc, Ok(Some(_)));
    let appending = desc.unwrap().unwrap();
    assert_eq!(
        appending.bi_vec[0].chunkinfo.compress_offset(),
        discontinuous_blob_offset
    );
    assert_eq!(
        appending.bi_vec[1].chunkinfo.compress_offset(),
        discontinuous_blob_offset + 100
    );
    assert_eq!(appending.bi_vec.len(), 2);

    // 60 is smaller than real chunk
    // assert_matches!(desc, Ok(None));
    let desc = super_sb.carry_more_until(inode_1.as_ref(), 10000, tail_ck.as_ref(), 60);
    assert_matches!(desc, Ok(None));
}
#[test]
fn test_user_io_amplification_sparse_inodes() {
    let mut rafs_config = RafsConfig::new();
    rafs_config.mode = "cached".to_string();
    let mut super_sb = RafsSuper::new(&rafs_config).unwrap();
    let mut rafs_super_block = MockSuperBlock::new();

    // (1)file offset +
    // (2)compress offset + (3)compress size +
    // (4)decompress offset + (5)decompress size
    // not-trailing chunks have size of 200
    let ck1 = Arc::new(MockChunkInfo::mock(0, 700, 80, 900, CHUNK_SIZE));
    let ck2 = Arc::new(MockChunkInfo::mock(CHUNK_SIZE as u64, 780, 110, 1100, 100));
    let chunks = vec![ck1.clone(), ck2];

    let tail_ck = ck1;

    rafs_super_block.inodes.insert(
        1,
        Arc::new(MockInode::mock(1, CHUNK_SIZE as u64 + 100, chunks)),
    );

    // Next file, not chunk continuous
    let discontinuous_blob_offset = 780 + 110 + 140;
    let ck1 = Arc::new(MockChunkInfo::mock(
        0,
        discontinuous_blob_offset,
        100,
        900,
        CHUNK_SIZE,
    ));
    let ck2 = Arc::new(MockChunkInfo::mock(
        CHUNK_SIZE as u64,
        discontinuous_blob_offset + 100,
        110,
        CHUNK_SIZE as u64,
        80,
    ));
    let chunks = vec![ck1, ck2];
    rafs_super_block.inodes.insert(
        2,
        Arc::new(MockInode::mock(2, CHUNK_SIZE as u64 + 80, chunks)),
    );

    super_sb.superblock = Arc::new(rafs_super_block);
    let inode_1 = super_sb.get_inode(1, false).unwrap();

    let desc = super_sb.carry_more_until(inode_1.as_ref(), 20, tail_ck.as_ref(), (0 - 1) as u64);

    assert_matches!(desc, Ok(Some(_)));

    let appending = desc.unwrap().unwrap();
    assert_eq!(appending.bi_vec.len(), 1);
    assert_eq!(appending.bi_vec[0].chunkinfo.compress_offset(), 780);
}

#[test]
fn test_user_io_amplification_2_inodes_4_chunks_3_amplified() {
    let mut rafs_config = RafsConfig::new();
    rafs_config.mode = "cached".to_string();
    let mut super_sb = RafsSuper::new(&rafs_config).unwrap();
    let mut rafs_super_block = MockSuperBlock::new();

    // (1)file offset +
    // (2)compress offset + (3)compress size +
    // (4)decompress offset + (5)decompress size
    // not-trailing chunks have size of 200
    let ck1 = Arc::new(MockChunkInfo::mock(0, 700, 80, 900, CHUNK_SIZE));
    let ck2 = Arc::new(MockChunkInfo::mock(CHUNK_SIZE as u64, 780, 110, 1100, 100));
    let chunks = vec![ck1.clone(), ck2];

    let tail_ck = ck1;

    rafs_super_block.inodes.insert(
        1,
        Arc::new(MockInode::mock(1, CHUNK_SIZE as u64 + 100, chunks)),
    );

    // Next file
    let discontinuous_blob_offset = 780 + 110;
    let ck1 = Arc::new(MockChunkInfo::mock(
        0,
        discontinuous_blob_offset,
        100,
        900,
        CHUNK_SIZE,
    ));
    let ck2 = Arc::new(MockChunkInfo::mock(
        CHUNK_SIZE as u64,
        discontinuous_blob_offset + 100,
        110,
        CHUNK_SIZE as u64,
        80,
    ));
    let chunks = vec![ck1, ck2];
    rafs_super_block.inodes.insert(
        2,
        Arc::new(MockInode::mock(2, CHUNK_SIZE as u64 + 80, chunks)),
    );

    super_sb.superblock = Arc::new(rafs_super_block);
    let inode_1 = super_sb.get_inode(1, false).unwrap();

    let desc = super_sb.carry_more_until(inode_1.as_ref(), 20, tail_ck.as_ref(), (0 - 1) as u64);

    assert_matches!(desc, Ok(Some(_)));

    let appending = desc.unwrap().unwrap();
    assert_eq!(appending.bi_vec.len(), 3);
    assert_eq!(
        appending.bi_vec[2].chunkinfo.compress_offset(),
        discontinuous_blob_offset + 100
    );
}

#[test]
fn test_user_io_amplification_huge_expected() {
    let mut rafs_config = RafsConfig::new();
    rafs_config.mode = "cached".to_string();
    let mut super_sb = RafsSuper::new(&rafs_config).unwrap();
    let mut rafs_super_block = MockSuperBlock::new();

    // (1)file offset +
    // (2)compress offset + (3)compress size +
    // (4)decompress offset + (5)decompress size
    // not-trailing chunks have size of 200
    let ck1 = Arc::new(MockChunkInfo::mock(0, 700, 80, 900, CHUNK_SIZE));
    let ck2 = Arc::new(MockChunkInfo::mock(CHUNK_SIZE as u64, 780, 110, 1100, 100));
    let chunks = vec![ck1.clone(), ck2];

    // Only a file resided
    rafs_super_block
        .inodes
        .insert(1, Arc::new(MockInode::mock(1, 300, chunks)));

    super_sb.superblock = Arc::new(rafs_super_block);
    let inode_1 = super_sb.get_inode(1, false).unwrap();

    // File size is 400 bytes, first chunk is 80 bytes, should amplify by next chunk
    let desc = super_sb.carry_more_until(inode_1.as_ref(), 81, ck1.as_ref(), (0 - 1) as u64);

    if let Ok(Some(d)) = desc {
        assert_eq!(d.bi_vec.len(), 1);
        assert_eq!(d.bi_vec[0].chunkinfo.compress_offset(), 780);
    } else {
        panic!();
    }
}
 */
