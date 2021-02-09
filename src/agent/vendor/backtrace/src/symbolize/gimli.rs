//! Support for symbolication using the `gimli` crate on crates.io
//!
//! This is the default symbolication implementation for Rust.

use self::gimli::read::EndianSlice;
use self::gimli::NativeEndian as Endian;
use self::mmap::Mmap;
use self::stash::Stash;
use super::BytesOrWideString;
use super::ResolveWhat;
use super::SymbolName;
use addr2line::gimli;
use core::convert::TryInto;
use core::mem;
use core::u32;
use libc::c_void;
use mystd::ffi::OsString;
use mystd::fs::File;
use mystd::path::Path;
use mystd::prelude::v1::*;

#[cfg(backtrace_in_libstd)]
mod mystd {
    pub use crate::*;
}
#[cfg(not(backtrace_in_libstd))]
extern crate std as mystd;

cfg_if::cfg_if! {
    if #[cfg(windows)] {
        #[path = "gimli/mmap_windows.rs"]
        mod mmap;
    } else if #[cfg(any(
        target_os = "android",
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "openbsd",
        target_os = "solaris",
    ))] {
        #[path = "gimli/mmap_unix.rs"]
        mod mmap;
    } else {
        #[path = "gimli/mmap_fake.rs"]
        mod mmap;
    }
}

mod stash;

const MAPPINGS_CACHE_SIZE: usize = 4;

struct Mapping {
    // 'static lifetime is a lie to hack around lack of support for self-referential structs.
    cx: Context<'static>,
    _map: Mmap,
    _stash: Stash,
}

impl Mapping {
    fn mk<F>(data: Mmap, mk: F) -> Option<Mapping>
    where
        F: for<'a> Fn(&'a [u8], &'a Stash) -> Option<Context<'a>>,
    {
        let stash = Stash::new();
        let cx = mk(&data, &stash)?;
        Some(Mapping {
            // Convert to 'static lifetimes since the symbols should
            // only borrow `map` and `stash` and we're preserving them below.
            cx: unsafe { core::mem::transmute::<Context<'_>, Context<'static>>(cx) },
            _map: data,
            _stash: stash,
        })
    }
}

struct Context<'a> {
    dwarf: addr2line::Context<EndianSlice<'a, Endian>>,
    object: Object<'a>,
}

impl<'data> Context<'data> {
    fn new(stash: &'data Stash, object: Object<'data>) -> Option<Context<'data>> {
        fn load_section<'data, S>(stash: &'data Stash, obj: &Object<'data>) -> S
        where
            S: gimli::Section<gimli::EndianSlice<'data, Endian>>,
        {
            let data = obj.section(stash, S::section_name()).unwrap_or(&[]);
            S::from(EndianSlice::new(data, Endian))
        }

        let dwarf = addr2line::Context::from_sections(
            load_section(stash, &object),
            load_section(stash, &object),
            load_section(stash, &object),
            load_section(stash, &object),
            load_section(stash, &object),
            load_section(stash, &object),
            load_section(stash, &object),
            load_section(stash, &object),
            load_section(stash, &object),
            gimli::EndianSlice::new(&[], Endian),
        )
        .ok()?;
        Some(Context { dwarf, object })
    }
}

fn mmap(path: &Path) -> Option<Mmap> {
    let file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len().try_into().ok()?;
    unsafe { Mmap::map(&file, len) }
}

