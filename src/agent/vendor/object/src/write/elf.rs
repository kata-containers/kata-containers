use std::mem;
use std::vec::Vec;

use crate::elf;
use crate::endian::*;
use crate::pod::{bytes_of, BytesMut, WritableBuffer};
use crate::write::string::*;
use crate::write::util::*;
use crate::write::*;

#[derive(Default, Clone, Copy)]
struct ComdatOffsets {
    offset: usize,
    str_id: Option<StringId>,
    len: usize,
}

#[derive(Default, Clone, Copy)]
struct SectionOffsets {
    index: usize,
    offset: usize,
    str_id: Option<StringId>,
    reloc_index: usize,
    reloc_offset: usize,
    reloc_len: usize,
    reloc_str_id: Option<StringId>,
}

#[derive(Default, Clone, Copy)]
struct SymbolOffsets {
    index: usize,
    str_id: Option<StringId>,
}

impl Object {
    pub(crate) fn elf_section_info(
        &self,
        section: StandardSection,
    ) -> (&'static [u8], &'static [u8], SectionKind) {
        match section {
            StandardSection::Text => (&[], &b".text"[..], SectionKind::Text),
            StandardSection::Data => (&[], &b".data"[..], SectionKind::Data),
            StandardSection::ReadOnlyData | StandardSection::ReadOnlyString => {
                (&[], &b".rodata"[..], SectionKind::ReadOnlyData)
            }
            StandardSection::ReadOnlyDataWithRel => (&[], b".data.rel.ro", SectionKind::Data),
            StandardSection::UninitializedData => {
                (&[], &b".bss"[..], SectionKind::UninitializedData)
            }
            StandardSection::Tls => (&[], &b".tdata"[..], SectionKind::Tls),
            StandardSection::UninitializedTls => {
                (&[], &b".tbss"[..], SectionKind::UninitializedTls)
            }
            StandardSection::TlsVariables => {
                // Unsupported section.
                (&[], &[], SectionKind::TlsVariables)
            }
            StandardSection::Common => {
                // Unsupported section.
                (&[], &[], SectionKind::Common)
            }
        }
    }

    pub(crate) fn elf_subsection_name(&self, section: &[u8], value: &[u8]) -> Vec<u8> {
        let mut name = section.to_vec();
        name.push(b'.');
        name.extend(value);
        name
    }

    fn elf_has_relocation_addend(&self) -> Result<bool> {
        Ok(match self.architecture {
            Architecture::Arm => false,
            Architecture::Aarch64 => true,
            Architecture::I386 => false,
            Architecture::X86_64 => true,
            Architecture::S390x => true,
            _ => {
                return Err(Error(format!(
                    "unimplemented architecture {:?}",
                    self.architecture
                )));
            }
        })
    }

    pub(crate) fn elf_fixup_relocation(&mut self, mut relocation: &mut Relocation) -> Result<i64> {
        // Return true if we should use a section symbol to avoid preemption.
        fn want_section_symbol(relocation: &Relocation, symbol: &Symbol) -> bool {
            if symbol.scope != SymbolScope::Dynamic {
                // Only dynamic symbols can be preemptible.
                return false;
            }
            match symbol.kind {
                SymbolKind::Text | SymbolKind::Data => {}
                _ => return false,
            }
            match relocation.kind {
                // Anything using GOT or PLT is preemptible.
                // We also require that `Other` relocations must already be correct.
                RelocationKind::Got
                | RelocationKind::GotRelative
                | RelocationKind::GotBaseRelative
                | RelocationKind::PltRelative
                | RelocationKind::Elf(_) => return false,
                // Absolute relocations are preemptible for non-local data.
                // TODO: not sure if this rule is exactly correct
                // This rule was added to handle global data references in debuginfo.
                // Maybe this should be a new relocation kind so that the caller can decide.
                RelocationKind::Absolute => {
                    if symbol.kind == SymbolKind::Data {
                        return false;
                    }
                }
                _ => {}
            }
            true
        }

        // Use section symbols for relocations where required to avoid preemption.
        // Otherwise, the linker will fail with:
        //     relocation R_X86_64_PC32 against symbol `SomeSymbolName' can not be used when
        //     making a shared object; recompile with -fPIC
        let symbol = &self.symbols[relocation.symbol.0];
        if want_section_symbol(relocation, symbol) {
            if let Some(section) = symbol.section.id() {
                relocation.addend += symbol.value as i64;
                relocation.symbol = self.section_symbol(section);
            }
        }

        // Determine whether the addend is stored in the relocation or the data.
        if self.elf_has_relocation_addend()? {
            Ok(0)
        } else {
            let constant = relocation.addend;
            relocation.addend = 0;
            Ok(constant)
        }
    }

