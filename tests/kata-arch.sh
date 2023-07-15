#!/usr/bin/env bash
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
cat <<EOF
Description: Display host architecture name in various formats.

Usage: $script_name [options]

Options:

 -d, --default         : Show arch(1) architecture (this is the default).
 -g, --golang          : Show architecture name using golang naming.
 -r, --rust            : Show architecture name using rust naming
 -h, --help            : Show this help.
 -k, --kernel          : Show architecture name compatible with Linux* build system.

EOF
}

# Convert architecture to the name used by golang
arch_to_golang()
{
	local -r arch="$1"

	case "$arch" in
		aarch64) echo "arm64";;
		ppc64le) echo "$arch";;
		x86_64) echo "amd64";;
		s390x) echo "s390x";;
		*) die "unsupported architecture: $arch";;
	esac
}

# Convert architecture to the name used by rust
arch_to_rust()
{
	local arch="$1"

	if [ "${arch}" == "ppc64le" ]; then
		arch="powerpc64le"
		ARCH="${arch}"
	fi

	[ "${CROSS_BUILD}" == "false" ] && echo "${arch}" || echo "${ARCH}"
}

# Convert architecture to the name used by the Linux kernel build system
arch_to_kernel()
{
	local -r arch="$1"

	case "$arch" in
		aarch64) echo "arm64";;
		ppc64le) echo "powerpc";;
		x86_64) echo "$arch";;
		s390x) echo "s390x";;
		*) die "unsupported architecture: $arch";;
	esac
}

main()
{
	local type="default"

	local getopt_cmd="getopt"

	# macOS default getopt does not recognize GNU options
	[ "$(uname -s)" == "Darwin" ] && getopt_cmd="/usr/local/opt/gnu-getopt/bin/${getopt_cmd}"

	local args=$("$getopt_cmd" \
		-n "$script_name" \
		-a \
		--options="dgrhk" \
		--longoptions="default golang  rust help kernel" \
		-- "$@")

	eval set -- "$args"
	[ $? -ne 0 ] && { usage >&2; exit 1; }

	while [ $# -gt 1 ]
	do
		case "$1" in
			-d|--default) ;;

			-g|--golang) type="golang";;

			-r|--rust) type="rust";;

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
		rust) arch_to_rust "${arch}";;
		kernel) arch_to_kernel "$arch";;
	esac
}

main "$@"

