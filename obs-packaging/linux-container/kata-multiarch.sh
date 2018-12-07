#!/usr/bin/env bash

set -e

# Convert architecture to the name used by the Linux kernel build system
arch_to_kernel() {
	local -r arch="$1"

	case "${arch}" in
		aarch64) echo "arm64";;
		ppc64le) echo "powerpc";;
		s390|s390x) echo "s390";;
		x86_64) echo "${arch}";;
		*) echo "unsupported architecture: ${arch}" >&2; exit 1;;
	esac
}

# Convert architecture to the location of the compressed linux image
arch_to_image() {
	local -r arch="$1"
	case "${arch}" in
		aarch64)
			echo "arch/arm64/boot/Image"
			;;
		ppc64le)
			# No compressed image
			;;
		s390|s390x)
			echo "arch/s390/boot/image"
			;;
		*)
			echo "arch/${arch}/boot/bzImage"
			;;
	esac
}

usage() {
	echo "$(basename $0) FLAG ARCHITECTURE"
	echo "Allowed flags:"
	echo "    -a :    Print kernel architecture"
	echo "    -i :    Print kernel compressed image location (may be empty)"
}

if [ "$#" != "2" ]; then
	echo -e "Invalid options\n\n$(usage)" >&2
	exit 1
fi

case "$1" in
	-a)
		arch_to_kernel $2
		;;
	-i)
		arch_to_image $2
		;;
	*)
		echo -e "Invalid options\n\n$(usage)" >&2
		exit 1
		;;
esac
