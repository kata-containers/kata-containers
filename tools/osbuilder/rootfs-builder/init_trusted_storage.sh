#!/bin/bash
#
# Copyright (c) 2022 Intel Corporation
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

die()
{
	local msg="$*"
	echo >&2 "ERROR: $msg"
	exit 1
}

setup()
{
	local cmds=()

	cmds+=("cryptsetup" "mkfs.ext4" "mount")

	local cmd
	for cmd in "${cmds[@]}"
	do
		command -v "$cmd" &>/dev/null || die "need command: '$cmd'"
	done
}

setup

device_num=${1:-}
if [ -z "$device_num" ]; then
	die "invalid arguments, at least one param for device num"
fi

data_integrity="true"
if [ -n "${2-}" ]; then
        data_integrity="$2"
fi

device_name=$(sed -e 's/DEVNAME=//g;t;d' "/sys/dev/block/${device_num}/uevent")
device_path="/dev/$device_name"
if [[ -n "$device_name" && -b "$device_path" ]]; then
	storage_key_path="/run/cc_storage.key"
	dd if=/dev/urandom of="$storage_key_path" bs=1 count=4096

	if [ "$data_integrity" == "false" ]; then
		echo "YES" | cryptsetup luksFormat --type luks2 "$device_path" --sector-size 4096 \
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
		echo "YES" | cryptsetup luksFormat --type luks2 "$device_path" --sector-size 4096 \
			--cipher aes-xts-plain64 --integrity hmac-sha256 "$storage_key_path" \
			--integrity-no-wipe
	fi

	cryptsetup luksOpen -d "$storage_key_path" "$device_path" ephemeral_image_encrypted_disk
	rm "$storage_key_path"
	if [ "$data_integrity" == "false" ]; then
	    mkfs.ext4 /dev/mapper/ephemeral_image_encrypted_disk -E lazy_journal_init
	else
	    # mkfs.ext4 doesn't perform whole sector writes and this will cause checksum failures
	    # with an unwiped integrity device. Therefore, first perform a dry run.
	    output=$(mkfs.ext4 /dev/mapper/ephemeral_image_encrypted_disk -F -n)

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
    	    block_nums=$(echo  "$blocks_list" | grep -Eo '[0-9]{4,}' | sort -n)

            # Add zero to list of blocks
            block_nums="0 $block_nums"

	    # Iterate through each block and write to it to ensure that it has valid checksum
	    for block_num in $block_nums
	    do
		echo "Clearing page at $block_num"
		# Zero out the page
		dd if=/dev/zero bs=4k count=1 oflag=direct \
		   of=/dev/mapper/ephemeral_image_encrypted_disk seek="$block_num"
	    done

	    # Now perform the actual ext4 format. Use lazy_journal_init so that the journal is
	    # initialized on demand. This is safe for ephemeral storage since we don't expect
	    # ephemeral storage to survice a power cycle.
	    mkfs.ext4 /dev/mapper/ephemeral_image_encrypted_disk -E lazy_journal_init
	fi


	[ ! -d "/run/image" ] && mkdir /run/image

	mount /dev/mapper/ephemeral_image_encrypted_disk /run/image
else
	die "Invalid device: '$device_path'"
fi
