# Design

## Objectives

- Provide a set of traits for accessing and configuring the physical memory of
  a virtual machine.
- Provide a clean abstraction of the VM memory such that rust-vmm components
  can use it without depending on the implementation details specific to
  different VMMs.

## API Principles

- Define consumer side interfaces to access VM's physical memory.
- Do not define provider side interfaces to supply VM physical memory.

The `vm-memory` crate focuses on defining consumer side interfaces to access
the physical memory of the VM. It does not define how the underlying VM memory
provider is implemented. Lightweight VMMs like
[CrosVM](https://chromium.googlesource.com/chromiumos/platform/crosvm/) and
[Firecracker](https://github.com/firecracker-microvm/firecracker) can make
assumptions about the structure of VM's physical memory and implement a
lightweight backend to access it. For VMMs like [Qemu](https://www.qemu.org/),
a high performance and full functionality backend may be implemented with less
assumptions.

## Architecture

The `vm-memory` is derived from two upstream projects:

- [CrosVM](https://chromium.googlesource.com/chromiumos/platform/crosvm/)
  commit 186eb8b0db644892e8ffba8344efe3492bb2b823
- [Firecracker](https://github.com/firecracker-microvm/firecracker) commit
  80128ea61b305a27df1f751d70415b04b503eae7

The high level abstraction of the VM memory has been heavily refactored to
provide a VMM agnostic interface.

The `vm-memory` crate could be divided into four logic parts as:

- [Abstraction of Address Space](#abstraction-of-address-space)
- [Specialization for Virtual Machine Physical Address Space](#specialization-for-virtual-machine-physical-address-space)
- [Backend Implementation Based on `mmap`](#backend-implementation-based-on-`mmap`)
- [Utilities and helpers](#utilities-and-helpers)

### Address Space Abstraction

The address space abstraction contains traits and implementations for working
with addresses as follows:

- `AddressValue`: stores the raw value of an address. Typically `u32`, `u64` or
   `usize` are used to store the raw value. Pointers such as `*u8`, can not be
   used as an implementation of `AddressValue` because the `Add` and `Sub`
   traits are not implemented for that type.
- `Address`: implementation of `AddressValue`.
- `Bytes`: trait for volatile access to memory. The `Bytes` trait can be
  parameterized with types that represent addresses, in order to enforce that
  addresses are used with the right "kind" of volatile memory.
- `VolatileMemory`: basic implementation of volatile access to memory.
  Implements `Bytes<usize>`.

To make the abstraction as generic as possible, all of above traits only define
methods to access the address space, and they never define methods to manage
(create, delete, insert, remove etc) address spaces. This way, the address
space consumers may be decoupled from the address space provider
(typically a VMM).

### Specialization for Virtual Machine Physical Address Space

The generic address space crates are specialized to access the physical memory
of the VM using the following traits:

- `GuestAddress`: represents a guest physical address (GPA). On ARM64, a
  32-bit VMM/hypervisor can be used to support a 64-bit VM. For simplicity,
  `u64` is used to store the the raw value no matter if it is a 32-bit or
  a 64-bit virtual machine.
- `GuestMemoryRegion`: represents a continuous region of the VM memory.
- `GuestMemory`: represents a collection of `GuestMemoryRegion` objects. The
  main responsibilities of the `GuestMemory` trait are:
  - hide the detail of accessing physical addresses (for example complex
    hierarchical structures).
  - map an address request to a `GuestMemoryRegion` object and relay the
    request to it.
  - handle cases where an access request is spanning two or more
    `GuestMemoryRegion` objects.

The VM memory consumers should only rely on traits and structs defined here to
access VM's physical memory and not on the implementation of the traits.

### Backend Implementation Based on `mmap`

Provides an implementation of the `GuestMemory` trait by mmapping the VM's physical
memory into the current process.

- `MmapRegion`: implementation of mmap a continuous range of physical memory
  with methods for accessing the mapped memory.
- `GuestRegionMmap`: implementation of `GuestMemoryRegion` providing a wrapper
  used to map VM's physical address into a `(mmap_region, offset)` tuple.
- `GuestMemoryMmap`: implementation of `GuestMemory` that manages a collection
  of `GuestRegionMmap` objects for a VM.

One of the main responsibilities of `GuestMemoryMmap` is to handle the use
cases where an access request crosses the memory region boundary. This scenario
may be triggered when memory hotplug is supported. There is a trade-off between
simplicity and code complexity:

- The following pattern currently used in both CrosVM and Firecracker is
  simple, but fails when the request crosses region boundary.

```rust
let guest_memory_mmap: GuestMemoryMmap = ...
let addr: GuestAddress = ...
let buf = &mut [0u8; 5];
let result = guest_memory_mmap.find_region(addr).unwrap().write(buf, addr);
```

- To support requests crossing region boundary, the following update is needed:

```rust
let guest_memory_mmap: GuestMemoryMmap = ...
let addr: GuestAddress = ...
let buf = &mut [0u8; 5];
let result = guest_memory_mmap.write(buf, addr);
```

### Utilities and Helpers

The following utilities and helper traits/macros are imported from the
[crosvm project](https://chromium.googlesource.com/chromiumos/platform/crosvm/)
with minor changes:

- `ByteValued` (originally `DataInit`): types which are safe to be initialized
  from raw data. A type `T` is `ByteValued` if and only if it can be
  initialized by reading its contents from a byte array. This is generally true
  for all plain-old-data structs.  It is notably not true for any type that
  includes a reference.
- `{Le,Be}_{16,32,64}`: explicit endian types useful for embedding in structs
  or reinterpreting data.

## Relationships between Traits, Structs and Types

**Traits**:

- `Address` inherits `AddressValue`
- `GuestMemoryRegion` inherits `Bytes<MemoryRegionAddress, E = Error>`. The
  `Bytes` trait must be implemented.
- `GuestMemory` has a generic implementation of `Bytes<GuestAddress>`.

**Types**:

- `GuestAddress`: `Address<u64>`
- `MemoryRegionAddress`: `Address<u64>`

**Structs**:

- `MmapRegion` implements `VolatileMemory`
- `GuestRegionMmap` implements `Bytes<MemoryRegionAddress> + GuestMemoryRegion`
- `GuestMemoryMmap` implements `GuestMemory`
- `VolatileSlice` implements
  `Bytes<usize, E = volatile_memory::Error> + VolatileMemory`
