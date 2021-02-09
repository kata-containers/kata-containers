# cgroups-rs ![Build](https://travis-ci.org/levex/cgroups-rs.svg?branch=master)
Native Rust library for managing control groups under Linux

Right now the crate only support the original, V1 hierarchy, however support
is planned for the Unified hierarchy.

# Examples

## Create a control group using the builder pattern

``` rust
// Acquire a handle for the V1 cgroup hierarchy.
let hier = ::hierarchies::V1::new();

// Use the builder pattern (see the documentation to create the control group)
//
// This creates a control group named "example" in the V1 hierarchy.
let cg: Cgroup = CgroupBuilder::new("example", &v1)
	.cpu()
		.shares(85)
		.done()
	.build();

// Now `cg` is a control group that gets 85% of the CPU time in relative to
// other control groups.

// Get a handle to the CPU controller.
let cpus: &CpuController = cg.controller_of().unwrap();
cpus.add_task(1234u64);

// [...]

// Finally, clean up and delete the control group.
cg.delete();

// Note that `Cgroup` does not implement `Drop` and therefore when the
// structure is dropped, the Cgroup will stay around. This is because, later
// you can then re-create the `Cgroup` using `load()`. We aren't too set on
// this behavior, so it might change in the feature. Rest assured, it will be a
// major version change.
```

# Disclaimer

This crate is licensed under:

- MIT License (see LICENSE-MIT); or
- Apache 2.0 License (see LICENSE-Apache-2.0),

at your option.

Please note that this crate is under heavy development, we will use sematic
versioning, but during the `0.0.*` phase, no guarantees are made about
backwards compatibility.

Regardless, check back often and thanks for taking a look!
