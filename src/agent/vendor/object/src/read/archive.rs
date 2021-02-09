//! Support for archive files.

use crate::read::{self, Error, ReadError};
use crate::{archive, Bytes};

/// The kind of archive format.
// TODO: Gnu64 and Darwin64 (and Darwin for writing)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArchiveKind {
    /// There are no special files that indicate the archive format.
    Unknown,
    /// The GNU (or System V) archive format.
    Gnu,
    /// The BSD archive format.
    Bsd,
    /// The Windows COFF archive format.
    Coff,
}

/// A partially parsed archive file.
#[derive(Debug)]
pub struct ArchiveFile<'data> {
    data: Bytes<'data>,
    kind: ArchiveKind,
    symbols: Bytes<'data>,
    names: Bytes<'data>,
}

impl<'data> ArchiveFile<'data> {
    /// Parse the archive header and special members.
    pub fn parse(data: &'data [u8]) -> read::Result<Self> {
        let data = Bytes(data);
        let mut tail = data;

        let magic = tail
            .read_bytes(archive::MAGIC.len())
            .read_error("Invalid archive size")?;
        if magic.0 != &archive::MAGIC[..] {
            return Err(Error("Unsupported archive identifier"));
        }

        let mut file = ArchiveFile {
            data: tail,
            kind: ArchiveKind::Unknown,
            symbols: Bytes(&[]),
            names: Bytes(&[]),
        };

        // The first few members may be special, so parse them.
        // GNU has:
        // - "/": symbol table (optional)
        // - "//": names table (optional)
        // COFF has:
        // - "/": first linker member
        // - "/": second linker member
        // - "//": names table
        // BSD has:
        // - "__.SYMDEF" or "__.SYMDEF SORTED": symbol table (optional)
        if !tail.is_empty() {
            let member = ArchiveMember::parse(&mut tail, Bytes(&[]))?;
            if member.name == b"/" {
                // GNU symbol table (unless we later determine this is COFF).
                file.kind = ArchiveKind::Gnu;
                file.symbols = member.data;
                file.data = tail;

                if !tail.is_empty() {
                    let member = ArchiveMember::parse(&mut tail, Bytes(&[]))?;
                    if member.name == b"/" {
                        // COFF linker member.
                        file.kind = ArchiveKind::Coff;
                        file.symbols = member.data;
                        file.data = tail;

                        if !tail.is_empty() {
                            let member = ArchiveMember::parse(&mut tail, Bytes(&[]))?;
                            if member.name == b"//" {
                                // COFF names table.
                                file.names = member.data;
                                file.data = tail;
                            }
                        }
                    } else if member.name == b"//" {
                        // GNU names table.
                        file.names = member.data;
                        file.data = tail;
                    }
                }
            } else if member.name == b"//" {
                // GNU names table.
                file.kind = ArchiveKind::Gnu;
                file.names = member.data;
                file.data = tail;
            } else if member.name == b"__.SYMDEF" || member.name == b"__.SYMDEF SORTED" {
                // BSD symbol table.
                file.kind = ArchiveKind::Bsd;
                file.symbols = member.data;
                file.data = tail;
            } else {
                // TODO: This could still be a BSD file. We leave this as unknown for now.
            }
        }
        Ok(file)
    }

    /// Return the archive format.
    #[inline]
    pub fn kind(&self) -> ArchiveKind {
        self.kind
    }

    /// Iterate over the members of the archive.
    ///
    /// This does not return special members.
    #[inline]
    pub fn members(&self) -> ArchiveMemberIterator<'data> {
        ArchiveMemberIterator {
            data: self.data,
            names: self.names,
        }
    }
}

/// An iterator over the members of an archive.
#[derive(Debug)]
pub struct ArchiveMemberIterator<'data> {
    data: Bytes<'data>,
    names: Bytes<'data>,
}

impl<'data> Iterator for ArchiveMemberIterator<'data> {
    type Item = read::Result<ArchiveMember<'data>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.is_empty() {
            return None;
        }
        let member = ArchiveMember::parse(&mut self.data, self.names);
        if member.is_err() {
            self.data = Bytes(&[]);
        }
        Some(member)
    }
}

