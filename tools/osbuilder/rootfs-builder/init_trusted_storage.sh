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

device_name=$(sed -e 's/DEVNAME=//g;t;d' /sys/dev/block/${device_num}/uevent)
device_path="/dev/$device_name"
if [[ -n "$device_name" && -b "$device_path" ]]; then
	storage_key_path="/run/cc_storage.key"
	dd if=/dev/urandom of="$storage_key_path" bs=1 count=4096

	if [ "$data_integrity" == "false" ]; then
		echo "YES" | cryptsetup luksFormat --type luks2 "$device_path" --sector-size 4096 \
			--cipher aes-xts-plain64 "$storage_key_path"
	else
		echo "YES" | cryptsetup luksFormat --type luks2 "$device_path" --sector-size 4096 \
			 --cipher aes-xts-plain64 --integrity hmac-sha256 "$storage_key_path"
	fi

	cryptsetup luksOpen -d "$storage_key_path" "$device_path" ephemeral_image_encrypted_disk
	rm "$storage_key_path"
	mkfs.ext4 /dev/mapper/ephemeral_image_encrypted_disk

	[ ! -d "/run/image" ] && mkdir /run/image

	mount /dev/mapper/ephemeral_image_encrypted_disk /run/image
else
	die "Invalid device: '$device_path'"
fi
