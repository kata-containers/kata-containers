# dbs-allocator

## Design

The resource manager in the `Dragonball Sandbox` needs to manage and allocate different kinds of resource for the
sandbox (virtual machine), such as memory-mapped I/O address space, port I/O address space, legacy IRQ numbers,
MSI/MSI-X vectors, device instance id, etc. The `dbs-allocator` crate is designed to help the resource manager
to track and allocate these types of resources.

Main components are:   
- *Constraints*: struct to declare constraints for resource allocation.
```rust
#[derive(Copy, Clone, Debug)]
pub struct Constraint {
    /// Size of resource to allocate.
    pub size: u64,
    /// Lower boundary for resource allocation.
    pub min: u64,
    /// Upper boundary for resource allocation.
    pub max: u64,
    /// Alignment for allocated resource.
    pub align: u64,
    /// Policy for resource allocation.
    pub policy: AllocPolicy,
}
```
- `IntervalTree`: An interval tree implementation specialized for VMM resource management.
```rust
pub struct IntervalTree<T> {
    pub(crate) root: Option<Node<T>>,
}
​
pub fn allocate(&mut self, constraint: &Constraint) -> Option<Range>
pub fn free(&mut self, key: &Range) -> Option<T>
pub fn insert(&mut self, key: Range, data: Option<T>) -> Self
pub fn update(&mut self, key: &Range, data: T) -> Option<T>
pub fn delete(&mut self, key: &Range) -> Option<T> 
pub fn get(&self, key: &Range) -> Option<NodeState<&T>>
```

## Usage
The concept of Interval Tree may seem complicated, but using dbs-allocator to do resource allocation and release is simple and straightforward. 
You can following these steps to allocate your VMM resource.
```rust
// 1. To start with, we should create an interval tree for some specific resouces and give maximum address/id range as root node. The range here could be address range, id range, etc.
​
let mut resources_pool = IntervalTree::new(); 
resources_pool.insert(Range::new(MIN_RANGE, MAX_RANGE), None); 
​
// 2. Next, create a constraint with the size for your resource, you could also assign the maximum, minimum and alignment for the constraint. Then we could use the constraint to allocate the resource in the range we previously decided. Interval Tree will give you the appropriate range. 
let mut constraint = Constraint::new(SIZE);
let mut resources_range = self.resources_pool.allocate(&constraint);
​
// 3. Then we could use the resource range to let other crates like vm-pci / vm-device to create and maintain the device
let mut device = Device::create(resources_range, ..)
```

## Example
We will show examples for allocating an unused PCI device ID from the PCI device ID pool and allocating memory address using dbs-allocator
```rust
use dbs_allocator::{Constraint, IntervalTree, Range};
​
// Init a dbs-allocator IntervalTree
let mut pci_device_pool = IntervalTree::new();
​
// Init PCI device id pool with the range 0 to 255
pci_device_pool.insert(Range::new(0x0u8, 0xffu8), None); 
​
// Construct a constraint with size 1 and alignment 1 to ask for an ID. 
let mut constraint = Constraint::new(1u64).align(1u64); 
​
// Get an ID from the pci_device_pool
let mut id = pci_device_pool.allocate(&constraint).map(|e| e.min as u8); 
​
// Pass the ID generated from dbs-allocator to vm-pci specified functions to create pci devices
let mut pci_device = PciDevice::new(id as u8, ..);

```

```rust
use dbs_allocator::{Constraint, IntervalTree, Range};
​
// Init a dbs-allocator IntervalTree
let mut mem_pool = IntervalTree::new();
​
// Init memory address from GUEST_MEM_START to GUEST_MEM_END
mem_pool.insert(Range::new(GUEST_MEM_START, GUEST_MEM_END), None); 
​
// Construct a constraint with size, maximum addr and minimum address of memory region to ask for an memory allocation range. 
let constraint = Constraint::new(region.len())
                .min(region.start_addr().raw_value())
                .max(region.last_addr().raw_value());
​
// Get the memory allocation range from the pci_device_pool
let mem_range = mem_pool.allocate(&constraint).unwrap(); 
​
// Update the mem_range in IntervalTree with memory region info
mem_pool.update(&mem_range, region);
​
// After allocation, we can use the memory range to do mapping and other memory related work.
...
```

## License

This project is licensed under [Apache License](http://www.apache.org/licenses/LICENSE-2.0), Version 2.0.