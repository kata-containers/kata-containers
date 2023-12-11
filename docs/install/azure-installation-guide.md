# Install Kata Containers on Microsoft Azure

Kata Containers on Azure use nested virtualization to provide an identical installation
experience to Kata on your preferred Linux distribution.

This guide assumes you have an Azure account set up and tools to remotely login to your virtual
machine (SSH). Instructions will use the Azure Portal to avoid
local dependencies and setup.

## Create a new virtual machine with nesting support

Create a new virtual machine with:
* Nesting support (v3 series)
* your distro of choice

## Set up with distribution specific quick start

Follow distribution specific [install guides](../install/README.md#packaged-installation-methods).
