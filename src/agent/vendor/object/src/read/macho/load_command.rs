use core::marker::PhantomData;

use crate::endian::Endian;
use crate::macho;
use crate::pod::Bytes;
use crate::read::macho::{MachHeader, SymbolTable};
use crate::read::{ReadError, Result, StringTable};

/// An iterator over the load commands of a `MachHeader`.
#[derive(Debug, Default, Clone, Copy)]
pub struct MachOLoadCommandIterator<'data, E: Endian> {
    endian: E,
    data: Bytes<'data>,
    ncmds: u32,
}

impl<'data, E: Endian> MachOLoadCommandIterator<'data, E> {
    pub(super) fn new(endian: E, data: Bytes<'data>, ncmds: u32) -> Self {
        MachOLoadCommandIterator {
            endian,
            data,
            ncmds,
        }
    }

    /// Return the next load command.
    pub fn next(&mut self) -> Result<Option<MachOLoadCommand<'data, E>>> {
        if self.ncmds == 0 {
            return Ok(None);
        }
        let header = self
            .data
            .read_at::<macho::LoadCommand<E>>(0)
            .read_error("Invalid Mach-O load command header")?;
        let cmd = header.cmd.get(self.endian);
        let cmdsize = header.cmdsize.get(self.endian) as usize;
        let data = self
            .data
            .read_bytes(cmdsize)
            .read_error("Invalid Mach-O load command size")?;
        self.ncmds -= 1;
        Ok(Some(MachOLoadCommand {
            cmd,
            data,
            marker: Default::default(),
        }))
    }
}

/// A parsed `LoadCommand`.
#[derive(Debug, Clone, Copy)]
pub struct MachOLoadCommand<'data, E: Endian> {
    cmd: u32,
    // Includes the header.
    data: Bytes<'data>,
    marker: PhantomData<E>,
}

impl<'data, E: Endian> MachOLoadCommand<'data, E> {
    /// Try to parse this command as a `SegmentCommand32`.
    pub fn segment_32(self) -> Result<Option<(&'data macho::SegmentCommand32<E>, Bytes<'data>)>> {
        if self.cmd == macho::LC_SEGMENT {
            let mut data = self.data;
            let command = data
                .read()
                .read_error("Invalid Mach-O LC_SEGMENT command size")?;
            Ok(Some((command, data)))
        } else {
            Ok(None)
        }
    }

    /// Try to parse this command as a `SymtabCommand`.
    pub fn symtab(self) -> Result<Option<&'data macho::SymtabCommand<E>>> {
        if self.cmd == macho::LC_SYMTAB {
            Some(
                self.data
                    .clone()
                    .read()
                    .read_error("Invalid Mach-O LC_SYMTAB command size"),
            )
            .transpose()
        } else {
            Ok(None)
        }
    }

    /// Try to parse this command as a `UuidCommand`.
    pub fn uuid(self) -> Result<Option<&'data macho::UuidCommand<E>>> {
        if self.cmd == macho::LC_UUID {
            Some(
                self.data
                    .clone()
                    .read()
                    .read_error("Invalid Mach-O LC_UUID command size"),
            )
            .transpose()
        } else {
            Ok(None)
        }
    }

    /// Try to parse this command as a `SegmentCommand64`.
    pub fn segment_64(self) -> Result<Option<(&'data macho::SegmentCommand64<E>, Bytes<'data>)>> {
        if self.cmd == macho::LC_SEGMENT_64 {
            let mut data = self.data;
            let command = data
                .read()
                .read_error("Invalid Mach-O LC_SEGMENT_64 command size")?;
            Ok(Some((command, data)))
        } else {
            Ok(None)
        }
    }

    /// Try to parse this command as an `EntryPointCommand`.
    pub fn entry_point(self) -> Result<Option<&'data macho::EntryPointCommand<E>>> {
        if self.cmd == macho::LC_MAIN {
            Some(
                self.data
                    .clone()
                    .read()
                    .read_error("Invalid Mach-O LC_MAIN command size"),
            )
            .transpose()
        } else {
            Ok(None)
        }
    }
}

impl<E: Endian> macho::SymtabCommand<E> {
    /// Return the symbol table that this command references.
    pub fn symbols<'data, Mach: MachHeader<Endian = E>>(
        &self,
        endian: E,
        data: Bytes<'data>,
    ) -> Result<SymbolTable<'data, Mach>> {
        let symbols = data
            .read_slice_at(
                self.symoff.get(endian) as usize,
                self.nsyms.get(endian) as usize,
            )
            .read_error("Invalid Mach-O symbol table offset or size")?;
        let strings = data
            .read_bytes_at(
                self.stroff.get(endian) as usize,
                self.strsize.get(endian) as usize,
            )
            .read_error("Invalid Mach-O string table offset or size")?;
        let strings = StringTable::new(strings);
        Ok(SymbolTable::new(symbols, strings))
    }
}
