#!/bin/bash
#
# Copyright (c) 2018-2023 Intel Corporation
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
USE_VERSIONS_FILE=""
PROJECT="Kata Containers"

source "${script_dir}/common.bash"

install_dest="/usr/local/"

function finish() {
	rm -rf "$tmp_dir"
}

function usage(){
	exit_code="$1"
	cat <<EOF
Usage:

${script_name} [options] <args>

Args:
<go-version> : Install a specific go version.

Example:
${script_name} 1.10

Options
-d <path> : destination path, path where go will be installed.
-f        : Force remove old go version and install the specified one.
-h        : Show this help
-p        : Install go defined in ${PROJECT} versions file.

EOF

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
		p)	USE_VERSIONS_FILE="true" ;;
	esac
done

shift $(( $OPTIND - 1 ))

go_version="${1:-""}"

if [ -z "$go_version" ] && [ "${USE_VERSIONS_FILE}"  = "true" ] ;then
	go_version=$(get_from_kata_deps ".languages.golang.meta.newest-version")
fi

if [ -z "$go_version" ];then
	echo "Missing go version or -p option"
	usage 0
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

goarch=$(arch_to_golang)

info "Download go version ${go_version}"
kernel_name=$(uname -s)
curl -OL "https://storage.googleapis.com/golang/go${go_version}.${kernel_name,,}-${goarch}.tar.gz"
info "Install go"
mkdir -p "${install_dest}"
sudo tar -C "${install_dest}" -xzf "go${go_version}.${kernel_name,,}-${goarch}.tar.gz"
popd
