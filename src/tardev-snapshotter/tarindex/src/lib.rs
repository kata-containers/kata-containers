use std::collections::{BTreeMap, VecDeque};
use std::{cell::RefCell, io, mem, rc::Rc};
use tar::Archive;
use zerocopy::byteorder::{LE, U16, U32, U64};
use zerocopy::AsBytes;

#[derive(zerocopy::AsBytes, zerocopy::Unaligned)]
#[repr(C)]
struct Inode {
    mode: U16<LE>,
    _padding: u8,
    hmtime: u8, // High 4 bits of mtime.
    owner: U32<LE>,
    group: U32<LE>,
    lmtime: U32<LE>, // Lower 32 bits of mtime.
    size: U64<LE>,
    offset: U64<LE>, // 64 bits of offset, or 32 LSB are minor dev and 32 MSB are major dev.
}

#[derive(zerocopy::AsBytes, zerocopy::Unaligned)]
#[repr(C)]
struct DirEntry {
    ino: U64<LE>,
    name_offset: U64<LE>,
    name_len: U64<LE>,
    etype: u8,
    _padding: [u8; 7],
}

#[derive(zerocopy::AsBytes, zerocopy::Unaligned)]
#[repr(C)]
struct SuperBlock {
    inode_table_offset: U64<LE>,
    inode_count: U64<LE>,
}

const S_IFMT: u16 = 0o0170000;
const S_IFLNK: u16 = 0o0120000;
const S_IFREG: u16 = 0o0100000;
const S_IFDIR: u16 = 0o0040000;

const DT_UNKNOWN: u8 = 0;
const DT_DIR: u8 = 4;
const DT_REG: u8 = 8;
const DT_LNK: u8 = 10;

/*
#define DT_FIFO		1
#define DT_CHR		2
#define DT_BLK		6
#define DT_SOCK		12
#define DT_WHT		14
*/

#[derive(Default)]
struct Entry {
    offset: u64,
    size: u64,
    children: BTreeMap<Vec<u8>, Rc<RefCell<Entry>>>,
    mode: u16,
    ino: u64,
    emitted: bool,

    mtime: u64,
    owner: u32,
    group: u32,
}

impl Entry {
    fn find_or_create_child(&mut self, name: &[u8]) -> Rc<RefCell<Entry>> {
        self.children
            .entry(name.to_vec())
            .or_insert_with(|| Rc::new(RefCell::new(Entry::default())))
            .clone()
    }
}

fn visit_breadth_first_mut(
    root: Rc<RefCell<Entry>>,
    mut visitor: impl FnMut(&mut Entry) -> io::Result<()>,
) -> io::Result<()> {
    let mut q = VecDeque::new();
    q.push_back(root);

    while let Some(e) = q.pop_front() {
        visitor(&mut e.borrow_mut())?;

        for child in e.borrow().children.values() {
            q.push_back(child.clone());
        }
    }

    Ok(())
}

