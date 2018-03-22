#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# Currently we will use this repository until this issue is solved
# See https://github.com/kata-containers/packaging/issues/1

set -e

cidir=$(dirname "$0")
source "${cidir}/lib.sh"

repo_owner="clearcontainers"
repo_name="linux"

linux_releases_url="https://github.com/${repo_owner}/${repo_name}/releases"
#fake repository dir to query kernel version from remote
fake_repo_dir=$(mktemp -t -d kata-kernel.XXXX)

function cleanup {
	rm  -rf "${fake_repo_dir}"
}

trap cleanup EXIT

function usage() {
	cat << EOT
Usage: $0 <version>
Install the containers clear kernel image <version> from "${repo_owner}/${repo_name}".

version: Use 'latest' to pull latest kernel or a version from "${cc_linux_releases_url}"
EOT

	exit 1
}

#Get latest version by checking remote tags
#We dont ask to github api directly because force a user to provide a GITHUB token
function get_latest_version {
	pushd "${fake_repo_dir}" >> /dev/null
	git init -q
	git remote add origin  https://github.com/"${repo_owner}/${repo_name}".git

	cc_release=$(git ls-remote --tags 2>/dev/null \
		| grep -oP '\-\d+\.container'  \
			| grep -oP '\d+' \
				| sort -n | \
					tail -1 )

	tag=$(git ls-remote --tags 2>/dev/null \
		| grep -oP "v\d+\.\d+\.\d+\-${cc_release}.container" \
			| tail -1)

	popd >> /dev/null
	echo "${tag}"
}

function download_kernel() {
	local version="$1"
	arch=$(arch)
	[ -n "${version}" ] || die "version not provided"
	[ "${version}" == "latest" ] && version=$(get_latest_version)
	echo "version to install ${version}"
	local binaries_dir="${version}-binaries"
	local binaries_tarball="${binaries_dir}.tar.gz"
	local shasum_file="SHA512SUMS"
	if [ "$arch" = x86_64 ]; then
		curl -OL "${linux_releases_url}/download/${version}/${binaries_tarball}"
		curl -OL "${linux_releases_url}/download/${version}/${shasum_file}"
		sha512sum -c "${shasum_file}"
		tar xf "${binaries_tarball}"
	else
        	die "Unsupported architecture: $arch"
	fi

	pushd "${binaries_dir}"
	sudo make install
	popd
}

cc_kernel_version="$1"

[ -z "${cc_kernel_version}" ] && usage
download_kernel "${cc_kernel_version}"

# Make symbolic link to kata-containers
# FIXME: see https://github.com/kata-containers/packaging/issues/1
sudo ln -sf /usr/share/clear-containers/vmlinux.container /usr/share/kata-containers/
sudo ln -sf /usr/share/clear-containers/vmlinuz.container /usr/share/kata-containers/