/// A partially parsed archive member.
#[derive(Debug)]
pub struct ArchiveMember<'data> {
    header: &'data archive::Header,
    name: &'data [u8],
    data: Bytes<'data>,
}

impl<'data> ArchiveMember<'data> {
    /// Parse the archive member header, name, and file data.
    ///
    /// This reads the extended name (if any) and adjusts the file size.
    fn parse(data: &mut Bytes<'data>, names: Bytes<'data>) -> read::Result<Self> {
        let header = data
            .read::<archive::Header>()
            .read_error("Invalid archive member header")?;
        if header.terminator != archive::TERMINATOR {
            return Err(Error("Invalid archive terminator"));
        }

        let size =
            parse_usize_digits(&header.size, 10).read_error("Invalid archive member size")?;
        let mut file_data = data
            .read_bytes(size)
            .read_error("Archive member size is too large")?;
        // Entries are padded to an even number of bytes.
        if (size & 1) != 0 {
            data.skip(1).ok();
        }

        let name = if header.name[0] == b'/' && (header.name[1] as char).is_digit(10) {
            // Read file name from the names table.
            parse_sysv_extended_name(&header.name[1..], names)
                .read_error("Invalid archive extended name offset")?
        } else if &header.name[..3] == b"#1/" && (header.name[3] as char).is_digit(10) {
            // Read file name from the start of the file data.
            parse_bsd_extended_name(&header.name[3..], &mut file_data)
                .read_error("Invalid archive extended name length")?
        } else if header.name[0] == b'/' {
            let name_len =
                (header.name.iter().position(|&x| x == b' ')).unwrap_or_else(|| header.name.len());
            &header.name[..name_len]
        } else {
            let name_len = (header.name.iter().position(|&x| x == b'/'))
                .or_else(|| header.name.iter().position(|&x| x == b' '))
                .unwrap_or_else(|| header.name.len());
            &header.name[..name_len]
        };

        Ok(ArchiveMember {
            header,
            name,
            data: file_data,
        })
    }

    /// Return the raw header.
    #[inline]
    pub fn header(&self) -> &'data archive::Header {
        self.header
    }

    /// Return the parsed file name.
    ///
    /// This may be an extended file name.
    #[inline]
    pub fn name(&self) -> &'data [u8] {
        self.name
    }

    /// Parse the file modification timestamp from the header.
    #[inline]
    pub fn date(&self) -> Option<usize> {
        parse_usize_digits(&self.header.date, 10)
    }

    /// Parse the user ID from the header.
    #[inline]
    pub fn uid(&self) -> Option<usize> {
        parse_usize_digits(&self.header.uid, 10)
    }

    /// Parse the group ID from the header.
    #[inline]
    pub fn gid(&self) -> Option<usize> {
        parse_usize_digits(&self.header.gid, 10)
    }

    /// Parse the file mode from the header.
    #[inline]
    pub fn mode(&self) -> Option<usize> {
        parse_usize_digits(&self.header.mode, 8)
    }

    /// Return the file data.
    #[inline]
    pub fn data(&self) -> &'data [u8] {
        self.data.0
    }
}

// Ignores bytes starting from the first space.
fn parse_usize_digits(digits: &[u8], radix: u32) -> Option<usize> {
    let len = digits
        .iter()
        .position(|&x| x == b' ')
        .unwrap_or_else(|| digits.len());
    let digits = &digits[..len];
    if digits.is_empty() {
        return None;
    }
    let mut result: usize = 0;
    for &c in digits {
        let x = (c as char).to_digit(radix)?;
        result = result
            .checked_mul(radix as usize)?
            .checked_add(x as usize)?;
    }
    Some(result)
}

fn parse_sysv_extended_name<'data>(
    digits: &[u8],
    mut names: Bytes<'data>,
) -> Result<&'data [u8], ()> {
    let offset = parse_usize_digits(digits, 10).ok_or(())?;
    names.skip(offset)?;
    let name = match names.0.iter().position(|&x| x == b'/' || x == 0) {
        Some(len) => names.read_bytes(len)?,
        None => names,
    };
    Ok(name.0)
}