// TODO: Handle white-outs.
// https://www.kernel.org/doc/Documentation/filesystems/overlayfs.txt contains information about
// how overlayfs handles this.
fn read_all_entries(
    reader: &mut (impl io::Read + io::Seek),
    root: &mut Rc<RefCell<Entry>>,
    mut cb: impl FnMut(&mut Rc<RefCell<Entry>>, &[u8], &Entry),
    mut hardlink: impl FnMut(&mut Rc<RefCell<Entry>>, &[u8], &[u8]),
) -> io::Result<u64> {
    let mut ar = Archive::new(reader);

    for file in ar.entries()? {
        let f = file?;
        let h = f.header();

        let mut mode = if let Ok(m) = h.mode() {
            m as u16 & 0x1ff
        } else {
            continue;
        };

        // TODO: For now we only accept symlinks, regular files and directories. Need to support
        // more in the future.
        let entry_size;
        let entry_offset;
        match h.entry_type() {
            tar::EntryType::Regular => {
                mode |= S_IFREG;
                entry_size = f.size();
                entry_offset = f.raw_file_position();
            }
            tar::EntryType::Directory => {
                mode |= S_IFDIR;
                entry_size = 0;
                entry_offset = 0;
            }
            tar::EntryType::Symlink => {
                mode |= S_IFLNK;
                match f.link_name_bytes() {
                    Some(name) => {
                        let hname = h
                            .link_name_bytes()
                            .unwrap_or(std::borrow::Cow::Borrowed(b""));
                        if *hname != *name {
                            // TODO: Handle this case by duplicating the full name.
                            eprintln!(
                                "Skipping symlink with long link name ({}, {} bytes, {}, {} bytes): {}",
                                String::from_utf8_lossy(&name), name.len(),
                                String::from_utf8_lossy(&hname), hname.len(),
                                String::from_utf8_lossy(&f.path_bytes())
                            );
                            continue;
                        }

                        entry_size = name.len() as u64;
                        entry_offset = f.raw_header_position() + 157;
                    }
                    None => {
                        eprintln!(
                            "Skipping symlink without a link name: {}",
                            String::from_utf8_lossy(&f.path_bytes())
                        );
                        continue;
                    }
                }
            }
            tar::EntryType::Link => {
                match f.link_name_bytes() {
                    Some(name) => hardlink(root, &f.path_bytes(), &name),
                    None => {
                        eprintln!(
                            "Skipping hardlink without a link name: {}",
                            String::from_utf8_lossy(&f.path_bytes())
                        );
                    }
                }
                continue;
            }
            _ => {
                eprintln!(
                    "Skipping unhandled file due to its type ({:?}): {}",
                    h.entry_type(),
                    String::from_utf8_lossy(&f.path_bytes())
                );
                continue;
            }
        }

        cb(
            root,
            &f.path_bytes(),
            &Entry {
                size: entry_size,
                offset: entry_offset,
                children: BTreeMap::new(),
                mode,
                ino: 0,
                emitted: false,
                mtime: h.mtime().unwrap_or(0),
                owner: h.uid().unwrap_or(0) as u32, // TODO: This can be a u64 in `tar`.
                group: h.gid().unwrap_or(0) as u32, // TODO: This can be a u64 in `tar`.
            },
        );
    }

    ar.into_inner().seek(io::SeekFrom::End(0))
}

fn clean_path(str: &[u8]) -> Option<Vec<&[u8]>> {
    let mut ret = Vec::new();

    for component in str.split(|&c| c == b'/') {
        match component {
            // Empty entries or "." are just ignored.
            b"" | b"." => {}

            // Pop an element when we see "..".
            b".." => {
                if ret.is_empty() {
                    return None;
                }
                ret.pop();
            }

            // Add anything else.
            _ => {
                ret.push(component);
            }
        }
    }

    Some(ret)
}

/// Initilises the `offset` of all `Entry` instances that represent directories.
///
/// Returns the next available offset.
///
/// `first_offset` is the offset of the first directory entry.
fn init_direntry_offset(root: Rc<RefCell<Entry>>, first_offset: u64) -> io::Result<u64> {
    let mut offset = first_offset;
    visit_breadth_first_mut(root, |e| {
        if e.mode & S_IFMT != S_IFDIR {
            return Ok(());
        }

        e.offset = offset;
        e.size = mem::size_of::<DirEntry>() as u64 * e.children.len() as u64;

        offset += e.size;
        Ok(())
    })?;
    Ok(offset)
}

/// Writes all directory entries to the given file.
///
/// Returns the next available offset for the strings.
///
/// `first_string_offset` is the offset of the first string.
fn write_direntry_bodies(
    root: Rc<RefCell<Entry>>,
    first_string_offset: u64,
    file: &mut impl io::Write,
) -> io::Result<u64> {
    let mut offset = first_string_offset;
    visit_breadth_first_mut(root, |e| {
        if e.mode & S_IFMT != S_IFDIR {
            return Ok(());
        }

        for (name, child) in &e.children {
            let child = child.borrow();
            let dirent = DirEntry {
                ino: child.ino.into(),
                name_offset: offset.into(),
                name_len: (name.len() as u64).into(),
                etype: match child.mode & S_IFMT {
                    S_IFREG => DT_REG,
                    S_IFDIR => DT_DIR,
                    S_IFLNK => DT_LNK,
                    _ => DT_UNKNOWN,
                },
                _padding: [0; 7],
            };
            file.write_all(dirent.as_bytes())?;
            offset += u64::from(dirent.name_len);
        }

        Ok(())
    })?;
    Ok(offset)
}