cfg_if::cfg_if! {
    if #[cfg(windows)] {
        use core::mem::MaybeUninit;
        use super::super::windows::*;
        use mystd::os::windows::prelude::*;
        use alloc::vec;

        mod coff;
        use self::coff::Object;

        // For loading native libraries on Windows, see some discussion on
        // rust-lang/rust#71060 for the various strategies here.
        fn native_libraries() -> Vec<Library> {
            let mut ret = Vec::new();
            unsafe { add_loaded_images(&mut ret); }
            return ret;
        }

        unsafe fn add_loaded_images(ret: &mut Vec<Library>) {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, 0);
            if snap == INVALID_HANDLE_VALUE {
                return;
            }

            let mut me = MaybeUninit::<MODULEENTRY32W>::zeroed().assume_init();
            me.dwSize = mem::size_of_val(&me) as DWORD;
            if Module32FirstW(snap, &mut me) == TRUE {
                loop {
                    if let Some(lib) = load_library(&me) {
                        ret.push(lib);
                    }

                    if Module32NextW(snap, &mut me) != TRUE {
                        break;
                    }
                }

            }

            CloseHandle(snap);
        }

        unsafe fn load_library(me: &MODULEENTRY32W) -> Option<Library> {
            let pos = me
                .szExePath
                .iter()
                .position(|i| *i == 0)
                .unwrap_or(me.szExePath.len());
            let name = OsString::from_wide(&me.szExePath[..pos]);

            // MinGW libraries currently don't support ASLR
            // (rust-lang/rust#16514), but DLLs can still be relocated around in
            // the address space. It appears that addresses in debug info are
            // all as-if this library was loaded at its "image base", which is a
            // field in its COFF file headers. Since this is what debuginfo
            // seems to list we parse the symbol table and store addresses as if
            // the library was loaded at "image base" as well.
            //
            // The library may not be loaded at "image base", however.
            // (presumably something else may be loaded there?) This is where
            // the `bias` field comes into play, and we need to figure out the
            // value of `bias` here. Unfortunately though it's not clear how to
            // acquire this from a loaded module. What we do have, however, is
            // the actual load address (`modBaseAddr`).
            //
            // As a bit of a cop-out for now we mmap the file, read the file
            // header information, then drop the mmap. This is wasteful because
            // we'll probably reopen the mmap later, but this should work well
            // enough for now.
            //
            // Once we have the `image_base` (desired load location) and the
            // `base_addr` (actual load location) we can fill in the `bias`
            // (difference between the actual and desired) and then the stated
            // address of each segment is the `image_base` since that's what the
            // file says.
            //
            // For now it appears that unlike ELF/MachO we can make do with one
            // segment per library, using `modBaseSize` as the whole size.
            let mmap = mmap(name.as_ref())?;
            let image_base = coff::get_image_base(&mmap)?;
            let base_addr = me.modBaseAddr as usize;
            Some(Library {
                name,
                bias: base_addr.wrapping_sub(image_base),
                segments: vec![LibrarySegment {
                    stated_virtual_memory_address: image_base,
                    len: me.modBaseSize as usize,
                }],
            })
        }
    } else if #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
    ))] {
        // macOS uses the Mach-O file format and uses DYLD-specific APIs to
        // load a list of native libraries that are part of the application.

        use mystd::os::unix::prelude::*;
        use mystd::ffi::{OsStr, CStr};

        mod macho;
        use self::macho::Object;

        #[allow(deprecated)]
        fn native_libraries() -> Vec<Library> {
            let mut ret = Vec::new();
            let images = unsafe { libc::_dyld_image_count() };
            for i in 0..images {
                ret.extend(native_library(i));
            }
            return ret;
        }

        #[allow(deprecated)]
        fn native_library(i: u32) -> Option<Library> {
            use object::macho;
            use object::read::macho::{MachHeader, Segment};
            use object::{Bytes, NativeEndian};

            // Fetch the name of this library which corresponds to the path of
            // where to load it as well.
            let name = unsafe {
                let name = libc::_dyld_get_image_name(i);
                if name.is_null() {
                    return None;
                }
                CStr::from_ptr(name)
            };

            // Load the image header of this library and delegate to `object` to
            // parse all the load commands so we can figure out all the segments
            // involved here.
            let (mut load_commands, endian) = unsafe {
                let header = libc::_dyld_get_image_header(i);
                if header.is_null() {
                    return None;
                }
                match (*header).magic {
                    macho::MH_MAGIC => {
                        let endian = NativeEndian;
                        let header = &*(header as *const macho::MachHeader32<NativeEndian>);
                        let data = core::slice::from_raw_parts(
                            header as *const _ as *const u8,
                            mem::size_of_val(header) + header.sizeofcmds.get(endian) as usize
                        );
                        (header.load_commands(endian, Bytes(data)).ok()?, endian)
                    }
                    macho::MH_MAGIC_64 => {
                        let endian = NativeEndian;
                        let header = &*(header as *const macho::MachHeader64<NativeEndian>);
                        let data = core::slice::from_raw_parts(
                            header as *const _ as *const u8,
                            mem::size_of_val(header) + header.sizeofcmds.get(endian) as usize
                        );
                        (header.load_commands(endian, Bytes(data)).ok()?, endian)
                    }
                    _ => return None,
                }
            };

            // Iterate over the segments and register known regions for segments
            // that we find. Additionally record information bout text segments
            // for processing later, see comments below.
            let mut segments = Vec::new();
            let mut first_text = 0;
            let mut text_fileoff_zero = false;
            while let Some(cmd) = load_commands.next().ok()? {
                if let Some((seg, _)) = cmd.segment_32().ok()? {
                    if seg.name() == b"__TEXT" {
                        first_text = segments.len();
                        if seg.fileoff(endian) == 0 && seg.filesize(endian) > 0 {
                            text_fileoff_zero = true;
                        }
                    }
                    segments.push(LibrarySegment {
                        len: seg.vmsize(endian).try_into().ok()?,
                        stated_virtual_memory_address: seg.vmaddr(endian).try_into().ok()?,
                    });
                }
                if let Some((seg, _)) = cmd.segment_64().ok()? {
                    if seg.name() == b"__TEXT" {
                        first_text = segments.len();
                        if seg.fileoff(endian) == 0 && seg.filesize(endian) > 0 {
                            text_fileoff_zero = true;
                        }
                    }
                    segments.push(LibrarySegment {
                        len: seg.vmsize(endian).try_into().ok()?,
                        stated_virtual_memory_address: seg.vmaddr(endian).try_into().ok()?,
                    });
                }
            }

            // Determine the "slide" for this library which ends up being the
            // bias we use to figure out where in memory objects are loaded.
            // This is a bit of a weird computation though and is the result of
            // trying a few things in the wild and seeing what sticks.
            //
            // The general idea is that the `bias` plus a segment's
            // `stated_virtual_memory_address` is going to be where in the
            // actual address space the segment resides. The other thing we rely
            // on though is that a real address minus the `bias` is the index to
            // look up in the symbol table and debuginfo.
            //
            // It turns out, though, that for system loaded libraries these
            // calculations are incorrect. For native executables, however, it
            // appears correct. Lifting some logic from LLDB's source it has
            // some special-casing for the first `__TEXT` section loaded from
            // file offset 0 with a nonzero size. For whatever reason when this
            // is present it appears to mean that the symbol table is relative
            // to just the vmaddr slide for the library. If it's *not* present
            // then the symbol table is relative to the the vmaddr slide plus
            // the segment's stated address.
            //
            // To handle this situation if we *don't* find a text section at
            // file offset zero then we increase the bias by the first text
            // sections's stated address and decrease all stated addresses by
            // that amount as well. That way the symbol table is always appears
            // relative to the library's bias amount. This appears to have the
            // right results for symbolizing via the symbol table.
            //
            // Honestly I'm not entirely sure whether this is right or if
            // there's something else that should indicate how to do this. For
            // now though this seems to work well enough (?) and we should
            // always be able to tweak this over time if necessary.
            //
            // For some more information see #318
            let mut slide = unsafe { libc::_dyld_get_image_vmaddr_slide(i) as usize };
            if !text_fileoff_zero {
                let adjust = segments[first_text].stated_virtual_memory_address;
                for segment in segments.iter_mut() {
                    segment.stated_virtual_memory_address -= adjust;
                }
                slide += adjust;
            }

            Some(Library {
                name: OsStr::from_bytes(name.to_bytes()).to_owned(),
                segments,
                bias: slide,
            })
        }
    } else if #[cfg(any(
        target_os = "linux",
        target_os = "fuchsia",
    ))] {
        // Other Unix (e.g. Linux) platforms use ELF as an object file format
        // and typically implement an API called `dl_iterate_phdr` to load
        // native libraries.

        use mystd::os::unix::prelude::*;
        use mystd::ffi::{OsStr, CStr};

        mod elf;
        use self::elf::Object;

        fn native_libraries() -> Vec<Library> {
            let mut ret = Vec::new();
            unsafe {
                libc::dl_iterate_phdr(Some(callback), &mut ret as *mut Vec<_> as *mut _);
            }
            return ret;
        }

        // `info` should be a valid pointers.
        // `vec` should be a valid pointer to a `std::Vec`.
        unsafe extern "C" fn callback(
            info: *mut libc::dl_phdr_info,
            _size: libc::size_t,
            vec: *mut libc::c_void,
        ) -> libc::c_int {
            let info = &*info;
            let libs = &mut *(vec as *mut Vec<Library>);
            let is_main_prog = info.dlpi_name.is_null() || *info.dlpi_name == 0;
            let name = if is_main_prog {
                if libs.is_empty() {
                    mystd::env::current_exe().map(|e| e.into()).unwrap_or_default()
                } else {
                    OsString::new()
                }
            } else {
                let bytes = CStr::from_ptr(info.dlpi_name).to_bytes();
                OsStr::from_bytes(bytes).to_owned()
            };
            let headers = core::slice::from_raw_parts(info.dlpi_phdr, info.dlpi_phnum as usize);
            libs.push(Library {
                name,
                segments: headers
                    .iter()
                    .map(|header| LibrarySegment {
                        len: (*header).p_memsz as usize,
                        stated_virtual_memory_address: (*header).p_vaddr as usize,
                    })
                    .collect(),
                bias: info.dlpi_addr as usize,
            });
            0
        }
    } else if #[cfg(target_env = "libnx")] {
        // DevkitA64 doesn't natively support debug info, but the build system will place debug
        // info at the path `romfs:/debug_info.elf`.
        mod elf;
        use self::elf::Object;

        fn native_libraries() -> Vec<Library> {
            extern "C" {
                static __start__: u8;
            }

            let bias = unsafe { &__start__ } as *const u8 as usize;

            let mut ret = Vec::new();
            let mut segments = Vec::new();
            segments.push(LibrarySegment {
                stated_virtual_memory_address: 0,
                len: usize::max_value() - bias,
            });

            let path = "romfs:/debug_info.elf";
            ret.push(Library {
                name: path.into(),
                segments,
                bias,
            });

            ret
        }
    } else {
        // Everything else should use ELF, but doesn't know how to load native
        // libraries.

        use mystd::os::unix::prelude::*;
        mod elf;
        use self::elf::Object;

        fn native_libraries() -> Vec<Library> {
            Vec::new()
        }
    }
}

