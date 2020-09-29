# Install Kata Containers on VEXXHOST

Kata Containers on VEXXHOST use nested virtualization to provide an identical
installation experience to Kata on your preferred Linux distribution.

This guide assumes you have an OpenStack public cloud account set up and tools
to remotely connect to your virtual machine (SSH).

## Create a new virtual machine with nesting support

All regions support nested virtualization using the V2 flavors (those prefixed
with v2).  The recommended machine type for container workloads is `v2-highcpu` range.

## Set up with distribution specific quick start

Follow distribution specific [install guides](../install/README.md#packaged-installation-methods).
