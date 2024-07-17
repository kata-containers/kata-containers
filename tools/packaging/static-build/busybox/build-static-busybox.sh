#!/usr/bin/env bash
#
# Copyright (c) 2024 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0


set -o errexit
set -o nounset
set -o pipefail

set -x

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck source=/dev/null
source "${script_dir}/../../scripts/lib.sh"

build_busybox_from_source()
{
	echo "build busybox from source"

	URL_TARBZ2="${BUSYBOX_URL:?}/busybox-${BUSYBOX_VERSION:?}.tar.bz2"
	URL_SHA="${BUSYBOX_URL:?}/busybox-${BUSYBOX_VERSION:?}.tar.bz2.sha256"
	URL_SIG="${BUSYBOX_URL:?}/busybox-${BUSYBOX_VERSION:?}.tar.bz2.sig"

	curl -O "${URL_TARBZ2}"
	curl -O "${URL_SHA}"
	curl -O "${URL_SIG}"

	echo "Verifying SHA256 checksum..."
	sha256_file="$(basename "${URL_SHA}")"
	sha256sum -c "${sha256_file}"

	gpg --keyserver hkps://keyserver.ubuntu.com --recv-keys C9E9416F76E610DBD09D040F47B70C55ACC9965B

	echo "Verifying GPG signature..."
	tarbz_file="$(basename "${URL_TARBZ2}")"
	sig_file="$(basename "${URL_SIG}")"

	gpg --verify "${sig_file}" "${tarbz_file}"

	tar xvf busybox-"${BUSYBOX_VERSION:?}".tar.bz2

	cd busybox-"${BUSYBOX_VERSION:?}"

	cp "${BUSYBOX_CONF_DIR:?}/${BUSYBOX_CONF_FILE:?}" .config

	# we do not want to install to CONFIG_PREFIX="./_install"
	# we want CONFIG_PREFIX="${DESTDIR}"
	sed -i "s|CONFIG_PREFIX=\"./_install\"|CONFIG_PREFIX=\"${DESTDIR}\"|g" .config

	make
	make install

}


build_busybox_from_source "$@"
