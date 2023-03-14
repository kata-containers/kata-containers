# [v0.6.0]

## Changed
- Crate is now using edition 2021.

## Added
- Derived `Eq` for `Error` types and the `PvhBootCapability` enum.

## Fixed
- Fixed a bug in `load_cmdline` due to which the command line was not null
  terminated. This resulted in a change in the `Cmdline` API where instead of
  returning the cmdline as a String, we're now returning it as a `CString` as
  the latter has support for converting it to a null terminated bytes array.
- Fixed an off-by-one error in load_cmdline, where we were doing validations
  on the first address after the command line memory region, instead of the
  last inclusive one of it.

# [v0.5.0]

## Fixed
- [[#104]](https://github.com/rust-vmm/linux-loader/issues/104) Fixed
  the `--no-default-features` not working.

## Changed
- [[#111]](https://github.com/rust-vmm/linux-loader/pull/111) Use
  caret requirements for dependencies.

## Added
- [[#99]](https://github.com/rust-vmm/linux-loader/pull/99) Implement
   `Debug` and `PartialEq` for `CmdLine`.
- [[#100]](https://github.com/rust-vmm/linux-loader/pull/100) Added
   `Clone` derive for `CmdLine`.

# [v0.4.0]

## Fixed

- [[#66]](https://github.com/rust-vmm/linux-loader/issues/66) Fixed potential
  overflow in calls to `align_up`.

## Changed

- [[#62]](https://github.com/rust-vmm/linux-loader/issues/62) The
  `load_cmdline` function now takes as a parameter the crate defined
  `Cmdline` object instead of `Cstr`. This means that customers don't need to
  convert the object before calling into `load_cmdline`.
- [[#83]](https://github.com/rust-vmm/linux-loader/issues/83) Updated the
  vm-memory dependency requirement to the latest version (0.6.0).

## Added

- [[#79]](https://github.com/rust-vmm/linux-loader/pull/79) Implemented
  `From<Cmdline>` for `Vec<u8>`. This replaces the obsolete `Into`
  implementation.

# [v0.3.0]

## Fixed

- Replaced panic condition in `align_up` with returning an Error.
- Fixed potential hang condition in Elf::load caused by arithmetic overflow.
- Disallow overflow when computing the kernel load address when loading ELF.
- Fix unchecked arithmetic in BzImage::load that could lead to undefined
  behavior.


## Added

- Added functions for specifying virtio MMIO devices when building the kernel
  command line.
- Added a function to specify multiple values in `key=values` pairs when
  building the kernel command line.

# [v0.2.0]

## Added

- Added traits and structs for loading ELF (`vmlinux`), big zImage (`bzImage`)
  and PE (`Image`) kernels into guest memory.
- Added traits and structs for writing boot parameters to guest memory.
