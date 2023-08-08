# dbs-address-space

## Design

The `dbs-address-space` crate is an address space manager for virtual machines, which manages memory and MMIO resources resident in the guest physical address space.

Main components are:
- `AddressSpaceRegion`: Struct to maintain configuration information about a guest address region.
```rust
#[derive(Debug, Clone)]
pub struct AddressSpaceRegion {
    /// Type of address space regions.
    pub ty: AddressSpaceRegionType,
    /// Base address of the region in virtual machine's physical address space.
    pub base: GuestAddress,
    /// Size of the address space region.
    pub size: GuestUsize,
    /// Host NUMA node ids assigned to this region.
    pub host_numa_node_id: Option<u32>,

    /// File/offset tuple to back the memory allocation.
    file_offset: Option<FileOffset>,
    /// Mmap permission flags.
    perm_flags: i32,
    /// Hugepage madvise hint.
    ///
    /// It needs 'advise' or 'always' policy in host shmem config.
    is_hugepage: bool,
    /// Hotplug hint.
    is_hotplug: bool,
    /// Anonymous memory hint.
    ///
    /// It should be true for regions with the MADV_DONTFORK flag enabled.
    is_anon: bool,
}
```
- `AddressSpaceBase`: Base implementation to manage guest physical address space, without support of region hotplug.
```rust
#[derive(Clone)]
pub struct AddressSpaceBase {
    regions: Vec<Arc<AddressSpaceRegion>>,
    layout: AddressSpaceLayout,
}
```
- `AddressSpaceBase`: An address space implementation with region hotplug capability.
```rust
/// The `AddressSpace` is a wrapper over [AddressSpaceBase] to support hotplug of
/// address space regions.
#[derive(Clone)]
pub struct AddressSpace {
    state: Arc<ArcSwap<AddressSpaceBase>>,
}
```

## Usage
```rust
// 1. create several memory regions
let reg = Arc::new(
    AddressSpaceRegion::create_default_memory_region(
        GuestAddress(0x100000),
        0x100000,
        None,
        "shmem",
        "",
        false,
        false,
        false,
    )
    .unwrap()
);
let regions = vec![reg];
// 2. create layout (depending on archs)
let layout = AddressSpaceLayout::new(GUEST_PHYS_END, GUEST_MEM_START, GUEST_MEM_END);
// 3. create address space from regions and layout
let address_space = AddressSpace::from_regions(regions, layout.clone());
```

## License

This project is licensed under [Apache License](http://www.apache.org/licenses/LICENSE-2.0), Version 2.0.