/// Modifies `data` to start after the extended name.
fn parse_bsd_extended_name<'data>(
    digits: &[u8],
    data: &mut Bytes<'data>,
) -> Result<&'data [u8], ()> {
    let len = parse_usize_digits(digits, 10).ok_or(())?;
    let mut name_data = data.read_bytes(len)?;
    let name = match name_data.0.iter().position(|&x| x == 0) {
        Some(len) => name_data.read_bytes(len)?,
        None => name_data,
    };
    Ok(name.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind() {
        let data = b"!<arch>\n";
        let archive = ArchiveFile::parse(data).unwrap();
        assert_eq!(archive.kind(), ArchiveKind::Unknown);

        let data = b"\
            !<arch>\n\
            /                                               4         `\n\
            0000";
        let archive = ArchiveFile::parse(data).unwrap();
        assert_eq!(archive.kind(), ArchiveKind::Gnu);

        let data = b"\
            !<arch>\n\
            //                                              4         `\n\
            0000";
        let archive = ArchiveFile::parse(data).unwrap();
        assert_eq!(archive.kind(), ArchiveKind::Gnu);

        let data = b"\
            !<arch>\n\
            /                                               4         `\n\
            0000\
            //                                              4         `\n\
            0000";
        let archive = ArchiveFile::parse(data).unwrap();
        assert_eq!(archive.kind(), ArchiveKind::Gnu);

        let data = b"\
            !<arch>\n\
            __.SYMDEF                                       4         `\n\
            0000";
        let archive = ArchiveFile::parse(data).unwrap();
        assert_eq!(archive.kind(), ArchiveKind::Bsd);

        let data = b"\
            !<arch>\n\
            #1/9                                            13        `\n\
            __.SYMDEF0000";
        let archive = ArchiveFile::parse(data).unwrap();
        assert_eq!(archive.kind(), ArchiveKind::Bsd);

        let data = b"\
            !<arch>\n\
            #1/16                                           20        `\n\
            __.SYMDEF SORTED0000";
        let archive = ArchiveFile::parse(data).unwrap();
        assert_eq!(archive.kind(), ArchiveKind::Bsd);

        let data = b"\
            !<arch>\n\
            /                                               4         `\n\
            0000\
            /                                               4         `\n\
            0000\
            //                                              4         `\n\
            0000";
        let archive = ArchiveFile::parse(data).unwrap();
        assert_eq!(archive.kind(), ArchiveKind::Coff);
    }

    #[test]
    fn gnu_names() {
        let data = b"\
            !<arch>\n\
            //                                              18        `\n\
            0123456789abcdef/\n\
            0123456789abcde/0           0     0     644     3         `\n\
            odd\n\
            /0              0           0     0     644     4         `\n\
            even";
        let archive = ArchiveFile::parse(data).unwrap();
        assert_eq!(archive.kind(), ArchiveKind::Gnu);
        let mut members = archive.members();

        let member = members.next().unwrap().unwrap();
        assert_eq!(member.name(), b"0123456789abcde");
        assert_eq!(member.data(), b"odd");

        let member = members.next().unwrap().unwrap();
        assert_eq!(member.name(), b"0123456789abcdef");
        assert_eq!(member.data(), b"even");

        assert!(members.next().is_none());
    }

    #[test]
    fn bsd_names() {
        let data = b"\
            !<arch>\n\
            0123456789abcde 0           0     0     644     3         `\n\
            odd\n\
            #1/16           0           0     0     644     20        `\n\
            0123456789abcdefeven";
        let archive = ArchiveFile::parse(data).unwrap();
        assert_eq!(archive.kind(), ArchiveKind::Unknown);
        let mut members = archive.members();

        let member = members.next().unwrap().unwrap();
        assert_eq!(member.name(), b"0123456789abcde");
        assert_eq!(member.data(), b"odd");

        let member = members.next().unwrap().unwrap();
        assert_eq!(member.name(), b"0123456789abcdef");
        assert_eq!(member.data(), b"even");

        assert!(members.next().is_none());
    }
}
