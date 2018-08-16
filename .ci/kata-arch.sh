#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

typeset -r script_name=${0##*/}

typeset -r cidir=$(dirname "$0")
source "${cidir}/lib.sh"

usage()
{
cat <<EOT
Description: Display host architecture name in various formats.

Usage: $script_name [options]

Options:

 -d, --default         : Show arch(1) architecture (this is the default).
 -g, --golang          : Show architecture name using golang naming.
 -h, --help            : Show this help.
 -k, --kernel          : Show architecture name compatible with Linux* build system.

EOT
}

# Convert architecture to the name used by golang
arch_to_golang()
{
	local -r arch="$1"

	case "$arch" in
		aarch64) echo "arm64";;
		ppc64le) echo "$arch";;
		x86_64) echo "amd64";;
		*) die "unsupported architecture: $arch";;
	esac
}

# Convert architecture to the name used by the Linux kernel build system
arch_to_kernel()
{
	local -r arch="$1"

	case "$arch" in
		aarch64) echo "arm64";;
		ppc64le) echo "powerpc";;
		x86_64) echo "$arch";;
		*) die "unsupported architecture: $arch";;
	esac
}

main()
{
	local type="default"

	local args=$(getopt \
		-n "$script_name" \
		-a \
		--options="dghk" \
		--longoptions="default golang help kernel" \
		-- "$@")

	eval set -- "$args"
	[ $? -ne 0 ] && { usage >&2; exit 1; }

	while [ $# -gt 1 ]
	do
		case "$1" in
			-d|--default) ;;

			-g|--golang) type="golang";;

			-h|--help)
				usage
				exit 0
				;;

			-k|--kernel) type="kernel";;

			--)
				shift
				break
				;;
		esac
		shift
	done

	local -r arch=$(uname -m)

	case "$type" in
		default) echo "$arch";;
		golang) arch_to_golang "$arch";;
		kernel) arch_to_kernel "$arch";;
	esac
}

main "$@"
