#!/usr/bin/env bash
#
# Copyright (c) 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

ARCH=$(uname -m)
ARCH_LIBC=""
LIBC=""

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${script_dir}/../../scripts/lib.sh"

virtiofsd_repo="${virtiofsd_repo:-}"
virtiofsd_version="${virtiofsd_version:-}"
virtiofsd_zip="${virtiofsd_zip:-}"

[ -n "$virtiofsd_repo" ] || die "failed to get virtiofsd repo"
[ -n "$virtiofsd_version" ] || die "failed to get virtiofsd version"
[ -n "${virtiofsd_zip}" ] || die "failed to get virtiofsd binary URL"

[ -d "virtiofsd" ] && rm -r virtiofsd

virtiofsd_latest_build_url="${jenkins_url}/job/kata-containers-2.0-virtiofsd-cc-$(uname -m)/${cached_artifacts_path}"

pull_virtiofsd_released_binary() {
	if [ "${ARCH}" != "x86_64" ]; then
		info "Only x86_64 binaries are distributed as part of the virtiofsd releases" && return 1
	fi
	info "Download virtiofsd version: ${virtiofsd_version}"

	mkdir -p virtiofsd

	pushd virtiofsd
	curl --fail -L ${virtiofsd_zip} -o virtiofsd.zip || return 1
	unzip virtiofsd.zip
	mv -f target/x86_64-unknown-linux-musl/release/virtiofsd virtiofsd
	chmod +x virtiofsd
	rm -rf target
	rm virtiofsd.zip
	popd
}

init_env() {
	source "$HOME/.cargo/env"

	extra_rust_flags=" -C link-self-contained=yes"
	case ${ARCH} in
		"aarch64")
			LIBC="musl"
			ARCH_LIBC=""
			;;
		"ppc64le")
			LIBC="gnu"
			ARCH="powerpc64le"
			ARCH_LIBC=${ARCH}-linux-${LIBC}
			;;
		"s390x")
			LIBC="gnu"
			ARCH_LIBC=${ARCH}-linux-${LIBC}
			extra_rust_flags=""
			;;
		"x86_64")
			LIBC="musl"
			ARCH_LIBC=""
			;;
	esac

}

build_virtiofsd_from_source() {
	echo "build viriofsd from source"
	init_env

	git clone --depth 1 --branch ${virtiofsd_version} ${virtiofsd_repo} virtiofsd
	pushd virtiofsd

	export RUSTFLAGS='-C target-feature=+crt-static'${extra_rust_flags}
	export LIBSECCOMP_LINK_TYPE=static
	export LIBSECCOMP_LIB_PATH=/usr/lib/${ARCH_LIBC}
	export LIBCAPNG_LINK_TYPE=static
	export LIBCAPNG_LIB_PATH=/usr/lib/${ARCH_LIBC}

	cargo build --release --target ${ARCH}-unknown-linux-${LIBC}

	binary=$(find ./ -name virtiofsd)
	mv -f ${binary} .
	chmod +x virtiofsd

	popd
}

check_cached_virtiofsd() {
	local current_virtiofsd_version=$(curl -sfL "${virtiofsd_latest_build_url}"/latest) || latest="none"
	info "Current virtiofsd version: ${current_virtiofsd_version}"
	info "Cached virtiofsd version: ${cached_virtiofsd_version}"
	if [ "${current_virtiofsd_version}" == "${cached_virtiofsd_version}" ] && [ "$(uname -m)" == "x86_64" ]; then
		install_cached_virtiofsd
	else
		pull_virtiofsd_released_binary || build_virtiofsd_from_source
	fi
}

install_cached_virtiofsd() {
	local cached_path="$(echo ${script_dir} | sed 's,/*[^/]\+/*$,,' | sed 's,/*[^/]\+/*$,,' | sed 's,/*[^/]\+/*$,,' | sed 's,/*[^/]\+/*$,,')"
	local virtiofsd_directory="${cached_path}/tools/packaging/kata-deploy/local-build/build/cc-virtiofsd/builddir/virtiofsd"
	local checksum_file="sha256sum-virtiofsd"
	info "Downloading virtiofsd binary"
	curl -fOL --progress-bar "${virtiofsd_latest_build_url}/virtiofsd" || return 1
	info "Checking virtiofsd binary checksum"
	curl -fOL --progress-bar "${virtiofsd_latest_build_url}/${checksum_file}" || return 1
	info "Verify checksum"
	sudo sha256sum -c "${checksum_file}" || return 1
	chmod +x virtiofsd
}


main() {
	check_cached_virtiofsd
}
main $*