#[derive(Default)]
struct Cache {
    /// All known shared libraries that have been loaded.
    libraries: Vec<Library>,

    /// Mappings cache where we retain parsed dwarf information.
    ///
    /// This list has a fixed capacity for its entire liftime which never
    /// increases. The `usize` element of each pair is an index into `libraries`
    /// above where `usize::max_value()` represents the current executable. The
    /// `Mapping` is corresponding parsed dwarf information.
    ///
    /// Note that this is basically an LRU cache and we'll be shifting things
    /// around in here as we symbolize addresses.
    mappings: Vec<(usize, Mapping)>,
}

struct Library {
    name: OsString,
    /// Segments of this library loaded into memory, and where they're loaded.
    segments: Vec<LibrarySegment>,
    /// The "bias" of this library, typically where it's loaded into memory.
    /// This value is added to each segment's stated address to get the actual
    /// virtual memory address that the segment is loaded into. Additionally
    /// this bias is subtracted from real virtual memory addresses to index into
    /// debuginfo and the symbol table.
    bias: usize,
}

struct LibrarySegment {
    /// The stated address of this segment in the object file. This is not
    /// actually where the segment is loaded, but rather this address plus the
    /// containing library's `bias` is where to find it.
    stated_virtual_memory_address: usize,
    /// The size of ths segment in memory.
    len: usize,
}

