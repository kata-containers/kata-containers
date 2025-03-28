# ext4
This file describes the layout on disk of ext4. It is a living document and probably will be deleted rather than committed to git.

The primary reference document is [here](https://ext4.wiki.kernel.org/index.php/Ext4_Disk_Layout).

Also useful are:

* https://blogs.oracle.com/linux/post/understanding-ext4-disk-layout-part-2
* https://www.sans.org/blog/understanding-ext4-part-6-directories/ - blog series
* https://digital-forensics.sans.org/blog/2017/06/07/understanding-ext4-part-6-directories
* https://metebalci.com/blog/a-minimum-complete-tutorial-of-linux-ext4-file-system/

## Concepts

* Sector: a section of 512 bytes
* Block: a contiguous group of sectors. Block size usually is either 4K (4096 bytes) or 1K (1024 bytes), i.e. 8 sectors or 2 sectors. Block size minimum is 1KB (2 sectors), max is 64KB (128 sectors). Each block is associated with exactly one file. A file may contain more than one block - e.g. if a file is larger than the size of a single block - but each block belongs to exactly one file.
* inode: metadata about a file or directory. Each inode contains metadata about exactly one file. The number of inodes in a system is identical to the number of blocks for 32-bit, or far fewer for 64-bit.
* Block group: a contiguous group of blocks. Each block group is (`8*block_size_in_bytes`) blocks. So if block size is 4K, or 4096 bytes, then a block group is `8*4096` = 32,768 blocks, each of size 4096 bytes, for a block group of 128MB. If block size is 1K, a block group is 8192 blocks, or 8MB.
* 64-bit feature: ext4 filesystems normally uses 32-bit, which means the maximum blocks per filesystem is 2^32. If the 64-bit feature is enabled, then the maximum blocks per filesystem is 2^64.
* Superblock: A block that contains information about the entire filesystem. Exists in block group 0 and sometimes is backed up to other block groups. The superblock contains information about the filesystem as a whole: inode size, block size, last mount time, etc.
* Block Group Descriptor: Block Group Descriptors contain information about each block group: start block, end block, inodes, etc. One Descriptor per Group. But it is stored next to the Superblock (and backups), not with each Group.
* Extent: an extent is a contiguous group of blocks. Extents are used to store files. Extents are mapped beginning with the inode, and provide the way of getting from an inode to the blocks that contain the file's data.


### Block Group

Each block group is built in the following order. There is a distinction between Group 0 - the first one
in the filesystem - and all others.

Block groups come in one of several types. It isn't necessary to list all of them here. The key elements are as follows.

Block 0:

1. Padding: 1024 bytes, used for boot sector

Block 0 (above 1024 bytes, if blocksize >1024) or Block 1; all backup blocks:

2. Superblock: One block
3. Group Descriptors: Many blocks
4. Reserved GDT Blocks: Many blocks, reserved in case we need to expand to more Group Descriptors in the future

All blocks:

5. Data block bitmap: 1 block. One bit per block in the block group. Set to 1 if a data block is in use, 0 if not.
6. inode bitmap: 1 block. One bit per inode in the block group. Set to 1 if an inode is in use, 0 if not.
7. inode table: many blocks. Calculated by `(inodes_per_group)*(size_of_inode)`. Remember that `inodes_per_group` = `blocks_per_group` = `8*block_size_in_bytes`. The original `size_of_inode` in ext2 was 128 bytes. In ext4 it uses 156 bytes, but is stored in 256 bytes of space, so `inode_size_in_bytes` = 256 bytes.
8. Data blocks: all of the rest of the blocks in the block group

The variant on the above is with Flexible Block Groups. If flexbg is enabled, then block groups are grouped together, normally
groups of 16 (but the actual number is in the superblock). The data block bitmap, inode bitmap and inode table are
in the first block group for each flexible block group.

This means you can have all sorts of combinations:

* block that is both first in a block group (contains block bitmap, inode bitmap, inode table) and superblock/backup (contains superblock, GDT, reserved GDT blocks)
* block that is first in a block group (block bitmap, inode bitmap, inode table) but not first in a block group or Flex BG
* block that is superblock/backup (superblock, GDT, reserved GDT blocks) but not first in a block group or Flex BG
* neither of the above (contains just data blocks)

Summary: block bitmap, inode bitmap and inode table are in the first block in a blockgroup or Flex BG, which is a consistent
number. Superblock backups are in specific blocks, calculated by being a block number that is a power of 3, 5 or 7.

## How to

Different actions. These all will be replaced by actual code. Things we need to be able to do:

* walk the tree to a particular directory or file
* inode to data blocks
* read directory entries
* create a new directory entry
* read contents of a file
* write contents to a file

### Walk the Tree

In order to get to any particular file or directory in the ext4 filesystem, you need to "walk the tree".
For example, say you want to read the contents of directory `/usr/local/bin/`.

1. Find the inode of the root directory in the inode table. This **always** is inode 2.
1. Read inode of the root directory to get the data blocks that contain the contents of the root directory. See [inode to data blocks](#inode-to-data-blocks).
1. Read the directory entries in the data blocks to get the names of the files and directories in root. This can be linear or hash.
   * linear: read sequentially until you find the one whose name matches the desired subdirectory, for example `usr`
   * hash: hash the name and use that to get the correct location
1. Using the matched directory entry, get the inode number for that subdirectory.
1. Use the superblock to read how many inodes are in each block group, e.g. 8144
1. Calculate which block group contains the inode you are looking for. Using the above example, 0-8143 are in group 0, 8144-16287 are in group 1, etc.
1. Read the inode of that subdirectory in the inode table of the given block group to get the data blocks that contain the contents of that directory.
1. Repeat until you have read the data blocks for the desired entry.

### Inode to Data Blocks

Start with the inode

1. Read the inode
1. Read the `i_block` value, 60 bytes at location 0x28 (= 40)
1. The first 12 bytes are an extent header:
   * magic number 0xf30a (little endian) - 2 bytes
   * number of entries following the header - 2 bytes - in the inode, always 1, 2, 3, or 4
   * maximum number of entries that could follow the header - 2 bytes - in the inode, always 4
   * depth of this node in the extent tree, where 0 = leaf, parent to that is 1, etc. - 2 bytes
   * generation (unused) - 4 bytes
1. Read the entries that follow.

If the data inside the inode is a leaf node (header depth = 0), then the entries will be leaf entries of 12 bytes:

* first block in the file that this extent covers - 4 bytes
* number of blocks in this extent - 2 bytes - If the value of this field is <= 32768, the extent is initialized. If the value of the field is > 32768, the extent is uninitialized and the actual extent length is ee_len - 32768. Therefore, the maximum length of a initialized extent is 32768 blocks, and the maximum length of an uninitialized extent is 32767.
* upper 16 bits of the block location - 2 bytes
* lower 32 bits of the block location - 4 bytes

For example, if a file has 1,000 blocks, and a particular extent entry points to blocks 100-299 of the file, and it starts
at filesystem block 10000, then the entry will be:

* 100 (4 bytes)
* 200 (2 bytes) - is this correct? This would indicate uninitialized
* 0 (2 bytes)
* 10000 (4 bytes)

If the data inside the inode is an internal node (header depth > 0), then the entries will be internal entries of 12 bytes:

* first file block that this extent and all its children cover - 4 bytes
* lower 32 bits of the block number os the extent node on the next lower level - 4 bytes
* upper 16 bits of the block number of the extent node on the next lower level - 2 bytes
* unused - 2 bytes

For example, if a file has 10,000 blocks, covered in 15 extents, then there will be 15 level 0 extents, and 1 level 1 extent,
and the 15 extents are stored in filesystem block 20000.

The lower level 0 extent will look like our leaf node example above.
The upper level 1 extent will look like:

* 0 (4 bytes) - because this starts from file block 0
* 20000 (4 bytes) - the block number of the extent node on the next lower level
* 0 (2 bytes) - because lower 4 bytes were enough to cover

You can find all of the blocks simply by looking at the root of the extent tree in the inode.

* If the extents for the file are 4 or fewer, then the extent tree is stored in the inode itself.
* If the extents for the file are more than 4, but enough to fit the extents in 1-4 blocks, then:
  * level 0 extents are stored in a single separate block
  * level 1 extents are stored in the inode, with up to 4 entries pointing to the level 0 extents blocks
* If the extents for the file are more than fit in 4 blocks, then:
  * level 0 extents are stored in as many blocks as needed
  * level 1 extents are stored in other blocks pointing to level 0 extent blocks
  * level 2 extents - up to 4 - are stored in the inode

Each of these is repeated upwards. The maximum at the top of the tree is 4, the maximum in each block is `(blocksize-12)/12`. 
Because:

- each block of extent nodes needs a header of 12 bytes
- each extent node is 12 bytes

### Read Directory Entries
To read directory entries

1. Walk the tree until you find the inode for the directory you want.
2. Read the data blocks pointed to by that inode, see [inode to data blocks](#inode-to-data-blocks).
3. Interpret the data blocks.

The directory itself is just a single "file". It has an inode that indicates the file "length", which is the number of bytes that the listing takes up.

There are two types of directories: Classic and Hash Tree. Classic are just linear, unsorted, unordered lists of files. They work fine for shorter lists, but large directories can be slow to traverse if they grow too large. Once the contents of the directory "file" will be larger than a single block, ext4 switches it to a Hash Tree Directory Entry.

Which directory type it is - classical linear or hash tree - does not affect the inode, for which it is just a file, but the contents of the directory entry "file". You can tell if it is linear or hash tree by checking the inode flag `EXT4_INDEX_FL`. If it is set (i.e. `& 0x1000`), then it is a hash tree.

#### Classic Directory Entry
Each directory entry is at most 263 bytes long. They are arranged in sequential order in the file. The contents are:

* first four bytes are a `uint32` giving the inode number
* next 2 bytes give the length of the directory entry (max 263)
* next 1 byte gives the length of the file name (which could be calculated from the directory entry length...)
* next 1 byte gives type: unknown, file, directory, char device, block device, FIFO, socket, symlink
* next (up to 255) bytes contain chars with the file or directory name

The above is for the second version of ext4 directory entry (`ext4_dir_entry_2`). The slightly older version (`ext4_dir_entry`) is similar, except it does not give the file type, which in any case is in the inode. Instead it uses 2 bytes for the file name length.

#### Hash Tree Directory Entry
Entries in the block are structured as follows:

* `.` and `..` are the first two entries, and are classic `ext4_dir_entry_2`
* Look in byte `0x1c` to find the hash algorithm
* take the desired file/subdirectory name (just the `basename`) and hash it, see [Calculating the hash value][Calculating the hash value]
* look in the root directory entry in the hashmap to find the relative block number. Note that the block number is relative to the block in the directory, not the filesystem or block group.
* Next step depends on the hash tree depth:
    * Depth = 0: read directory entry from the given block.
    * Depth > 0: use the block as another lookup table, repeating the steps above, until we come to the depth.
* Once we have the final leaf block given by the hash table, we just read the block sequentially; it will be full of classical directory entries linearly.

When reading the hashmap, it may not match precisely. Instead, it will fit within a range. The hashmap is sorted by `>=` to `<`. So if the table has entries as follows:

| Hash   | Block |
| -------|-------|
| 0      | 1     |
| 100    | 25    |
| 300    | 16    |

Then:

* all hash values from `0`-`99` will be in block `1`
* all hash values from `100-299` will be in block `25`
* all hash values from `300` to infinite will be in block `16`

##### Calculating the hash value

The hashing uses one of several algorithms. Most commonly, it is Half MD4.

MD4 gives a digest length of 128 bits = 16 bytes.

The "half md4" algorithm is given by the transformation code
[here](https://elixir.bootlin.com/linux/v4.6/source/lib/halfmd4.c#L26). The result
of it is 4 bytes. Those 4 bytes are the input to the hash.

### Create a Directory Entry

To create a directory, you need to go through the following steps:

1. "Walk the tree" to find the parent directory. E.g. if you are creating `/usr/local/foo`, then you need to walk the tree to get to the directory "file" for `/usr/local`. If the parent directory is just the root `/`, e.g. you are creating `/foo`, then you use the root directory, whose inode always is `2`.
2. Determine if the parent directory is classical linear or hash tree, by checking the flag `EXT4_INDEX_FL` in the parent directory's inode.
   * if hash:
     1. find a block in the "directory" file with space to add a linear entry
     1. create and add the entry
     1. calculate the hash of the filename
     1. add the `hash:block_number` entry into the tree
     1. rebalance if needed
   * if linear, create the entry:
     * if adding one will not exceed the size for linear, write it and done
     * if adding one will exceed the size for linear, convert to hash, then write it

#### Hash Tree

1. Calculate the hash of the new directory entry name
2. Determine which block in the parent directory "file" the new entry should live, based on the hash table.
3. Find the block.
4. Add a classical linear entry at the end of it.
5. Update the inode for the parent directory with the new file size.

If there is no room at the end of the block, you need to rebalance the hash tree. See below.

#### Classical Linear

1. Find the last block in the parent directory "file"
   * if there is no room for another entry, extend the file size by another block, and update the inode for the file with the block map
2. Add a classical linear directory entry at the end of it.
3. Update the inode for the parent directory with the new file size, if any. E.g. if the entry fit within padding, there is no change in size.

If this entry will cause the directory "file" to extend beyond a single block, convert to a hash tree. See below.

### Rebalance Hash Tree

Rebalancing the hash tree is rebalancing a btree, where the keys are the hash values.
You only ever need to rebalance when you add or remove an entry.

#### Adding an entry

When adding an entry, you only ever need to rebalance the node to which you add it, and parents up to the root.

1. Calculate the hash of the entry
1. Determine the leaf node into which it should go
1. If the leaf node has less than the maximum number of elements, add it and done
1. If the lead node has the maximum number of elements:
   1. Add the new node in the right place
   1. Find the median
   1. Move the median up to the parent node
   1. If necessary, rebalance the parent node

#### Removing an entry

When removing an entry, you only ever need to rebalance the node from which you remove it, and parents up to the root.

1. Calculate the hash of the entry
1. Determine the leaf node in which it exists
1. If the leaf node has less than the maximum number of elements, add it and done
1. If the lead node has the maximum number of elements:
   1. Add the new node in the right place
   1. Find the median
   1. Move the median up to the parent node
   1. If necessary, rebalance the parent node

### Convert Classical Linear Directory Entries to Hash Tree

The conversion usually happens when a single entry will exceed the capacity of a single block.

1. Switch the flag in the inode to hash-tree
1. Calculate the hash of each entry
1. Create 2 new blocks:
   * 1 for the bottom half of the entries
   * 1 for the top half of the entries
1. Move the bottom half of the entries into the bottom block
1. Move the top half of the entries into the top block
1. Zero out the current single file block, which previously had the classic linear directory entries
1. Write the header into the tree block, with the 0-hash-value pointing to the bottom block
1. Write one entry after the header, for the lowest hash value of the upper block, pointing to the upper block

### Read File Contents

1. Walk the tree until you find the inode for the file you want.
1. Find the data blocks for that inode, see [inode to data blocks](#inode-to-data-blocks).
1. Interpret the data blocks.

### Create File

1. Walk the tree until you find the inode for the parent directory.
1. Find a free inode using the inode bitmap.
1. Find a free block using the block bitmap.
1. Create the inode for the new file in the inode table. Be sure to update all the dependencies:
   * inode bitmap
   * inode table
   * inode count in the block group table
   * inode count in the superblock
1. Reserve a data block for the new file in the block group table. Be sure to update all the dependencies:
   * block bitmap
   * block count in the block group table
   * block count in the superblock
1. Create the file entry in the parent directory. Depends on if this is classic linear directory or hash tree directory. Note that if it is classic linear, calculate the new size before writing the entry. If it is bigger than a single block, convert to hash tree. TODO: is this the right boundary, single block?
   * Classic linear directory:
     1. Find the last block in the parent directory "file"
     1. Add a classical linear directory entry at the end of it
     1. Update the inode for the parent directory with the new file size
   * Hash tree directory:
     1. Calculate the hash of the new directory entry name
     1. Determine which block in the parent directory "file" the new entry should live, based on the hash table
     1. Find the block
     1. Add a classical linear entry at the end of it
     1. Update the inode for the parent directory with the new file size


### Write File Contents

1. Walk the tree until you find the inode for the file you want.
1. Find the data blocks for that inode, see [inode to data blocks](#inode-to-data-blocks).
1. Write the data to the data blocks.
1. If the data written exceeds the end of the last block, reserve a new block, update the inode extent tree, and write the data to the new block.
1. Update the inode with the filesize
1. Update the block group table with the used blocks
1. Update the superblock with the used blocks
