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

# Path to the ORAS cache helper for downloading tarballs (sourced when needed)
oras_cache_helper="${script_dir}/../../scripts/download-with-oras-cache.sh"

# Use ORAS cache for busybox downloads (busybox.net can be unreliable)
USE_ORAS_CACHE="${USE_ORAS_CACHE:-yes}"

download_busybox_tarball()
{
	local tarball_name="busybox-${BUSYBOX_VERSION:?}.tar.bz2"

	# Use ORAS cache if available and enabled
	if [[ "${USE_ORAS_CACHE}" == "yes" ]] && [[ -f "${oras_cache_helper}" ]]; then
		echo "Using ORAS cache for busybox download"
		# shellcheck source=/dev/null
		source "${oras_cache_helper}"
		BUSYBOX_TARBALL=$(download_component busybox "$(pwd)")
		if [[ -f "${BUSYBOX_TARBALL}" ]]; then
			echo "Busybox tarball downloaded from cache: ${BUSYBOX_TARBALL}"
			return 0
		fi
		echo "ORAS cache download failed, falling back to direct download"
	fi

	# Fallback to direct download
	BUSYBOX_TARBALL="busybox-${BUSYBOX_VERSION:?}.tar.bz2"
	URL_TARBZ2="${BUSYBOX_URL:?}/${BUSYBOX_TARBALL}"
	URL_SHA="${BUSYBOX_URL:?}/${BUSYBOX_TARBALL}.sha256"
	URL_SIG="${BUSYBOX_URL:?}/${BUSYBOX_TARBALL}.sig"

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
}

build_busybox_from_source()
{
	echo "build busybox from source"

	download_busybox_tarball

	tar xvf "${BUSYBOX_TARBALL}"

	cd busybox-"${BUSYBOX_VERSION:?}"

	cp "${BUSYBOX_CONF_DIR:?}/${BUSYBOX_CONF_FILE:?}" .config

	# we do not want to install to CONFIG_PREFIX="./_install"
	# we want CONFIG_PREFIX="${DESTDIR}"
	sed -i "s|CONFIG_PREFIX=\"./_install\"|CONFIG_PREFIX=\"${DESTDIR}\"|g" .config

	make -j "$(nproc)"
	make install

}


build_busybox_from_source "$@"