    pub(crate) fn elf_write(&self, buffer: &mut dyn WritableBuffer) -> Result<()> {
        let address_size = self.architecture.address_size().unwrap();
        let endian = self.endian;
        let elf32 = Elf32 { endian };
        let elf64 = Elf64 { endian };
        let elf: &dyn Elf = match address_size {
            AddressSize::U32 => &elf32,
            AddressSize::U64 => &elf64,
        };
        let pointer_align = address_size.bytes() as usize;

        // Calculate offsets of everything.
        let mut offset = 0;

        // ELF header.
        let e_ehsize = elf.file_header_size();
        offset += e_ehsize;

        // Create reloc section header names.
        let is_rela = self.elf_has_relocation_addend()?;
        let reloc_names: Vec<_> = self
            .sections
            .iter()
            .map(|section| {
                let mut reloc_name = Vec::new();
                if !section.relocations.is_empty() {
                    reloc_name.extend_from_slice(if is_rela {
                        &b".rela"[..]
                    } else {
                        &b".rel"[..]
                    });
                    reloc_name.extend_from_slice(&section.name);
                }
                reloc_name
            })
            .collect();

        // Calculate size of section data.
        let mut shstrtab = StringTable::default();
        let mut comdat_offsets = vec![ComdatOffsets::default(); self.comdats.len()];
        let mut section_offsets = vec![SectionOffsets::default(); self.sections.len()];
        // Null section.
        let mut section_num = 1;
        for (index, comdat) in self.comdats.iter().enumerate() {
            if comdat.kind != ComdatKind::Any {
                return Err(Error(format!(
                    "unsupported COMDAT symbol `{}` kind {:?}",
                    self.symbols[comdat.symbol.0].name().unwrap_or(""),
                    comdat.kind
                )));
            }

            comdat_offsets[index].str_id = Some(shstrtab.add(b".group"));
            section_num += 1;
            offset = align(offset, 4);
            comdat_offsets[index].offset = offset;
            let len = (comdat.sections.len() + 1) * 4;
            comdat_offsets[index].len = len;
            offset += len;
        }
        for (index, section) in self.sections.iter().enumerate() {
            section_offsets[index].str_id = Some(shstrtab.add(&section.name));
            section_offsets[index].index = section_num;
            section_num += 1;

            let len = section.data.len();
            if len != 0 {
                offset = align(offset, section.align as usize);
                section_offsets[index].offset = offset;
                offset += len;
            } else {
                section_offsets[index].offset = offset;
            }

            if !section.relocations.is_empty() {
                section_offsets[index].reloc_str_id = Some(shstrtab.add(&reloc_names[index]));
                section_offsets[index].reloc_index = section_num;
                section_num += 1;
            }
        }

        // Calculate index of symbols and add symbol strings to strtab.
        let mut strtab = StringTable::default();
        let mut symbol_offsets = vec![SymbolOffsets::default(); self.symbols.len()];
        // Null symbol.
        let mut symtab_count = 1;
        // Local symbols must come before global.
        for (index, symbol) in self.symbols.iter().enumerate() {
            if symbol.is_local() {
                symbol_offsets[index].index = symtab_count;
                symtab_count += 1;
            }
        }
        let symtab_count_local = symtab_count;
        for (index, symbol) in self.symbols.iter().enumerate() {
            if !symbol.is_local() {
                symbol_offsets[index].index = symtab_count;
                symtab_count += 1;
            }
        }
        for (index, symbol) in self.symbols.iter().enumerate() {
            if symbol.kind != SymbolKind::Section {
                symbol_offsets[index].str_id = Some(strtab.add(&symbol.name));
            }
        }

        // Calculate size of symtab.
        let symtab_str_id = shstrtab.add(&b".symtab"[..]);
        offset = align(offset, pointer_align);
        let symtab_offset = offset;
        let symtab_len = symtab_count * elf.symbol_size();
        offset += symtab_len;
        let symtab_index = section_num;
        section_num += 1;

        // Calculate size of symtab_shndx.
        let mut need_symtab_shndx = false;
        for symbol in &self.symbols {
            let index = symbol
                .section
                .id()
                .map(|s| section_offsets[s.0].index)
                .unwrap_or(0);
            if index >= elf::SHN_LORESERVE as usize {
                need_symtab_shndx = true;
                break;
            }
        }
        let symtab_shndx_offset = offset;
        let mut symtab_shndx_str_id = None;
        let mut symtab_shndx_len = 0;
        if need_symtab_shndx {
            symtab_shndx_str_id = Some(shstrtab.add(&b".symtab_shndx"[..]));
            symtab_shndx_len = symtab_count * 4;
            offset += symtab_shndx_len;
            section_num += 1;
        }

        // Calculate size of strtab.
        let strtab_str_id = shstrtab.add(&b".strtab"[..]);
        let strtab_offset = offset;
        let mut strtab_data = Vec::new();
        // Null name.
        strtab_data.push(0);
        strtab.write(1, &mut strtab_data);
        offset += strtab_data.len();
        let strtab_index = section_num;
        section_num += 1;

        // Calculate size of relocations.
        for (index, section) in self.sections.iter().enumerate() {
            let count = section.relocations.len();
            if count != 0 {
                offset = align(offset, pointer_align);
                section_offsets[index].reloc_offset = offset;
                let len = count * elf.rel_size(is_rela);
                section_offsets[index].reloc_len = len;
                offset += len;
            }
        }

        // Calculate size of shstrtab.
        let shstrtab_str_id = shstrtab.add(&b".shstrtab"[..]);
        let shstrtab_offset = offset;
        let mut shstrtab_data = Vec::new();
        // Null section name.
        shstrtab_data.push(0);
        shstrtab.write(1, &mut shstrtab_data);
        offset += shstrtab_data.len();
        let shstrtab_index = section_num;
        section_num += 1;

        // Calculate size of section headers.
        offset = align(offset, pointer_align);
        let e_shoff = offset;
        let e_shentsize = elf.section_header_size();
        offset += section_num * e_shentsize;

        // Start writing.
        buffer
            .reserve(offset)
            .map_err(|_| Error(String::from("Cannot allocate buffer")))?;

        // Write file header.
        let e_ident = elf::Ident {
            magic: elf::ELFMAG,
            class: match address_size {
                AddressSize::U32 => elf::ELFCLASS32,
                AddressSize::U64 => elf::ELFCLASS64,
            },
            data: if endian.is_little_endian() {
                elf::ELFDATA2LSB
            } else {
                elf::ELFDATA2MSB
            },
            version: elf::EV_CURRENT,
            os_abi: elf::ELFOSABI_NONE,
            abi_version: 0,
            padding: [0; 7],
        };
        let e_type = elf::ET_REL;
        let e_machine = match self.architecture {
            Architecture::Arm => elf::EM_ARM,
            Architecture::Aarch64 => elf::EM_AARCH64,
            Architecture::I386 => elf::EM_386,
            Architecture::X86_64 => elf::EM_X86_64,
            Architecture::S390x => elf::EM_S390,
            _ => {
                return Err(Error(format!(
                    "unimplemented architecture {:?}",
                    self.architecture
                )));
            }
        };
        let e_flags = if let FileFlags::Elf { e_flags } = self.flags {
            e_flags
        } else {
            0
        };
        let e_shnum = if section_num >= elf::SHN_LORESERVE as usize {
            0
        } else {
            section_num as u16
        };
        let e_shstrndx = if shstrtab_index >= elf::SHN_LORESERVE as usize {
            elf::SHN_XINDEX
        } else {
            shstrtab_index as u16
        };

        elf.write_file_header(
            buffer,
            FileHeader {
                e_ident,
                e_type,
                e_machine,
                e_version: elf::EV_CURRENT.into(),
                e_entry: 0,
                e_phoff: 0,
                e_shoff: e_shoff as u64,
                e_flags,
                e_ehsize: e_ehsize as u16,
                e_phentsize: 0,
                e_phnum: 0,
                e_shentsize: e_shentsize as u16,
                e_shnum,
                e_shstrndx,
            },
        );

        // Write section data.
        for (index, comdat) in self.comdats.iter().enumerate() {
            let mut data = BytesMut::new();
            data.write(&U32::new(endian, elf::GRP_COMDAT));
            for section in &comdat.sections {
                data.write(&U32::new(endian, section_offsets[section.0].index as u32));
            }

            write_align(buffer, 4);
            debug_assert_eq!(comdat_offsets[index].offset, buffer.len());
            debug_assert_eq!(comdat_offsets[index].len, data.len());
            buffer.extend(data.as_slice());
        }
        for (index, section) in self.sections.iter().enumerate() {
            let len = section.data.len();
            if len != 0 {
                write_align(buffer, section.align as usize);
                debug_assert_eq!(section_offsets[index].offset, buffer.len());
                buffer.extend(section.data.as_slice());
            }
        }

        // Write symbols.
        write_align(buffer, pointer_align);
        debug_assert_eq!(symtab_offset, buffer.len());
        elf.write_symbol(
            buffer,
            Sym {
                st_name: 0,
                st_info: 0,
                st_other: 0,
                st_shndx: 0,
                st_value: 0,
                st_size: 0,
            },
        );
        let mut symtab_shndx = BytesMut::new();
        if need_symtab_shndx {
            symtab_shndx.write(&U32::new(endian, 0));
        }
        let mut write_symbol = |index: usize, symbol: &Symbol| -> Result<()> {
            let st_info = if let SymbolFlags::Elf { st_info, .. } = symbol.flags {
                st_info
            } else {
                let st_type = match symbol.kind {
                    SymbolKind::Null => elf::STT_NOTYPE,
                    SymbolKind::Text => {
                        if symbol.is_undefined() {
                            elf::STT_NOTYPE
                        } else {
                            elf::STT_FUNC
                        }
                    }
                    SymbolKind::Data => {
                        if symbol.is_undefined() {
                            elf::STT_NOTYPE
                        } else if symbol.is_common() {
                            elf::STT_COMMON
                        } else {
                            elf::STT_OBJECT
                        }
                    }
                    SymbolKind::Section => elf::STT_SECTION,
                    SymbolKind::File => elf::STT_FILE,
                    SymbolKind::Tls => elf::STT_TLS,
                    SymbolKind::Label => elf::STT_NOTYPE,
                    SymbolKind::Unknown => {
                        if symbol.is_undefined() {
                            elf::STT_NOTYPE
                        } else {
                            return Err(Error(format!(
                                "unimplemented symbol `{}` kind {:?}",
                                symbol.name().unwrap_or(""),
                                symbol.kind
                            )));
                        }
                    }
                };
                let st_bind = if symbol.weak {
                    elf::STB_WEAK
                } else if symbol.is_undefined() {
                    elf::STB_GLOBAL
                } else if symbol.is_local() {
                    elf::STB_LOCAL
                } else {
                    elf::STB_GLOBAL
                };
                (st_bind << 4) + st_type
            };
            let st_other = if let SymbolFlags::Elf { st_other, .. } = symbol.flags {
                st_other
            } else if symbol.scope == SymbolScope::Linkage {
                elf::STV_HIDDEN
            } else {
                elf::STV_DEFAULT
            };
            let (st_shndx, xindex) = match symbol.section {
                SymbolSection::None => {
                    debug_assert_eq!(symbol.kind, SymbolKind::File);
                    (elf::SHN_ABS, 0)
                }
                SymbolSection::Undefined => (elf::SHN_UNDEF, 0),
                SymbolSection::Absolute => (elf::SHN_ABS, 0),
                SymbolSection::Common => (elf::SHN_COMMON, 0),
                SymbolSection::Section(id) => {
                    let index = section_offsets[id.0].index as u32;
                    (
                        if index >= elf::SHN_LORESERVE as u32 {
                            elf::SHN_XINDEX
                        } else {
                            index as u16
                        },
                        index,
                    )
                }
            };
            let st_name = symbol_offsets[index]
                .str_id
                .map(|id| strtab.get_offset(id))
                .unwrap_or(0) as u32;
            elf.write_symbol(
                buffer,
                Sym {
                    st_name,
                    st_info,
                    st_other,
                    st_shndx,
                    st_value: symbol.value,
                    st_size: symbol.size,
                },
            );
            if need_symtab_shndx {
                symtab_shndx.write(&U32::new(endian, xindex));
            }
            Ok(())
        };
        for (index, symbol) in self.symbols.iter().enumerate() {
            if symbol.is_local() {
                write_symbol(index, symbol)?;
            }
        }
        for (index, symbol) in self.symbols.iter().enumerate() {
            if !symbol.is_local() {
                write_symbol(index, symbol)?;
            }
        }
        if need_symtab_shndx {
            debug_assert_eq!(symtab_shndx_offset, buffer.len());
            debug_assert_eq!(symtab_shndx_len, symtab_shndx.len());
            buffer.extend(symtab_shndx.as_slice());
        }

        // Write strtab section.
        debug_assert_eq!(strtab_offset, buffer.len());
        buffer.extend(&strtab_data);

        // Write relocations.
        for (index, section) in self.sections.iter().enumerate() {
            if !section.relocations.is_empty() {
                write_align(buffer, pointer_align);
                debug_assert_eq!(section_offsets[index].reloc_offset, buffer.len());
                for reloc in &section.relocations {
                    let r_type = match self.architecture {
                        Architecture::I386 => match (reloc.kind, reloc.size) {
                            (RelocationKind::Absolute, 32) => elf::R_386_32,
                            (RelocationKind::Relative, 32) => elf::R_386_PC32,
                            (RelocationKind::Got, 32) => elf::R_386_GOT32,
                            (RelocationKind::PltRelative, 32) => elf::R_386_PLT32,
                            (RelocationKind::GotBaseOffset, 32) => elf::R_386_GOTOFF,
                            (RelocationKind::GotBaseRelative, 32) => elf::R_386_GOTPC,
                            (RelocationKind::Absolute, 16) => elf::R_386_16,
                            (RelocationKind::Relative, 16) => elf::R_386_PC16,
                            (RelocationKind::Absolute, 8) => elf::R_386_8,
                            (RelocationKind::Relative, 8) => elf::R_386_PC8,
                            (RelocationKind::Elf(x), _) => x,
                            _ => {
                                return Err(Error(format!("unimplemented relocation {:?}", reloc)));
                            }
                        },
                        Architecture::X86_64 => match (reloc.kind, reloc.encoding, reloc.size) {
                            (RelocationKind::Absolute, RelocationEncoding::Generic, 64) => {
                                elf::R_X86_64_64
                            }
                            (RelocationKind::Relative, _, 32) => elf::R_X86_64_PC32,
                            (RelocationKind::Got, _, 32) => elf::R_X86_64_GOT32,
                            (RelocationKind::PltRelative, _, 32) => elf::R_X86_64_PLT32,
                            (RelocationKind::GotRelative, _, 32) => elf::R_X86_64_GOTPCREL,
                            (RelocationKind::Absolute, RelocationEncoding::Generic, 32) => {
                                elf::R_X86_64_32
                            }
                            (RelocationKind::Absolute, RelocationEncoding::X86Signed, 32) => {
                                elf::R_X86_64_32S
                            }
                            (RelocationKind::Absolute, _, 16) => elf::R_X86_64_16,
                            (RelocationKind::Relative, _, 16) => elf::R_X86_64_PC16,
                            (RelocationKind::Absolute, _, 8) => elf::R_X86_64_8,
                            (RelocationKind::Relative, _, 8) => elf::R_X86_64_PC8,
                            (RelocationKind::Elf(x), _, _) => x,
                            _ => {
                                return Err(Error(format!("unimplemented relocation {:?}", reloc)));
                            }
                        },
                        Architecture::Aarch64 => match (reloc.kind, reloc.encoding, reloc.size) {
                            (RelocationKind::Absolute, RelocationEncoding::Generic, 32) => {
                                elf::R_AARCH64_ABS32
                            }
                            (RelocationKind::Absolute, RelocationEncoding::Generic, 64) => {
                                elf::R_AARCH64_ABS64
                            }
                            (RelocationKind::Elf(x), _, _) => x,
                            _ => {
                                return Err(Error(format!("unimplemented relocation {:?}", reloc)));
                            }
                        },
                        Architecture::S390x => match (reloc.kind, reloc.encoding, reloc.size) {
                            (RelocationKind::Absolute, RelocationEncoding::Generic, 8) => {
                                elf::R_390_8
                            }
                            (RelocationKind::Absolute, RelocationEncoding::Generic, 16) => {
                                elf::R_390_16
                            }
                            (RelocationKind::Absolute, RelocationEncoding::Generic, 32) => {
                                elf::R_390_32
                            }
                            (RelocationKind::Absolute, RelocationEncoding::Generic, 64) => {
                                elf::R_390_64
                            }
                            (RelocationKind::Relative, RelocationEncoding::Generic, 16) => {
                                elf::R_390_PC16
                            }
                            (RelocationKind::Relative, RelocationEncoding::Generic, 32) => {
                                elf::R_390_PC32
                            }
                            (RelocationKind::Relative, RelocationEncoding::Generic, 64) => {
                                elf::R_390_PC64
                            }
                            (RelocationKind::Relative, RelocationEncoding::S390xDbl, 16) => {
                                elf::R_390_PC16DBL
                            }
                            (RelocationKind::Relative, RelocationEncoding::S390xDbl, 32) => {
                                elf::R_390_PC32DBL
                            }
                            (RelocationKind::PltRelative, RelocationEncoding::S390xDbl, 16) => {
                                elf::R_390_PLT16DBL
                            }
                            (RelocationKind::PltRelative, RelocationEncoding::S390xDbl, 32) => {
                                elf::R_390_PLT32DBL
                            }
                            (RelocationKind::Got, RelocationEncoding::Generic, 16) => {
                                elf::R_390_GOT16
                            }
                            (RelocationKind::Got, RelocationEncoding::Generic, 32) => {
                                elf::R_390_GOT32
                            }
                            (RelocationKind::Got, RelocationEncoding::Generic, 64) => {
                                elf::R_390_GOT64
                            }
                            (RelocationKind::GotRelative, RelocationEncoding::S390xDbl, 32) => {
                                elf::R_390_GOTENT
                            }
                            (RelocationKind::GotBaseOffset, RelocationEncoding::Generic, 16) => {
                                elf::R_390_GOTOFF16
                            }
                            (RelocationKind::GotBaseOffset, RelocationEncoding::Generic, 32) => {
                                elf::R_390_GOTOFF32
                            }
                            (RelocationKind::GotBaseOffset, RelocationEncoding::Generic, 64) => {
                                elf::R_390_GOTOFF64
                            }
                            (RelocationKind::GotBaseRelative, RelocationEncoding::Generic, 64) => {
                                elf::R_390_GOTPC
                            }
                            (RelocationKind::GotBaseRelative, RelocationEncoding::S390xDbl, 32) => {
                                elf::R_390_GOTPCDBL
                            }
                            (RelocationKind::Elf(x), _, _) => x,
                            _ => {
                                return Err(Error(format!("unimplemented relocation {:?}", reloc)));
                            }
                        },
                        _ => {
                            return Err(Error(format!(
                                "unimplemented architecture {:?}",
                                self.architecture
                            )));
                        }
                    };
                    let r_sym = symbol_offsets[reloc.symbol.0].index as u32;
                    elf.write_rel(
                        buffer,
                        is_rela,
                        Rel {
                            r_offset: reloc.offset,
                            r_sym,
                            r_type,
                            r_addend: reloc.addend,
                        },
                    );
                }
            }
        }

        // Write shstrtab section.
        debug_assert_eq!(shstrtab_offset, buffer.len());
        buffer.extend(&shstrtab_data);

        // Write section headers.
        write_align(buffer, pointer_align);
        debug_assert_eq!(e_shoff, buffer.len());
        elf.write_section_header(
            buffer,
            SectionHeader {
                sh_name: 0,
                sh_type: 0,
                sh_flags: 0,
                sh_addr: 0,
                sh_offset: 0,
                sh_size: if section_num >= elf::SHN_LORESERVE as usize {
                    section_num as u64
                } else {
                    0
                },
                sh_link: if shstrtab_index >= elf::SHN_LORESERVE as usize {
                    shstrtab_index as u32
                } else {
                    0
                },
                // TODO: e_phnum overflow
                sh_info: 0,
                sh_addralign: 0,
                sh_entsize: 0,
            },
        );
        for (index, comdat) in self.comdats.iter().enumerate() {
            let sh_name = comdat_offsets[index]
                .str_id
                .map(|id| shstrtab.get_offset(id))
                .unwrap_or(0) as u32;
            elf.write_section_header(
                buffer,
                SectionHeader {
                    sh_name,
                    sh_type: elf::SHT_GROUP,
                    sh_flags: 0,
                    sh_addr: 0,
                    sh_offset: comdat_offsets[index].offset as u64,
                    sh_size: comdat_offsets[index].len as u64,
                    sh_link: symtab_index as u32,
                    sh_info: symbol_offsets[comdat.symbol.0].index as u32,
                    sh_addralign: 4,
                    sh_entsize: 4,
                },
            );
        }
        for (index, section) in self.sections.iter().enumerate() {
            let sh_type = match section.kind {
                SectionKind::UninitializedData | SectionKind::UninitializedTls => elf::SHT_NOBITS,
                SectionKind::Note => elf::SHT_NOTE,
                _ => elf::SHT_PROGBITS,
            };
            let sh_flags = if let SectionFlags::Elf { sh_flags } = section.flags {
                sh_flags
            } else {
                match section.kind {
                    SectionKind::Text => elf::SHF_ALLOC | elf::SHF_EXECINSTR,
                    SectionKind::Data => elf::SHF_ALLOC | elf::SHF_WRITE,
                    SectionKind::Tls => elf::SHF_ALLOC | elf::SHF_WRITE | elf::SHF_TLS,
                    SectionKind::UninitializedData => elf::SHF_ALLOC | elf::SHF_WRITE,
                    SectionKind::UninitializedTls => elf::SHF_ALLOC | elf::SHF_WRITE | elf::SHF_TLS,
                    SectionKind::ReadOnlyData => elf::SHF_ALLOC,
                    SectionKind::ReadOnlyString => {
                        elf::SHF_ALLOC | elf::SHF_STRINGS | elf::SHF_MERGE
                    }
                    SectionKind::OtherString => elf::SHF_STRINGS | elf::SHF_MERGE,
                    SectionKind::Other
                    | SectionKind::Debug
                    | SectionKind::Metadata
                    | SectionKind::Linker
                    | SectionKind::Note => 0,
                    SectionKind::Unknown | SectionKind::Common | SectionKind::TlsVariables => {
                        return Err(Error(format!(
                            "unimplemented section `{}` kind {:?}",
                            section.name().unwrap_or(""),
                            section.kind
                        )));
                    }
                }
                .into()
            };
            // TODO: not sure if this is correct, maybe user should determine this
            let sh_entsize = match section.kind {
                SectionKind::ReadOnlyString | SectionKind::OtherString => 1,
                _ => 0,
            };
            let sh_name = section_offsets[index]
                .str_id
                .map(|id| shstrtab.get_offset(id))
                .unwrap_or(0) as u32;
            elf.write_section_header(
                buffer,
                SectionHeader {
                    sh_name,
                    sh_type,
                    sh_flags,
                    sh_addr: 0,
                    sh_offset: section_offsets[index].offset as u64,
                    sh_size: section.size,
                    sh_link: 0,
                    sh_info: 0,
                    sh_addralign: section.align,
                    sh_entsize,
                },
            );

            if !section.relocations.is_empty() {
                let sh_name = section_offsets[index]
                    .reloc_str_id
                    .map(|id| shstrtab.get_offset(id))
                    .unwrap_or(0);
                elf.write_section_header(
                    buffer,
                    SectionHeader {
                        sh_name: sh_name as u32,
                        sh_type: if is_rela { elf::SHT_RELA } else { elf::SHT_REL },
                        sh_flags: elf::SHF_INFO_LINK.into(),
                        sh_addr: 0,
                        sh_offset: section_offsets[index].reloc_offset as u64,
                        sh_size: section_offsets[index].reloc_len as u64,
                        sh_link: symtab_index as u32,
                        sh_info: section_offsets[index].index as u32,
                        sh_addralign: pointer_align as u64,
                        sh_entsize: elf.rel_size(is_rela) as u64,
                    },
                );
            }
        }

        // Write symtab section header.
        elf.write_section_header(
            buffer,
            SectionHeader {
                sh_name: shstrtab.get_offset(symtab_str_id) as u32,
                sh_type: elf::SHT_SYMTAB,
                sh_flags: 0,
                sh_addr: 0,
                sh_offset: symtab_offset as u64,
                sh_size: symtab_len as u64,
                sh_link: strtab_index as u32,
                sh_info: symtab_count_local as u32,
                sh_addralign: pointer_align as u64,
                sh_entsize: elf.symbol_size() as u64,
            },
        );

        // Write symtab_shndx section header.
        if need_symtab_shndx {
            elf.write_section_header(
                buffer,
                SectionHeader {
                    sh_name: shstrtab.get_offset(symtab_shndx_str_id.unwrap()) as u32,
                    sh_type: elf::SHT_SYMTAB_SHNDX,
                    sh_flags: 0,
                    sh_addr: 0,
                    sh_offset: symtab_shndx_offset as u64,
                    sh_size: symtab_shndx_len as u64,
                    sh_link: symtab_index as u32,
                    sh_info: symtab_count_local as u32,
                    sh_addralign: 4,
                    sh_entsize: 4,
                },
            );
        }

        // Write strtab section header.
        elf.write_section_header(
            buffer,
            SectionHeader {
                sh_name: shstrtab.get_offset(strtab_str_id) as u32,
                sh_type: elf::SHT_STRTAB,
                sh_flags: 0,
                sh_addr: 0,
                sh_offset: strtab_offset as u64,
                sh_size: strtab_data.len() as u64,
                sh_link: 0,
                sh_info: 0,
                sh_addralign: 1,
                sh_entsize: 0,
            },
        );

        // Write shstrtab section header.
        elf.write_section_header(
            buffer,
            SectionHeader {
                sh_name: shstrtab.get_offset(shstrtab_str_id) as u32,
                sh_type: elf::SHT_STRTAB,
                sh_flags: 0,
                sh_addr: 0,
                sh_offset: shstrtab_offset as u64,
                sh_size: shstrtab_data.len() as u64,
                sh_link: 0,
                sh_info: 0,
                sh_addralign: 1,
                sh_entsize: 0,
            },
        );

        debug_assert_eq!(offset, buffer.len());

        Ok(())
    }
}

