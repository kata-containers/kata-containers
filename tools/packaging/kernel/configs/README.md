# Kata Containers kernel config files

This directory contains Linux Kernel config files used to configure Kata
Containers VM kernels.

## Types of config files

This directory holds config files for the Kata Linux Kernel in two forms:

- A tree of config file `fragments` in the `fragments` sub-folder, that are
  constructed into a complete config file using the kernel
  `scripts/kconfig/merge_config.sh` script.
- As complete config files that can be used as-is.

Kernel config fragments are the preferred method of constructing `.config` files
to build Kata Containers kernels, due to their improved clarity and ease of maintenance
over single file monolithic `.config`s.

## How to use config files

The recommended way to set up a kernel tree, populate it with a relevant `.config` file,
and build a kernel, is to use the [`build_kernel.sh`](../build-kernel.sh) script. For
example:

```bash
$ ./build-kernel.sh setup
```

The `build-kernel.sh` script understands both full and fragment based config files.

Run `./build-kernel.sh help` for more information.

## How to modify config files

Complete config files can be modified either with an editor, or preferably
using the kernel `Kconfig` configuration tools, for example:

```
$ cp x86_kata_kvm_4.14.x linux-4.14.22/.config
$ pushd linux-4.14.22
$ make menuconfig
$ popd
$ cp linux-4.14.22/.config x86_kata_kvm_4.14.x
```

Kernel fragments are best constructed using an editor. Tools such as `grep` and
`diff` can help find the differences between two config files to be placed
into a fragment.

If adding config entries for a new subsystem or feature, consider making a new
fragment with an appropriately descriptive name.

If you want to disable an entire fragment for a specific architecture, you can add the tag `# !${arch}` in the first line of the fragment. You can also exclude multiple architectures on the same line. Note the `#` at the beginning of the line, this is required to avoid that the tag is interpreted as a configuration.
Example of valid exclusion:
```
# !s390x !ppc64le
```

The fragment gathering tool performs some basic sanity checks, and the `build-kernel.sh` will
fail and report the error in the cases of:

- A duplicate `CONFIG` symbol appearing.
- A `CONFIG` symbol being in a fragment, but not appearing in the final .config
  - which indicates that `CONFIG` variable is not a part of the kernel `Kconfig` setup, which
    can indicate a typing mistake in the name of the symbol.
- A `CONFIG` symbol appearing in the fragments with multiple different values.
