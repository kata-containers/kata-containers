use fuser::{FileType, Request};
use libc::{makedev, EINVAL, ENODATA, ENOENT};
use std::io::{self, Error, ErrorKind};
use std::time::{Duration, UNIX_EPOCH};
use std::{ffi::OsStr, mem::size_of, os::unix::ffi::OsStrExt};
use tarfs_defs::*;
use zerocopy::FromBytes;

pub struct Tar {
    inode_table_offset: u64,
    inode_count: u64,
    data: memmap::Mmap,
}

const TARFS_BSIZE: u64 = 4096;

impl Tar {
    pub fn new(data: memmap::Mmap, last_offset: u64) -> io::Result<Self> {
        if last_offset > data.len() as u64 {
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                "last_offset beyond end of file",
            ));
        }

        if last_offset < 512 {
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                "last_offset too small",
            ));
        }

        // TODO: Validate that offsets are and that inode count is also ok.
        let sb = SuperBlock::read_from_prefix(&data[(last_offset - 512) as usize..]).unwrap();
        Ok(Self {
            inode_table_offset: sb.inode_table_offset.into(),
            inode_count: sb.inode_count.into(),
            data,
        })
    }

    fn inode(&self, ino: u64) -> Result<Inode, i32> {
        if ino < 1 || ino > self.inode_count {
            return Err(ENOENT);
        }

        // TODO: Remove this unwrap and check we're within range.
        Ok(Inode::read_from_prefix(
            &self.data
                [(self.inode_table_offset + (ino - 1) * size_of::<Inode>() as u64) as usize..],
        )
        .unwrap())
    }

    fn attr(&self, ino: u64) -> Result<fuser::FileAttr, i32> {
        let inode = self.inode(ino)?;
        let kind = match u16::from(inode.mode) & S_IFMT {
            S_IFIFO => FileType::NamedPipe,
            S_IFCHR => FileType::CharDevice,
            S_IFDIR => FileType::Directory,
            S_IFBLK => FileType::BlockDevice,
            S_IFREG => FileType::RegularFile,
            S_IFLNK => FileType::Symlink,
            S_IFSOCK => FileType::Socket,
            _ => return Err(ENOENT),
        };

        let d = Duration::from_secs(u64::from(inode.lmtime) | (u64::from(inode.hmtime) << 32));
        let ts = UNIX_EPOCH.checked_add(d).unwrap_or(UNIX_EPOCH);

        let rdev = match kind {
            FileType::BlockDevice | FileType::CharDevice => {
                let offset: u64 = inode.offset.into();
                makedev((offset >> 32) as _, offset as _) as u32
            }
            _ => 0,
        };

        Ok(fuser::FileAttr {
            ino,
            size: inode.size.into(),
            blocks: (u64::from(inode.size) + TARFS_BSIZE - 1) / TARFS_BSIZE,
            atime: ts,
            mtime: ts,
            ctime: ts,
            crtime: ts,
            kind,
            perm: u16::from(inode.mode) & 0o777,
            nlink: 1,
            uid: inode.owner.into(),
            gid: inode.group.into(),
            rdev,
            flags: 0,
            blksize: TARFS_BSIZE as _,
        })
    }

    fn for_each<T>(
        &self,
        parent: u64,
        first: i64,
        mut cb: impl FnMut(&DirEntry, u64) -> Result<Option<T>, i32>,
    ) -> Result<Option<T>, i32> {
        let inode = self.inode(parent)?;

        if u16::from(inode.mode) & S_IFMT != S_IFDIR {
            return Err(ENOENT);
        }

        if first < 0 || first % size_of::<DirEntry>() as i64 != 0 {
            return Err(ENOENT);
        }

        for offset in (first as u64..inode.size.into()).step_by(size_of::<DirEntry>()) {
            let dentry = DirEntry::read_from_prefix(
                &self.data[(u64::from(inode.offset) + offset) as usize..],
            )
            .unwrap();
            if let Some(v) = cb(&dentry, offset)? {
                return Ok(Some(v));
            }
        }

        Ok(None)
    }
}

