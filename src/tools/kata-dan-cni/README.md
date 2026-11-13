# kata-dan-cni

## Overview

kata-dan-cni is a Rust CNI plugin that provides Directly Attachable Network (DAN) support for Kata Containers.

The plugin serves as the default network CNI entry point. The non-DAN path delegates to the existing primary network plugin, while the DAN path reuses existing network allocations, creates host-side devices, and generates DAN JSON.

## Features

- Provides Kube-OVN integration.
- Supports CNI `ADD`, `DEL`, basic `CHECK`, and `VERSION` commands.
- Supports IPv4, IPv6, and dual-stack networking.
- Provides the `host-tap` backend.
- Serves as the default primary network CNI and can be used with chained plugins and Multus. Secondary network attachments are not currently supported when DAN is enabled.
- Includes scripts for DaemonSet installation, credential synchronization, and uninstallation.

## Supporting other CNIs

Add a Provider implementation under `crates/providers` and integrate it into the selection flow. The Provider translates the target CNI's existing network allocations and logical port information into common data for the Backend. When compatible with an existing Backend, device creation, CNI command handling, and DAN file management can be reused.

## Supporting other backends

Add and register a Backend implementation under `crates/backends`. The Backend uses the common network data supplied by the Provider to create devices and connect them to the network. It handles device checks, deletion, and failure rollback while reusing the common CNI dispatch and DAN file management flow.

## Platform support

- Supports x86_64 (amd64) Linux systems with the GNU C library.

## Quick start

Complete the [prerequisites](docs/prepare.md), then follow the [installation and configuration guide](docs/setup.md) to deploy the DaemonSet.

To enable DAN networking for a Pod, add the `networking.katacontainers.io/dan: "true"` annotation and select the RuntimeClass specified in the deployment configuration.
