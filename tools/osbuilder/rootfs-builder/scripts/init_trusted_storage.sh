#!/bin/bash
#
# Copyright (c) 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

[ -n "${DEBUG:-}" ] && set -o xtrace

handle_error() {
	local exit_code="${?}"
	local line_number="${1:-}"
	echo "error:"
	echo "Failed at $line_number: ${BASH_COMMAND}"
	exit "${exit_code}"
}
trap 'handle_error $LINENO' ERR

die() {
	local msg="$*"
	echo >&2 "ERROR: $msg"
	exit 1
}

setup() {
	local cmds=()

	cmds+=("cryptsetup" "mkfs.ext4" "mount")

	local cmd
	for cmd in "${cmds[@]}"; do
		command -v "$cmd" &>/dev/null || die "need command: '$cmd'"
	done
}

setup

device_num=${1:-}
if [ -z "$device_num" ]; then
	die "invalid arguments, at least one param for device num"
fi

is_encrypted="false"
if [ -n "${2-}" ]; then
	is_encrypted="$2"
fi

mount_point="/tmp/target_path"
if [ -n "${3-}" ]; then
	mount_point="$3"
fi

storage_key_path="/run/encrypt_storage.key"
if [ -n "${4-}" ]; then
	storage_key_path="$4"
fi

data_integrity="true"
if [ -n "${5-}" ]; then
	data_integrity="$5"
fi

device_name=$(sed -e 's/DEVNAME=//g;t;d' "/sys/dev/block/${device_num}/uevent")
device_path="/dev/$device_name"

opened_device_name=$(mktemp -u "encrypted_disk_XXXXX")

if [[ -n "$device_name" && -b "$device_path" ]]; then

	if [ "$is_encrypted" == "false" ]; then

		if [ "$data_integrity" == "false" ]; then
			cryptsetup --batch-mode luksFormat --type luks2 "$device_path" --sector-size 4096 \
				--cipher aes-xts-plain64 "$storage_key_path"
		else
			# Wiping a device is a time consuming operation. To avoid a full wipe, integritysetup
			# and crypt setup provide a --no-wipe option.
			# However, an integrity device that is not wiped will have invalid checksums. Normally
			# this should not be a problem since a page must first be written to before it can be read
			# (otherwise the data would be arbitrary). The act of writing would populate the checksum
			# for the page.
			# However, tools like mkfs.ext4 read pages before they are written; sometimes the read
			# of an unwritten page happens due to kernel buffering.
			# See https://gitlab.com/cryptsetup/cryptsetup/-/issues/525 for explanation and fix.
			# The way to propery format the non-wiped dm-integrity device is to figure out which pages
			# mkfs.ext4 will write to and then to write to those pages before hand so that they will
			# have valid integrity tags.
			cryptsetup --batch-mode luksFormat --type luks2 "$device_path" --sector-size 4096 \
				--cipher aes-xts-plain64 --integrity hmac-sha256 "$storage_key_path" \
				--integrity-no-wipe
		fi
	fi

	cryptsetup luksOpen -d "$storage_key_path" "$device_path" $opened_device_name
	rm "$storage_key_path"

	if [ "$data_integrity" == "false" ]; then
		mkfs.ext4 /dev/mapper/$opened_device_name -E lazy_journal_init
	else
		# mkfs.ext4 doesn't perform whole sector writes and this will cause checksum failures
		# with an unwiped integrity device. Therefore, first perform a dry run.
		output=$(mkfs.ext4 /dev/mapper/$opened_device_name -F -n)

		# The above command will produce output like
		# mke2fs 1.46.5 (30-Dec-2021)
		# Creating filesystem with 268435456 4k blocks and 67108864 inodes
		# Filesystem UUID: 4a5ff012-91c0-47d9-b4bb-8f83e830825f
		# Superblock backups stored on blocks:
		#         32768, 98304, 163840, 229376, 294912, 819200, 884736, 1605632, 2654208,
		#         4096000, 7962624, 11239424, 20480000, 23887872, 71663616, 78675968,
		#         102400000, 214990848
		delimiter="Superblock backups stored on blocks:"
		blocks_list=$([[ $output =~ $delimiter(.*) ]] && echo "${BASH_REMATCH[1]}")

		# Find list of blocks
		block_nums=$(echo "$blocks_list" | grep -Eo '[0-9]{4,}' | sort -n)

		# Add zero to list of blocks
		block_nums="0 $block_nums"

		# Iterate through each block and write to it to ensure that it has valid checksum
		for block_num in $block_nums; do
			echo "Clearing page at $block_num"
			# Zero out the page
			dd if=/dev/zero bs=4k count=1 oflag=direct \
				of=/dev/mapper/$opened_device_name seek="$block_num"
		done

		# Now perform the actual ext4 format. Use lazy_journal_init so that the journal is
		# initialized on demand. This is safe for ephemeral storage since we don't expect
		# ephemeral storage to survice a power cycle.
		mkfs.ext4 /dev/mapper/$opened_device_name -E lazy_journal_init
	fi

	[ ! -d "$mount_point" ] && mkdir -p $mount_point

	mount /dev/mapper/$opened_device_name $mount_point
else
	die "Invalid device: '$device_path'"
fi
