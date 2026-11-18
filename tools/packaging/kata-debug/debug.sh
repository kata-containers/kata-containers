#!/usr/bin/env bash
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

echo "Let's gather Kata Containers debug information"
echo ""
echo "::group::Check Kata Containers logs"
chroot /host /bin/bash -c "sudo journalctl -xe -t kata | tee"
echo "::endgroup::"
echo ""
echo "::group::Checking the loaded kernel modules"
chroot /host /bin/bash -c "sudo lsmod"
echo "::endgroup::"
echo ""
echo "::group::Check Kata Containers deployed binaries"
if command -v tree &>/dev/null; then
	tree /host/opt/kata /host/usr/local/bin
else
	ls -lR /host/opt/kata /host/usr/local/bin
fi
echo "::endgroup::"
echo ""
echo "::group:: Check node's dmesg"
chroot /host /bin/bash -c "sudo dmesg"
echo "::endgroup::"
