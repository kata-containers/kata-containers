#!/usr/bin/env bash
#
# Copyright (c) 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck source=/dev/null
source "${script_dir}/../../scripts/lib.sh"

# shellcheck disable=SC2154
[[ -n "${coco_guest_components_repo}" ]] || die "failed to get coco-guest-components repo"
# shellcheck disable=SC2154
[[ -n "${coco_guest_components_version}" ]] || die "failed to get coco-guest-components version"

[[ -d "guest-components" ]] && rm -rf  guest-components

build_coco_guest_components_from_source() {
	echo "build coco-guest-components from source"

	# shellcheck source=/dev/null
	. /etc/profile.d/rust.sh

	git clone --depth 1 "${coco_guest_components_repo}" guest-components
	pushd guest-components

	git fetch --depth=1 origin "${coco_guest_components_version}"
	git checkout FETCH_HEAD

	# shellcheck disable=SC2154
	DESTDIR="${DESTDIR}/usr/local/bin" TEE_PLATFORM=${TEE_PLATFORM} make build
	# shellcheck disable=SC2154
	strip "target/${RUST_ARCH}-unknown-linux-${LIBC}/release/confidential-data-hub"
	strip "target/${RUST_ARCH}-unknown-linux-${LIBC}/release/attestation-agent"
	strip "target/${RUST_ARCH}-unknown-linux-${LIBC}/release/api-server-rest"
	DESTDIR="${DESTDIR}/usr/local/bin" TEE_PLATFORM=${TEE_PLATFORM} make install

	install -D -m0644 "confidential-data-hub/hub/src/image/ocicrypt_config.json" "${DESTDIR}/etc/ocicrypt_config.json"

	# CDH's secure_mount LUKS-formats encrypted scratch volumes by exec'ing
	# cryptsetup. Encrypted storage is a CoCo-only feature, so cryptsetup ships
	# in this extension rather than the guest rootfs (the base-nvidia image carries
	# only veritysetup plus the plain mke2fs/mkfs.ext4/dd storage tooling).
	# cryptsetup's shared-library closure is identical to veritysetup's, which
	# the base already ships unconditionally, so bundle just the binary; the
	# coco-extension manifest puts ${extension_root}/usr/sbin on CDH's PATH so the
	# runtime lookup resolves (see kata-deploy-binaries.sh).
	install -D -m0755 /usr/sbin/cryptsetup "${DESTDIR}/usr/sbin/cryptsetup"

	if [[ -n "${NV_ATTESTER:-}" ]]; then
		echo "build attestation-agent-nv with nvidia-attester support"

		rm "target/${RUST_ARCH}-unknown-linux-${LIBC}/release/attestation-agent"

		ATTESTER="${NV_ATTESTER}" NVAT_USE_SYSTEM_LIB=1 RUSTFLAGS="-L /usr/local/lib" \
			DESTDIR="${DESTDIR}/usr/local/bin" TEE_PLATFORM=${TEE_PLATFORM} make build
		strip "target/${RUST_ARCH}-unknown-linux-${LIBC}/release/attestation-agent"
		install -D -m0755 "target/${RUST_ARCH}-unknown-linux-${LIBC}/release/attestation-agent" \
			"${DESTDIR}/usr/local/bin/attestation-agent-nv"

		mkdir -p "${DESTDIR}/usr/local/lib"
		cp -a /usr/local/lib/libnvat.so* "${DESTDIR}/usr/local/lib/"

		# attestation-agent-nv links libnvat.so, which in turn pulls in
		# libxml2/zlib/lzma and the C++ runtime. None of those ship in the
		# guest rootfs, so bundle every non-glibc dependency next to
		# libnvat.so. The coco-extension manifest points the nvidia attester's
		# LD_LIBRARY_PATH here, so the dynamic linker resolves them at runtime.
		ldd /usr/local/lib/libnvat.so | awk '/=> \// { print $3 }' | while read -r dep; do
			case "$(basename "${dep}")" in
				libc.so.*|libm.so.*|libdl.so.*|libpthread.so.*|librt.so.*|ld-linux*|linux-vdso*) continue ;;
			esac
			install -D -m0755 "${dep}" "${DESTDIR}/usr/local/lib/$(basename "${dep}")"
		done
	fi

	popd
}

build_coco_guest_components_from_source "$@"