/// Native endian version of `FileHeader64`.
struct FileHeader {
    e_ident: elf::Ident,
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

/// Native endian version of `SectionHeader64`.
struct SectionHeader {
    sh_name: u32,
    sh_type: u32,
    sh_flags: u64,
    sh_addr: u64,
    sh_offset: u64,
    sh_size: u64,
    sh_link: u32,
    sh_info: u32,
    sh_addralign: u64,
    sh_entsize: u64,
}

/// Native endian version of `Sym64`.
struct Sym {
    st_name: u32,
    st_info: u8,
    st_other: u8,
    st_shndx: u16,
    st_value: u64,
    st_size: u64,
}

/// Unified native endian version of `Rel*`.
struct Rel {
    r_offset: u64,
    r_sym: u32,
    r_type: u32,
    r_addend: i64,
}

trait Elf {
    fn file_header_size(&self) -> usize;
    fn section_header_size(&self) -> usize;
    fn symbol_size(&self) -> usize;
    fn rel_size(&self, is_rela: bool) -> usize;
    fn write_file_header(&self, buffer: &mut dyn WritableBuffer, section: FileHeader);
    fn write_section_header(&self, buffer: &mut dyn WritableBuffer, section: SectionHeader);
    fn write_symbol(&self, buffer: &mut dyn WritableBuffer, symbol: Sym);
    fn write_rel(&self, buffer: &mut dyn WritableBuffer, is_rela: bool, rel: Rel);
}

struct Elf32<E> {
    endian: E,
}

impl<E: Endian> Elf for Elf32<E> {
    fn file_header_size(&self) -> usize {
        mem::size_of::<elf::FileHeader32<E>>()
    }

