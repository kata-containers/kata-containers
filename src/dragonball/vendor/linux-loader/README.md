# Linux-loader

[![crates.io](https://img.shields.io/crates/v/linux-loader)](https://crates.io/crates/linux-loader)
[![docs.rs](https://img.shields.io/docsrs/linux-loader)](https://docs.rs/linux-loader/)

The `linux-loader` crate offers support for loading raw ELF (`vmlinux`) and
compressed big zImage (`bzImage`) format kernel images on `x86_64` and PE
(`Image`) kernel images on `aarch64`. ELF support includes the
[Linux](https://www.kernel.org/doc/Documentation/x86/boot.txt) and
[PVH](https://xenbits.xen.org/docs/unstable/misc/pvh.html) boot protocols.

The `linux-loader` crate is not yet fully independent and self-sufficient, and
much of the boot process remains the VMM's responsibility. See [Usage] for details.

## Supported features

- Parsing and loading kernel images into guest memory.
   - `x86_64`: `vmlinux` (raw ELF image), `bzImage`
   - `aarch64`: `Image`
- Parsing and building the kernel command line.
- Loading device tree blobs (`aarch64`).
- Configuring boot parameters using the exported primitives.
  - `x86_64` Linux boot:
    - [`setup_header`](https://elixir.bootlin.com/linux/latest/source/arch/x86/include/uapi/asm/bootparam.h#L65)
    - [`boot_params`](https://elixir.bootlin.com/linux/latest/source/arch/x86/include/uapi/asm/bootparam.h#L175)
  - `x86_64` PVH boot:
    - [`hvm_start_info`](https://elixir.bootlin.com/linux/latest/source/include/xen/interface/hvm/start_info.h#L125)
    - [`hvm_modlist_entry`](https://elixir.bootlin.com/linux/latest/source/include/xen/interface/hvm/start_info.h#L145)
    - [`hvm_memmap_table_entry`](https://elixir.bootlin.com/linux/latest/source/include/xen/interface/hvm/start_info.h#L152)
  - `aarch64` boot:
    - [`arm64_image_header`](https://elixir.bootlin.com/linux/latest/source/arch/arm64/include/asm/image.h#L44)

## Usage

Booting a guest using the `linux-loader` crate involves several steps,
depending on the boot protocol used. A simplified overview follows.

Consider an `x86_64` VMM that:
- interfaces with `linux-loader`;
- uses `GuestMemoryMmap` for its guest memory backend;
- loads an ELF kernel image from a `File`.

### Loading the kernel

One of the first steps in starting the guest is to load the kernel from a
[`Read`er](https://doc.rust-lang.org/std/io/trait.Read.html) into guest memory.
For this step, the VMM is required to have configured its guest memory.

In this example, the VMM specifies both the kernel starting address and the
starting address of high memory.

```rust
use linux_loader::loader::elf::Elf as Loader;
use vm_memory::GuestMemoryMmap;

use std::fs::File;
use std::result::Result;

impl MyVMM {
    fn start_vm(&mut self) {
        let guest_memory = self.create_guest_memory();
        let kernel_file = self.open_kernel_file();

        let load_result = Loader::load::<File, GuestMemoryMmap>(
            &guest_memory,
            Some(self.kernel_start_addr()),
            &mut kernel_file,
            Some(self.himem_start_addr()),
        )
        .expect("Failed to load kernel");
    }
}
```

### Configuring the devices and kernel command line

After the guest memory has been created and the kernel parsed and loaded, the
VMM will optionally configure devices and the kernel command line. The latter
can then be loaded in guest memory.

```rust
impl MyVMM {
    fn start_vm(&mut self) {
        ...
        let cmdline_size = self.kernel_cmdline().as_str().len() + 1;
        linux_loader::loader::load_cmdline::<GuestMemoryMmap>(
            &guest_memory,
            self.cmdline_start_addr(),
            &CString::new(kernel_cmdline).expect("Failed to parse cmdline")
        ).expect("Failed to load cmdline");
    }
```

### Configuring boot parameters

The VMM sets up initial registry values in this phase, without using
`linux-loader`. It can also configure additional boot parameters, using the
structs exported by `linux-loader`.

```rust
use linux_loader::configurator::linux::LinuxBootConfigurator;
use linux_loader::configurator::{BootConfigurator, BootParams};

impl MyVMM {
    fn start_vm(&mut self) {
        ...
        let mut bootparams = boot_params::default();
        self.configure_bootparams(&mut bootparams);
        LinuxBootConfigurator::write_bootparams(
            BootParams::new(&params, self.zeropage_addr()),
            &guest_memory,
        ).expect("Failed to write boot params in guest memory");
    }
```

Done!

## Testing

See [`docs/TESTING.md`](docs/TESTING.md).

## License

This project is licensed under either of:
- [Apache License](LICENSE-APACHE), Version 2.0
- [BSD-3-Clause License](LICENSE-BSD-3-Clause)
