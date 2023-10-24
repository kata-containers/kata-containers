#!/bin/sh
#
# Copyright (c) 2022 Intel
#
# SPDX-License-Identifier: Apache-2.0

[ -d /dev ] || mkdir -m 0755 /dev
[ -d /root ] || mkdir -m 0700 /root
[ -d /sys ] || mkdir /sys
[ -d /proc ] || mkdir /proc
[ -d /mnt ] || mkdir /mnt
[ -d /tmp ] || mkdir /tmp

mount -t sysfs -o nodev,noexec,nosuid sysfs /sys
mount -t proc -o nodev,noexec,nosuid proc /proc

echo "/sbin/mdev" > /proc/sys/kernel/hotplug
mdev -s

get_option() {
	local value
	value=" $(cat /proc/cmdline) "
	value="${value##* ${1}=}"
	value="${value%% *}"
	[ "${value}" != "" ] && echo "${value}"
}

rootfs_verifier=$(get_option rootfs_verity.scheme)
rootfs_hash=$(get_option rootfs_verity.hash)
root_device=$(get_option root)
hash_device=${root_device%?}2

# The root device should exist to be either verified then mounted or
# just mounted when verification is disabled.
if [ ! -e "${root_device}" ]
then
	echo "No root device ${root_device} found"
	exit 1
fi

if [ "${rootfs_verifier}" = "dm-verity" ]
then
	echo "Verify the root device with ${rootfs_verifier}"

	if [ ! -e "${hash_device}" ]
	then
		echo "No hash device ${hash_device} found. Cannot verify the root device"
		exit 1
	fi

	veritysetup open "${root_device}" root "${hash_device}" "${rootfs_hash}"
	mount /dev/mapper/root /mnt
else
	echo "No LUKS device found"
	mount "${root_device}" /mnt
fi

umount /proc
umount /sys
exec switch_root /mnt /sbin/init
