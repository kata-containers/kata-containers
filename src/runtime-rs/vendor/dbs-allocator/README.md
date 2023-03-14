# dbs-allocator

## Design

dbs-allocator is designed as a resource allocator for Dragonball VMM. It provides allocation and release strategy of different kinds of resources that might be used by VM, such as memory-mapped I/O address space, port I/O address space, legacy IRQ numbers, MSI/MSI-X vectors, device instance id, etc. All these kinds of resource should be allocated and released by dbs-allocator in order to make VMM easier to construct and resource allocation easier to track and maintain.
Main components are:   
Constraints: describe resource allocation constraints.
IntervalTree: An VMM specified interval tree responsible for allocating and releasing resources.

```rust
pub struct Constraint {
    /// Size to allocate.
    pub size: u64,
    /// Lower boundary for the allocated resource.
    pub min: u64,
    /// Upper boundary for the allocated resource.
    pub max: u64,
    /// Alignment for the allocated resource.
    pub align: u64,
    /// Resource allocation policy.
    pub policy: AllocPolicy,
}
```

Struct Constraint is used to describe the overall information of the resource needed to be allocated and IntervalTree could use the Constraint information to know where and how to allocate the resource.
```rust
pub struct IntervalTree<T> {
    pub(crate) root: Option<Node<T>>,
}
​
pub(crate) struct Node<T>(pub(crate) Box<InnerNode<T>>);
​
pub(crate) struct InnerNode<T> {
    /// Interval handled by this node.
    pub(crate) key: Range,
    /// Optional contained data, None if the node is free.
    pub(crate) data: NodeState<T>,
    /// Optional left child of current node.
    pub(crate) left: Option<Node<T>>,
    /// Optional right child of current node.
    pub(crate) right: Option<Node<T>>,
    /// Cached height of the node.
    pub(crate) height: u32,
    /// Cached maximum valued covered by this node.
    pub(crate) max_key: u64,
}
​
pub enum NodeState<T> {
    /// Node is free
    Free,
    /// Node is allocated but without associated data
    Allocated,
    /// Node is allocated with associated data.
    Valued(T),
}
​
pub fn allocate(&mut self, constraint: &Constraint) -> Option<Range>
pub fn free(&mut self, key: &Range) -> Option<T>
pub fn insert(&mut self, key: Range, data: Option<T>) -> Self
pub fn update(&mut self, key: &Range, data: T) -> Option<T>
pub fn delete(&mut self, key: &Range) -> Option<T> 
pub fn get(&self, key: &Range) -> Option<NodeState<&T>>

```
With the interval tree developed for VMM, we introduce 2 VMM specified functions - Allocate and Free.  We could do resource allocation and release with better query and creation performance, safe boundary check and better abstraction APIs.
We should assign a maximum resource range that the IntervalTree could hold as the root node. Then we could use different functions in allocator.
Allocate with constraint coule be used to allocate a range for specific resource in the interval tree. 
Free could be used to release an allocated range and return the associated resource.
Update could be used to update an existing entry and return the old value.
Insert could be used to insert specific resource into some range if we know exact range and resource to put and ensure there is no risk.
Delete could be used to remove the range from the tree and return the associated data.
Get could be used to get the data item associated with the range, or return None if no match found.
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