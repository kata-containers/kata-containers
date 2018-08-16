#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

local arch_qemu="x86_64"
local arch_image="bionic-server-cloudimg-amd64.img"
local arch_image_url="https://cloud-images.ubuntu.com/bionic/current/${arch_image}"
local arch_bios=""
local arch_bios_url=""
local arch_qemu_cpu="qemu64"
local arch_qemu_machine="pc"
local arch_qemu_extra_opts=""
if [ "$(arch)" == "x86_64" ]; then
	arch_qemu_cpu="host"
	arch_qemu_machine="pc,accel=kvm"
	arch_qemu_extra_opts="-enable-kvm"
fi