    fn section_header_size(&self) -> usize {
        mem::size_of::<elf::SectionHeader32<E>>()
    }

    fn symbol_size(&self) -> usize {
        mem::size_of::<elf::Sym32<E>>()
    }

    fn rel_size(&self, is_rela: bool) -> usize {
        if is_rela {
            mem::size_of::<elf::Rela32<E>>()
        } else {
            mem::size_of::<elf::Rel32<E>>()
        }
    }

    fn write_file_header(&self, buffer: &mut dyn WritableBuffer, file: FileHeader) {
        let endian = self.endian;
        let file = elf::FileHeader32 {
            e_ident: file.e_ident,
            e_type: U16::new(endian, file.e_type),
            e_machine: U16::new(endian, file.e_machine),
            e_version: U32::new(endian, file.e_version),
            e_entry: U32::new(endian, file.e_entry as u32),
            e_phoff: U32::new(endian, file.e_phoff as u32),
            e_shoff: U32::new(endian, file.e_shoff as u32),
            e_flags: U32::new(endian, file.e_flags),
            e_ehsize: U16::new(endian, file.e_ehsize),
            e_phentsize: U16::new(endian, file.e_phentsize),
            e_phnum: U16::new(endian, file.e_phnum),
            e_shentsize: U16::new(endian, file.e_shentsize),
            e_shnum: U16::new(endian, file.e_shnum),
            e_shstrndx: U16::new(endian, file.e_shstrndx),
        };
        buffer.extend(bytes_of(&file));
    }

