use super::{Context, Mapping, Mmap, Path, Stash, Vec};
use core::convert::TryInto;
use object::macho;
use object::read::macho::{MachHeader, Nlist, Section, Segment as _};
use object::{Bytes, NativeEndian};

#[cfg(target_pointer_width = "32")]
type Mach = object::macho::MachHeader32<NativeEndian>;
#[cfg(target_pointer_width = "64")]
type Mach = object::macho::MachHeader64<NativeEndian>;
type MachSegment = <Mach as MachHeader>::Segment;
type MachSection = <Mach as MachHeader>::Section;
type MachNlist = <Mach as MachHeader>::Nlist;

impl Mapping {
    // The loading path for OSX is is so different we just have a completely
    // different implementation of the function here. On OSX we need to go
    // probing the filesystem for a bunch of files.
    pub fn new(path: &Path) -> Option<Mapping> {
        // First up we need to load the unique UUID which is stored in the macho
        // header of the file we're reading, specified at `path`.
        let map = super::mmap(path)?;
        let (macho, data) = find_header(Bytes(&map))?;
        let endian = macho.endian().ok()?;
        let uuid = macho.uuid(endian, data).ok()??;

        // Next we need to look for a `*.dSYM` file. For now we just probe the
        // containing directory and look around for something that matches
        // `*.dSYM`. Once it's found we root through the dwarf resources that it
        // contains and try to find a macho file which has a matching UUID as
        // the one of our own file. If we find a match that's the dwarf file we
        // want to return.
        let parent = path.parent()?;
        for entry in parent.read_dir().ok()? {
            let entry = entry.ok()?;
            let filename = match entry.file_name().into_string() {
                Ok(name) => name,
                Err(_) => continue,
            };
            if !filename.ends_with(".dSYM") {
                continue;
            }
            let candidates = entry.path().join("Contents/Resources/DWARF");
            if let Some(mapping) = load_dsym(&candidates, uuid) {
                return Some(mapping);
            }
        }

        // Looks like nothing matched our UUID, so let's at least return our own
        // file. This should have the symbol table for at least some
        // symbolication purposes.
        let stash = Stash::new();
        let inner = super::cx(&stash, Object::parse(macho, endian, data)?)?;
        return Some(mk!(Mapping { map, inner, stash }));

        fn load_dsym(dir: &Path, uuid: [u8; 16]) -> Option<Mapping> {
            for entry in dir.read_dir().ok()? {
                let entry = entry.ok()?;
                let map = super::mmap(&entry.path())?;
                let (macho, data) = find_header(Bytes(&map))?;
                let endian = macho.endian().ok()?;
                let entry_uuid = macho.uuid(endian, data).ok()??;
                if entry_uuid != uuid {
                    continue;
                }
                let stash = Stash::new();
                if let Some(cx) =
                    Object::parse(macho, endian, data).and_then(|o| super::cx(&stash, o))
                {
                    return Some(mk!(Mapping { map, cx, stash }));
                }
            }

            None
        }
    }
}

fn find_header(mut data: Bytes<'_>) -> Option<(&'_ Mach, Bytes<'_>)> {
    use object::endian::BigEndian;

    let desired_cpu = || {
        if cfg!(target_arch = "x86") {
            Some(macho::CPU_TYPE_X86)
        } else if cfg!(target_arch = "x86_64") {
            Some(macho::CPU_TYPE_X86_64)
        } else if cfg!(target_arch = "arm") {
            Some(macho::CPU_TYPE_ARM)
        } else if cfg!(target_arch = "aarch64") {
            Some(macho::CPU_TYPE_ARM64)
        } else {
            None
        }
    };

    match data
        .clone()
        .read::<object::endian::U32<NativeEndian>>()
        .ok()?
        .get(NativeEndian)
    {
        macho::MH_MAGIC_64 | macho::MH_CIGAM_64 | macho::MH_MAGIC | macho::MH_CIGAM => {}

        macho::FAT_MAGIC | macho::FAT_CIGAM => {
            let mut header_data = data;
            let endian = BigEndian;
            let header = header_data.read::<macho::FatHeader>().ok()?;
            let nfat = header.nfat_arch.get(endian);
            let arch = (0..nfat)
                .filter_map(|_| header_data.read::<macho::FatArch32>().ok())
                .find(|arch| desired_cpu() == Some(arch.cputype.get(endian)))?;
            let offset = arch.offset.get(endian);
            let size = arch.size.get(endian);
            data = data
                .read_bytes_at(offset.try_into().ok()?, size.try_into().ok()?)
                .ok()?;
        }

        macho::FAT_MAGIC_64 | macho::FAT_CIGAM_64 => {
            let mut header_data = data;
            let endian = BigEndian;
            let header = header_data.read::<macho::FatHeader>().ok()?;
            let nfat = header.nfat_arch.get(endian);
            let arch = (0..nfat)
                .filter_map(|_| header_data.read::<macho::FatArch64>().ok())
                .find(|arch| desired_cpu() == Some(arch.cputype.get(endian)))?;
            let offset = arch.offset.get(endian);
            let size = arch.size.get(endian);
            data = data
                .read_bytes_at(offset.try_into().ok()?, size.try_into().ok()?)
                .ok()?;
        }

        _ => return None,
    }

    Mach::parse(data).ok().map(|h| (h, data))
}

pub struct Object<'a> {
    endian: NativeEndian,
    data: Bytes<'a>,
    dwarf: Option<&'a [MachSection]>,
    syms: Vec<(&'a [u8], u64)>,
}

impl<'a> Object<'a> {
    fn parse(mach: &'a Mach, endian: NativeEndian, data: Bytes<'a>) -> Option<Object<'a>> {
        let mut dwarf = None;
        let mut syms = Vec::new();
        let mut commands = mach.load_commands(endian, data).ok()?;
        while let Ok(Some(command)) = commands.next() {
            if let Some((segment, section_data)) = MachSegment::from_command(command).ok()? {
                if segment.name() == b"__DWARF" {
                    dwarf = segment.sections(endian, section_data).ok();
                }
            } else if let Some(symtab) = command.symtab().ok()? {
                let symbols = symtab.symbols::<Mach>(endian, data).ok()?;
                syms = symbols
                    .iter()
                    .filter_map(|nlist: &MachNlist| {
                        let name = nlist.name(endian, symbols.strings()).ok()?;
                        if name.len() > 0 && !nlist.is_undefined() {
                            Some((name, u64::from(nlist.n_value(endian))))
                        } else {
                            None
                        }
                    })
                    .collect();
                syms.sort_unstable_by_key(|(_, addr)| *addr);
            }
        }

        Some(Object {
            endian,
            data,
            dwarf,
            syms,
        })
    }

    pub fn section(&self, _: &Stash, name: &str) -> Option<&'a [u8]> {
        let name = name.as_bytes();
        let dwarf = self.dwarf?;
        let section = dwarf.into_iter().find(|section| {
            let section_name = section.name();
            section_name == name || {
                section_name.starts_with(b"__")
                    && name.starts_with(b".")
                    && &section_name[2..] == &name[1..]
            }
        })?;
        Some(section.data(self.endian, self.data).ok()?.0)
    }

    pub fn search_symtab<'b>(&'b self, addr: u64) -> Option<&'b [u8]> {
        let i = match self.syms.binary_search_by_key(&addr, |(_, addr)| *addr) {
            Ok(i) => i,
            Err(i) => i.checked_sub(1)?,
        };
        let (sym, _addr) = self.syms.get(i)?;
        Some(sym)
    }
}
