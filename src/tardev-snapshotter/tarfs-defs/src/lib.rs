use zerocopy::byteorder::{LE, U16, U32, U64};

/// Flags used in [`Inode::flags`].
pub mod inode_flags {
    /// Indicates that the inode is opaque.
    ///
    /// When set, inode will have the "trusted.overlay.opaque" set to "y" at runtime.
    pub const OPAQUE: u8 = 0x1;
}

/// An inode in the tarfs inode table.
#[derive(zerocopy::AsBytes, zerocopy::FromBytes, zerocopy::Unaligned)]
#[repr(C)]
pub struct Inode {
    /// The mode of the inode.
    ///
    /// The bottom 9 bits are the rwx bits for owner, group, all.
    ///
    /// The bits in the [`S_IFMT`] mask represent the file mode.
    pub mode: U16<LE>,

    /// Tarfs flags for the inode.
    ///
    /// Values are drawn from the [`inode_flags`] module.
    pub flags: u8,

    /// The bottom 4 bits represent the top 4 bits of mtime.
    pub hmtime: u8,

    /// The owner of the inode.
    pub owner: U32<LE>,

    /// The group of the inode.
    pub group: U32<LE>,

    /// The bottom 32 bits of mtime.
    pub lmtime: U32<LE>,

    /// Size of the contents of the inode.
    pub size: U64<LE>,

    /// Either the offset to the data, or the major and minor numbers of a device.
    ///
    /// For the latter, the 32 LSB are the minor, and the 32 MSB are the major numbers.
    pub offset: U64<LE>,
}

/// An entry in a tarfs directory entry table.
#[derive(zerocopy::AsBytes, zerocopy::FromBytes, zerocopy::Unaligned)]
#[repr(C)]
pub struct DirEntry {
    /// The inode number this entry refers to.
    pub ino: U64<LE>,

    /// The offset to the name of the entry.
    pub name_offset: U64<LE>,

    /// The length of the name of the entry.
    pub name_len: U64<LE>,

    /// The type of entry.
    pub etype: u8,

    /// Unused padding.
    pub _padding: [u8; 7],
}

/// The super-block of a tarfs instance.
#[derive(zerocopy::AsBytes, zerocopy::FromBytes, zerocopy::Unaligned)]
#[repr(C)]
pub struct SuperBlock {
    /// The offset to the beginning of the inode-table.
    pub inode_table_offset: U64<LE>,

    /// The number of inodes in the file system.
    pub inode_count: U64<LE>,
}

/// A mask to be applied to [`Inode::mode`] to extract the inode's type.
pub const S_IFMT: u16 = 0o0170000;

/// A socket.
pub const S_IFSOCK: u16 = 0o0140000;

/// A symbolic link.
pub const S_IFLNK: u16 = 0o0120000;

/// A regular file.
pub const S_IFREG: u16 = 0o0100000;

/// A block device.
pub const S_IFBLK: u16 = 0o0060000;

/// A directory.
pub const S_IFDIR: u16 = 0o0040000;

/// A character device.
pub const S_IFCHR: u16 = 0o0020000;

/// A (fifo) pipe.
pub const S_IFIFO: u16 = 0o0010000;

/// Unknown directory entry type.
pub const DT_UNKNOWN: u8 = 0;

/// A (fifo) pipe.
pub const DT_FIFO: u8 = 1;

/// A character device.
pub const DT_CHR: u8 = 2;

/// A directory.
pub const DT_DIR: u8 = 4;

/// A block device.
pub const DT_BLK: u8 = 6;

/// A regular file.
pub const DT_REG: u8 = 8;

/// A symbolic link.
pub const DT_LNK: u8 = 10;

/// A socket.
pub const DT_SOCK: u8 = 12;
