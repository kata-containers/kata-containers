# `dbs-pci`

## Introduction

`dbs-pci` is a crate for emulating PCI device.

There are several components in `dbs-pci` crate building together to emulate PCI device behaviour :

1. device mod: mainly provide the trait for `PciDevice`, providing the ability to get id, write PCI configuration space, read PCI configuration space and `as_any` to downcast the trait object to the actual device type.

2. configuration mod: simulate PCI device configuration header and manage PCI Bar configuration. The PCI Specification defines the organization of the 256-byte Configuration Space registers and imposes a specific template for the space. The first 64 bytes of configuration space are standardised as configuration space header.

3. bus mod: simulate PCI buses, to simplify the implementation, PCI hierarchy is not supported. So all PCI devices are directly connected to the PCI root bus. PCI Bus has bus id, PCI devices attached and PCI bus I/O port, I/O mem resource use condition.

4. root bus mod: mainly for emulating PCI root bridge and also create the PCI root bus with the given bus ID with the PCI root bridge.

5. root device mod: A pseudo PCI root device to manage accessing to PCI configuration space.
