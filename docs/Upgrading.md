# Introduction

This document outlines the options for upgrading from a
[Kata Containers 1.x release](https://github.com/kata-containers/runtime/releases) to a
[Kata Containers 2.x release](https://github.com/kata-containers/kata-containers/releases).

# Maintenance warning

Kata Containers 2.x is the new focus for the Kata Containers development
community.

Although Kata Containers 1.x releases will continue to be published for a
period of time, once a stable release for Kata Containers 2.x is published,
Kata Containers 1.x stable users should consider switching to the Kata 2.x
release.

# Determine current version

To display the current Kata Containers version, run one of the following:

```bash
$ kata-runtime --version
$ containerd-shim-kata-v2 --version
```

# Determine latest version

Kata Containers 2.x releases are published on the
[Kata Containers GitHub releases page](https://github.com/kata-containers/kata-containers/releases).

Alternatively, if you are using Kata Containers version 1.12.0 or newer, you
can check for newer releases using the command line:

```bash
$ kata-runtime check --check-version-only
```

There are various other related options. Run `kata-runtime check --help`
for further details.

# Configuration changes

The [Kata Containers 2.x configuration file](/src/runtime/README.md#configuration)
is compatible with the
[Kata Containers 1.x configuration file](https://github.com/kata-containers/runtime/blob/master/README.md#configuration).

However, if you have created a local configuration file
(`/etc/kata-containers/configuration.toml`), this will mask the newer Kata
Containers 2.x configuration file.

Since Kata Containers 2.x introduces a number of new options and changes
some default values, we recommend that you disable the local configuration
file (by moving or renaming it) until you have reviewed the changes to the
official configuration file and applied them to your local file if required.

# Upgrade Kata Containers

## Upgrade native distribution packaged version

As shown in the
[installation instructions](install),
Kata Containers provide binaries for popular distributions in their native
packaging formats. This allows Kata Containers to be upgraded using the
standard package management tools for your distribution.

> **Note:**
>
> Users should prefer the distribution packaged version of Kata Containers
> unless they understand the implications of a manual installation.

## Static installation

> **Note:**
>
> Unless you are an advanced user, if you are using a static installation of
> Kata Containers, we recommend you remove it and install a
> [native distribution packaged version](#upgrade-native-distribution-packaged-version)
> instead.

### Determine if you are using a static installation

If the following command displays the output "static", you are using a static
version of Kata Containers:

```bash
$ ls /opt/kata/bin/kata-runtime &>/dev/null && echo static
```

### Remove a static installation

Static installations are installed in `/opt/kata/`, so to uninstall simply
remove this directory.

### Upgrade a static installation

If you understand the implications of using a static installation, to upgrade
first
[remove the existing static installation](#remove-a-static-installation), then
[install the latest release](#determine-latest-version).

See the
[manual installation documentation](install/README.md#manual-installation)
for details on how to automatically install and configuration a static release
with containerd.

# Custom assets

> **Note:**
>
> This section only applies to advanced users who have built their own guest
> kernel or image.

If you are using custom
[guest assets](design/architecture/README.md#guest-assets),
you must upgrade them to work with Kata Containers 2.x since Kata
Containers 1.x assets will **not** work.

See the following for further details:

- [Guest kernel documentation](/tools/packaging/kernel)
- [Guest image and initrd documentation](/tools/osbuilder)

The official assets are packaged meaning they are automatically included in
new releases.