    fn write_section_header(&self, buffer: &mut dyn WritableBuffer, section: SectionHeader) {
        let endian = self.endian;
        let section = elf::SectionHeader32 {
            sh_name: U32::new(endian, section.sh_name),
            sh_type: U32::new(endian, section.sh_type),
            sh_flags: U32::new(endian, section.sh_flags as u32),
            sh_addr: U32::new(endian, section.sh_addr as u32),
            sh_offset: U32::new(endian, section.sh_offset as u32),
            sh_size: U32::new(endian, section.sh_size as u32),
            sh_link: U32::new(endian, section.sh_link),
            sh_info: U32::new(endian, section.sh_info),
            sh_addralign: U32::new(endian, section.sh_addralign as u32),
            sh_entsize: U32::new(endian, section.sh_entsize as u32),
        };
        buffer.extend(bytes_of(&section));
    }

    fn write_symbol(&self, buffer: &mut dyn WritableBuffer, symbol: Sym) {
        let endian = self.endian;
        let symbol = elf::Sym32 {
            st_name: U32::new(endian, symbol.st_name),
            st_info: symbol.st_info,
            st_other: symbol.st_other,
            st_shndx: U16::new(endian, symbol.st_shndx),
            st_value: U32::new(endian, symbol.st_value as u32),
            st_size: U32::new(endian, symbol.st_size as u32),
        };
        buffer.extend(bytes_of(&symbol));
    }