// unsafe because this is required to be externally synchronized
pub unsafe fn clear_symbol_cache() {
    Cache::with_global(|cache| cache.mappings.clear());
}

impl Cache {
    fn new() -> Cache {
        Cache {
            mappings: Vec::with_capacity(MAPPINGS_CACHE_SIZE),
            libraries: native_libraries(),
        }
    }

    // unsafe because this is required to be externally synchronized
    unsafe fn with_global(f: impl FnOnce(&mut Self)) {
        // A very small, very simple LRU cache for debug info mappings.
        //
        // The hit rate should be very high, since the typical stack doesn't cross
        // between many shared libraries.
        //
        // The `addr2line::Context` structures are pretty expensive to create. Its
        // cost is expected to be amortized by subsequent `locate` queries, which
        // leverage the structures built when constructing `addr2line::Context`s to
        // get nice speedups. If we didn't have this cache, that amortization would
        // never happen, and symbolicating backtraces would be ssssllllooooowwww.
        static mut MAPPINGS_CACHE: Option<Cache> = None;

        f(MAPPINGS_CACHE.get_or_insert_with(|| Cache::new()))
    }

    fn avma_to_svma(&self, addr: *const u8) -> Option<(usize, *const u8)> {
        self.libraries
            .iter()
            .enumerate()
            .filter_map(|(i, lib)| {
                // First up, test if this `lib` has any segment containing the
                // `addr` (handling relocation). If this check passes then we
                // can continue below and actually translate the address.
                //
                // Note that we're using `wrapping_add` here to avoid overflow
                // checks. It's been seen in the wild that the SVMA + bias
                // computation overflows. It seems a bit odd that would happen
                // but there's not a huge amount we can do about it other than
                // probably just ignore those segments since they're likely
                // pointing off into space. This originally came up in
                // rust-lang/backtrace-rs#329.
                if !lib.segments.iter().any(|s| {
                    let svma = s.stated_virtual_memory_address;
                    let start = svma.wrapping_add(lib.bias);
                    let end = start.wrapping_add(s.len);
                    let address = addr as usize;
                    start <= address && address < end
                }) {
                    return None;
                }

                // Now that we know `lib` contains `addr`, we can offset with
                // the bias to find the stated virutal memory address.
                let svma = (addr as usize).wrapping_sub(lib.bias);
                Some((i, svma as *const u8))
            })
            .next()
    }

