#!/usr/bin/env bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

tmp_dir=$(mktemp -d -t install-go-tmp.XXXXXXXXXX)
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
script_name="$(basename "${BASH_SOURCE[0]}")"
force=""

install_dest="/usr/local/"

finish() {
	rm -rf "$tmp_dir"
}

die() {
	echo >&2 "ERROR: $*"
	exit 1
}

info() {
	echo "INFO: $*"
}

usage(){
	exit_code="$1"
	cat <<EOF
Usage:

${script_name} [options]

Example:
${script_name}

Options
-d <path> : destination path, path where go will be installed.
-f        : enable force install, remove existent go pkg before installation.
-h        : display this help.
EOF

	exit "$exit_code"
}

trap finish EXIT

go_version=${1:-}
rust_version=${2:-}

ARCH=${ARCH:-$(uname -m)}
case "${ARCH}" in
	aarch64)
		goarch=arm64
		LIBC=musl
		# This is a hack needed as part of Ubuntu 20.04
		if [ ! -f /usr/bin/aarch64-linux-musl-gcc ]; then
			ln -sf /usr/bin/musl-gcc /usr/bin/aarch64-linux-musl-gcc
		fi
		;;
	ppc64le) 
		goarch=${ARCH}
		ARCH=powerpc64le
		LIBC=gnu
		;;
	s390x)
		goarch=${ARCH}
		LIBC=gnu
		;;
	x86_64)
		goarch=amd64
		LIBC=musl
		;;
	*)
		echo "unsupported architecture $(uname -m)"
		exit 1
		;;
esac

#curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSLf | sh -s -- -y --default-toolchain ${rust_version} -t ${ARCH}-unknown-linux-${LIBC}

#rustup target add ${ARCH}-unknown-linux-${LIBC}

pushd "${tmp_dir}"

while getopts "d:fh" opt
do
	case $opt in
		d)	install_dest="${OPTARG}" ;;
		f)	force="true" ;;
		h)	usage 0 ;;
	esac
done

shift $(( $OPTIND - 1 ))

if [ -z "$go_version" ];then
	echo "Missing go"
	usage 1
fi

if command -v go; then
	[[ "$(go version)" == *"go${go_version}"* ]] && \
		info "Go ${go_version} already installed" && \
		exit
	if [ "${force}" = "true" ]; then
		info "removing $(go version)"
		sudo rm -rf "${install_dest}/go"
	else
		die "$(go version) is installed, use -f or remove it before install go ${go_version}"
	fi
fi

info "Download go version ${go_version}"
kernel_name=$(uname -s)
curl -OL "https://storage.googleapis.com/golang/go${go_version}.${kernel_name,,}-${goarch}.tar.gz"
info "Install go"
mkdir -p "${install_dest}"
sudo tar -C "${install_dest}" -xzf "go${go_version}.${kernel_name,,}-${goarch}.tar.gz"
popd