impl fuser::Filesystem for Tar {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: fuser::ReplyEntry) {
        let ret = self.for_each(parent, 0, |dentry, _| {
            let ename = OsStr::from_bytes(
                &self.data[u64::from(dentry.name_offset) as usize..]
                    [..u64::from(dentry.name_len) as usize],
            );

            Ok(if ename != name {
                None
            } else {
                Some(self.attr(dentry.ino.into())?)
            })
        });

        match ret {
            Ok(Some(a)) => reply.entry(&Duration::MAX, &a, 0),
            Ok(None) => reply.error(ENOENT),
            Err(e) => reply.error(e),
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: fuser::ReplyAttr) {
        match self.attr(ino) {
            Ok(a) => reply.attr(&Duration::MAX, &a),
            Err(e) => reply.error(e),
        }
    }

    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock: Option<u64>,
        reply: fuser::ReplyData,
    ) {
        let inode = match self.inode(ino) {
            Ok(i) => i,
            Err(e) => return reply.error(e),
        };
        if u16::from(inode.mode) & S_IFMT != S_IFREG {
            return reply.error(ENOENT);
        }
        let fsize = u64::from(inode.size);
        if offset < 0 {
            return reply.error(EINVAL);
        }

        if offset as u64 >= fsize {
            return reply.data(&[]);
        }

        let available = fsize - offset as u64;
        reply.data(
            &self.data[(u64::from(inode.offset) + offset as u64) as usize..]
                [..std::cmp::min(available, size.into()) as usize],
        );
    }

    fn readlink(&mut self, _req: &Request, ino: u64, reply: fuser::ReplyData) {
        let inode = match self.inode(ino) {
            Ok(i) => i,
            Err(e) => return reply.error(e),
        };
        if u16::from(inode.mode) & S_IFMT != S_IFLNK {
            return reply.error(ENOENT);
        }
        reply
            .data(&self.data[u64::from(inode.offset) as usize..][..u64::from(inode.size) as usize]);
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        first: i64,
        mut reply: fuser::ReplyDirectory,
    ) {
        let ret = self.for_each(ino, first, |dentry, offset| {
            let etype = match dentry.etype {
                DT_FIFO => FileType::NamedPipe,
                DT_CHR => FileType::CharDevice,
                DT_DIR => FileType::Directory,
                DT_BLK => FileType::BlockDevice,
                DT_REG => FileType::RegularFile,
                DT_LNK => FileType::Symlink,
                DT_SOCK => FileType::Socket,
                _ => return Ok(None),
            };

            if reply.add(
                dentry.ino.into(),
                (offset + size_of::<DirEntry>() as u64) as i64,
                etype,
                OsStr::from_bytes(
                    &self.data[u64::from(dentry.name_offset) as usize..]
                        [..u64::from(dentry.name_len) as usize],
                ),
            ) {
                Ok(Some(()))
            } else {
                Ok(None)
            }
        });

        match ret {
            Err(e) => reply.error(e),
            Ok(_) => reply.ok(),
        }
    }

    fn getxattr(
        &mut self,
        _req: &Request,
        ino: u64,
        name: &OsStr,
        size: u32,
        reply: fuser::ReplyXattr,
    ) {
        let inode = match self.inode(ino) {
            Ok(i) => i,
            Err(e) => return reply.error(e),
        };
        if inode.flags & inode_flags::OPAQUE == 0 || name != "trusted.overlay.opaque" {
            return reply.error(ENODATA);
        }

        if size == 0 {
            reply.size(1);
        } else {
            reply.data(b"y");
        }
    }

    fn listxattr(&mut self, _req: &Request<'_>, ino: u64, size: u32, reply: fuser::ReplyXattr) {
        let inode = match self.inode(ino) {
            Ok(i) => i,
            Err(e) => return reply.error(e),
        };
        if inode.flags & inode_flags::OPAQUE == 0 {
            return reply.data(&[]);
        }
        const DATA: &[u8] = b"trusted.overlay.opaque\0";
        const DATA_SIZE: u32 = DATA.len() as u32;
        if size < DATA_SIZE {
            reply.size(DATA_SIZE);
        } else {
            reply.data(DATA);
        }
    }
}