    fn mapping_for_lib<'a>(&'a mut self, lib: usize) -> Option<&'a mut Context<'a>> {
        let idx = self.mappings.iter().position(|(idx, _)| *idx == lib);

        // Invariant: after this conditional completes without early returning
        // from an error, the cache entry for this path is at index 0.

        if let Some(idx) = idx {
            // When the mapping is already in the cache, move it to the front.
            if idx != 0 {
                let entry = self.mappings.remove(idx);
                self.mappings.insert(0, entry);
            }
        } else {
            // When the mapping is not in the cache, create a new mapping,
            // insert it into the front of the cache, and evict the oldest cache
            // entry if necessary.
            let name = &self.libraries[lib].name;
            let mapping = Mapping::new(name.as_ref())?;

            if self.mappings.len() == MAPPINGS_CACHE_SIZE {
                self.mappings.pop();
            }

            self.mappings.insert(0, (lib, mapping));
        }

        let cx: &'a mut Context<'static> = &mut self.mappings[0].1.cx;
        // don't leak the `'static` lifetime, make sure it's scoped to just
        // ourselves
        Some(unsafe { mem::transmute::<&'a mut Context<'static>, &'a mut Context<'a>>(cx) })
    }
}

pub unsafe fn resolve(what: ResolveWhat<'_>, cb: &mut dyn FnMut(&super::Symbol)) {
    let addr = what.address_or_ip();
    let mut call = |sym: Symbol<'_>| {
        // Extend the lifetime of `sym` to `'static` since we are unfortunately
        // required to here, but it's ony ever going out as a reference so no
        // reference to it should be persisted beyond this frame anyway.
        let sym = mem::transmute::<Symbol<'_>, Symbol<'static>>(sym);
        (cb)(&super::Symbol { inner: sym });
    };

    Cache::with_global(|cache| {
        let (lib, addr) = match cache.avma_to_svma(addr as *const u8) {
            Some(pair) => pair,
            None => return,
        };

        // Finally, get a cached mapping or create a new mapping for this file, and
        // evaluate the DWARF info to find the file/line/name for this address.
        let cx = match cache.mapping_for_lib(lib) {
            Some(cx) => cx,
            None => return,
        };
        let mut any_frames = false;
        if let Ok(mut frames) = cx.dwarf.find_frames(addr as u64) {
            while let Ok(Some(frame)) = frames.next() {
                any_frames = true;
                call(Symbol::Frame {
                    addr: addr as *mut c_void,
                    location: frame.location,
                    name: frame.function.map(|f| f.name.slice()),
                });
            }
        }
        if !any_frames {
            if let Some((object_cx, object_addr)) = cx.object.search_object_map(addr as u64) {
                if let Ok(mut frames) = object_cx.dwarf.find_frames(object_addr) {
                    while let Ok(Some(frame)) = frames.next() {
                        any_frames = true;
                        call(Symbol::Frame {
                            addr: addr as *mut c_void,
                            location: frame.location,
                            name: frame.function.map(|f| f.name.slice()),
                        });
                    }
                }
            }
        }
        if !any_frames {
            if let Some(name) = cx.object.search_symtab(addr as u64) {
                call(Symbol::Symtab {
                    addr: addr as *mut c_void,
                    name,
                });
            }
        }
    });
}