pub fn append_index(data: &mut (impl io::Read + io::Write + io::Seek)) -> io::Result<()> {
    let mut root = Rc::new(RefCell::new(Entry {
        mode: S_IFDIR | 0o555,
        ..Entry::default()
    }));

    let contents_size = read_all_entries(
        data,
        &mut root,
        |root, name, e| {
            // Break the name into path components.
            let path = if let Some(p) = clean_path(name) {
                p
            } else {
                // Skip files that don't point into the root.
                eprintln!("Skipping malformed name: {}", String::from_utf8_lossy(name));
                return;
            };

            // Find the right entry in the tree.
            let mut ptr = root.clone();
            for component in path {
                let new = ptr.borrow_mut().find_or_create_child(component);
                ptr = new;
            }

            let mut cur = ptr.borrow_mut();
            // Update the entry. We remove any previous existing entry.
            cur.children = BTreeMap::new();
            cur.mode = e.mode;
            cur.size = e.size;
            cur.offset = e.offset;
            cur.mtime = e.mtime;
            cur.owner = e.owner;
            cur.group = e.group;
        },
        |root, name, linkname| {
            // Find the destination.
            let path = if let Some(p) = clean_path(linkname) {
                p
            } else {
                // Skip files that don't point into the root.
                eprintln!(
                    "Skipping malformed linkname name: {}",
                    String::from_utf8_lossy(linkname)
                );
                return;
            };

            // Find existing file.
            let mut existing = root.clone();
            for component in path {
                let new = existing.borrow_mut().find_or_create_child(component);
                existing = new;
            }

            if existing.borrow().mode & S_IFMT != S_IFREG {
                eprintln!(
                    "Skipping link to non-file: {}",
                    String::from_utf8_lossy(linkname)
                );
                return;
            }

            // Find the file to create.
            let path = if let Some(p) = clean_path(name) {
                p
            } else {
                // Skip files that don't point into the root.
                eprintln!("Skipping malformed name: {}", String::from_utf8_lossy(name));
                return;
            };

            if path.is_empty() {
                *root = existing;
            } else {
                let mut ptr = root.clone();
                for component in path.iter().take(path.len() - 1) {
                    let new = ptr.borrow_mut().find_or_create_child(component);
                    ptr = new;
                }
                ptr.borrow_mut()
                    .children
                    .insert(path.last().unwrap().to_vec(), existing);
            }
        },
    )?;

    data.seek(io::SeekFrom::End(0))?;

    // Assign i-node numbers only for the entries that survided conversion to tree.
    let mut ino_count = 0u64;
    visit_breadth_first_mut(root.clone(), |e| {
        if e.ino == 0 {
            ino_count += 1;
            e.ino = ino_count;
        }
        Ok(())
    })?;

    // Calculate the offsets for directory entries.
    let inode_table_size: u64 = mem::size_of::<Inode>() as u64 * ino_count;
    let string_table_offset = init_direntry_offset(root.clone(), contents_size + inode_table_size)?;

    // Write the i-node table.
    visit_breadth_first_mut(root.clone(), |e| {
        if e.emitted {
            return Ok(());
        }

        e.emitted = true;
        let inode = Inode {
            mode: e.mode.into(),
            _padding: 0,
            hmtime: (e.mtime >> 32 & 0xf) as u8,
            owner: e.owner.into(),
            group: e.group.into(),
            lmtime: (e.mtime as u32).into(),
            size: e.size.into(),
            offset: e.offset.into(), // TODO: Handle the dev case and encode minor/major instead.
        };
        data.write_all(inode.as_bytes())?;
        Ok(())
    })?;

    // Write the directory bodies.
    let end_offset = write_direntry_bodies(root.clone(), string_table_offset, data)?;

    // Write the strings.
    visit_breadth_first_mut(root, |e| {
        if e.mode & S_IFMT != S_IFDIR {
            return Ok(());
        }

        for name in e.children.keys() {
            data.write_all(name)?;
        }

        Ok(())
    })?;

    // Write padding to align to the 512-byte boundary.
    for _ in 0..((512 - end_offset % 512) % 512) {
        data.write_all(&[0])?;
    }

    // Write the "super-block".
    let sb = SuperBlock {
        inode_table_offset: contents_size.into(),
        inode_count: ino_count.into(),
    };
    data.write_all(sb.as_bytes())?;

    // Write padding to align to the 512-byte boundary.
    for _ in 0..((512 - mem::size_of::<SuperBlock>() % 512) % 512) {
        data.write_all(&[0])?;
    }

    Ok(())
}
