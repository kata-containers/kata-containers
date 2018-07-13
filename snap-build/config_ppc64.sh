#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

local arch_qemu="ppc64"
local arch_image="bionic-server-cloudimg-ppc64el.img"
local arch_image_url="https://cloud-images.ubuntu.com/bionic/current/${arch_image}"
local arch_bios="QEMU_EFI.fd"
local arch_bios_url="https://releases.linaro.org/components/kernel/uefi-linaro/latest/release/qemu64/${arch_bios}"
local arch_qemu_cpu="POWER8"
local arch_qemu_machine="pseries,usb=off"
local arch_qemu_extra_opts="-echr 0x05 -boot c"