    fn write_rel(&self, buffer: &mut dyn WritableBuffer, is_rela: bool, rel: Rel) {
        let endian = self.endian;
        if is_rela {
            let rel = elf::Rela32 {
                r_offset: U32::new(endian, rel.r_offset as u32),
                r_info: elf::Rel32::r_info(endian, rel.r_sym, rel.r_type as u8),
                r_addend: I32::new(endian, rel.r_addend as i32),
            };
            buffer.extend(bytes_of(&rel));
        } else {
            let rel = elf::Rel32 {
                r_offset: U32::new(endian, rel.r_offset as u32),
                r_info: elf::Rel32::r_info(endian, rel.r_sym, rel.r_type as u8),
            };
            buffer.extend(bytes_of(&rel));
        }
    }
}

struct Elf64<E> {
    endian: E,
}

impl<E: Endian> Elf for Elf64<E> {
    fn file_header_size(&self) -> usize {
        mem::size_of::<elf::FileHeader64<E>>()
    }

    fn section_header_size(&self) -> usize {
        mem::size_of::<elf::SectionHeader64<E>>()
    }

    fn symbol_size(&self) -> usize {
        mem::size_of::<elf::Sym64<E>>()
    }

    fn rel_size(&self, is_rela: bool) -> usize {
        if is_rela {
            mem::size_of::<elf::Rela64<E>>()
        } else {
            mem::size_of::<elf::Rel64<E>>()
        }
    }