pub enum Symbol<'a> {
    /// We were able to locate frame information for this symbol, and
    /// `addr2line`'s frame internally has all the nitty gritty details.
    Frame {
        addr: *mut c_void,
        location: Option<addr2line::Location<'a>>,
        name: Option<&'a [u8]>,
    },
    /// Couldn't find debug information, but we found it in the symbol table of
    /// the elf executable.
    Symtab { addr: *mut c_void, name: &'a [u8] },
}

impl Symbol<'_> {
    pub fn name(&self) -> Option<SymbolName<'_>> {
        match self {
            Symbol::Frame { name, .. } => {
                let name = name.as_ref()?;
                Some(SymbolName::new(name))
            }
            Symbol::Symtab { name, .. } => Some(SymbolName::new(name)),
        }
    }

    pub fn addr(&self) -> Option<*mut c_void> {
        match self {
            Symbol::Frame { addr, .. } => Some(*addr),
            Symbol::Symtab { .. } => None,
        }
    }

    pub fn filename_raw(&self) -> Option<BytesOrWideString<'_>> {
        match self {
            Symbol::Frame { location, .. } => {
                let file = location.as_ref()?.file?;
                Some(BytesOrWideString::Bytes(file.as_bytes()))
            }
            Symbol::Symtab { .. } => None,
        }
    }

    pub fn filename(&self) -> Option<&Path> {
        match self {
            Symbol::Frame { location, .. } => {
                let file = location.as_ref()?.file?;
                Some(Path::new(file))
            }
            Symbol::Symtab { .. } => None,
        }
    }

    pub fn lineno(&self) -> Option<u32> {
        match self {
            Symbol::Frame { location, .. } => location.as_ref()?.line,
            Symbol::Symtab { .. } => None,
        }
    }

    pub fn colno(&self) -> Option<u32> {
        match self {
            Symbol::Frame { location, .. } => location.as_ref()?.column,
            Symbol::Symtab { .. } => None,
        }
    }
}
