#!/bin/bash
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
	cat <<EOT
Usage:

${script_name} [options]

Example:
${script_name}

Options
-d <path> : destination path, path where go will be installed.
EOT

	exit "$exit_code"
}

trap finish EXIT

pushd "${tmp_dir}"

while getopts "d:fhp" opt
do
	case $opt in
		d)	install_dest="${OPTARG}" ;;
		f)	force="true" ;;
		h)	usage 0 ;;
	esac
done

shift $(( $OPTIND - 1 ))


go_version=${1:-}

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

case "$(uname -m)" in
	aarch64) goarch="arm64";;
	ppc64le) goarch="ppc64le";;
	x86_64) goarch="amd64";;
	s390x) goarch="s390x";;
	*) echo "unsupported architecture: $(uname -m)"; exit 1;;
esac

info "Download go version ${go_version}"
kernel_name=$(uname -s)
curl -OL "https://storage.googleapis.com/golang/go${go_version}.${kernel_name,,}-${goarch}.tar.gz"
info "Install go"
mkdir -p "${install_dest}"
sudo tar -C "${install_dest}" -xzf "go${go_version}.${kernel_name,,}-${goarch}.tar.gz"
popd