    fn write_file_header(&self, buffer: &mut dyn WritableBuffer, file: FileHeader) {
        let endian = self.endian;
        let file = elf::FileHeader64 {
            e_ident: file.e_ident,
            e_type: U16::new(endian, file.e_type),
            e_machine: U16::new(endian, file.e_machine),
            e_version: U32::new(endian, file.e_version),
            e_entry: U64::new(endian, file.e_entry),
            e_phoff: U64::new(endian, file.e_phoff),
            e_shoff: U64::new(endian, file.e_shoff),
            e_flags: U32::new(endian, file.e_flags),
            e_ehsize: U16::new(endian, file.e_ehsize),
            e_phentsize: U16::new(endian, file.e_phentsize),
            e_phnum: U16::new(endian, file.e_phnum),
            e_shentsize: U16::new(endian, file.e_shentsize),
            e_shnum: U16::new(endian, file.e_shnum),
            e_shstrndx: U16::new(endian, file.e_shstrndx),
        };
        buffer.extend(bytes_of(&file))
    }

    fn write_section_header(&self, buffer: &mut dyn WritableBuffer, section: SectionHeader) {
        let endian = self.endian;
        let section = elf::SectionHeader64 {
            sh_name: U32::new(endian, section.sh_name),
            sh_type: U32::new(endian, section.sh_type),
            sh_flags: U64::new(endian, section.sh_flags),
            sh_addr: U64::new(endian, section.sh_addr),
            sh_offset: U64::new(endian, section.sh_offset),
            sh_size: U64::new(endian, section.sh_size),
            sh_link: U32::new(endian, section.sh_link),
            sh_info: U32::new(endian, section.sh_info),
            sh_addralign: U64::new(endian, section.sh_addralign),
            sh_entsize: U64::new(endian, section.sh_entsize),
        };
        buffer.extend(bytes_of(&section));
    }

    fn write_symbol(&self, buffer: &mut dyn WritableBuffer, symbol: Sym) {
        let endian = self.endian;
        let symbol = elf::Sym64 {
            st_name: U32::new(endian, symbol.st_name),
            st_info: symbol.st_info,
            st_other: symbol.st_other,
            st_shndx: U16::new(endian, symbol.st_shndx),
            st_value: U64::new(endian, symbol.st_value),
            st_size: U64::new(endian, symbol.st_size),
        };
        buffer.extend(bytes_of(&symbol));
    }

    fn write_rel(&self, buffer: &mut dyn WritableBuffer, is_rela: bool, rel: Rel) {
        let endian = self.endian;
        if is_rela {
            let rel = elf::Rela64 {
                r_offset: U64::new(endian, rel.r_offset),
                r_info: elf::Rela64::r_info(endian, rel.r_sym, rel.r_type),
                r_addend: I64::new(endian, rel.r_addend),
            };
            buffer.extend(bytes_of(&rel));
        } else {
            let rel = elf::Rel64 {
                r_offset: U64::new(endian, rel.r_offset),
                r_info: elf::Rel64::r_info(endian, rel.r_sym, rel.r_type),
            };
            buffer.extend(bytes_of(&rel));
        }
    }
}
