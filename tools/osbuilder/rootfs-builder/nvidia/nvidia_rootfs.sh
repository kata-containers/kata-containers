#!/usr/bin/env bash
#
# Copyright (c) 2024 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail
[[ -n "${DEBUG}" ]] && set -x

# Error helpers
trap 'echo "rootfs: ERROR at line ${LINENO}: ${BASH_COMMAND}" >&2' ERR
die() {
  local msg="${*:-fatal error}"
  echo "rootfs: ${msg}" >&2
  exit 1
}

readonly BUILD_DIR="/kata-containers/tools/packaging/kata-deploy/local-build/build/"
# catch errors and then assign
script_dir="$(dirname "$(readlink -f "$0")")"
readonly SCRIPT_DIR="${script_dir}/nvidia"

KBUILD_SIGN_PIN=${KBUILD_SIGN_PIN:-}
AGENT_POLICY="${AGENT_POLICY:-no}"

NVIDIA_GPU_STACK=${NVIDIA_GPU_STACK:?NVIDIA_GPU_STACK must be set}
BUILD_VARIANT=${BUILD_VARIANT:?BUILD_VARIANT must be set}
ARCH=${ARCH:?ARCH must be set}

machine_arch="${ARCH}"

if [[ "${machine_arch}" == "aarch64" ]]; then
    distro_arch="arm64"
elif [[ "${machine_arch}" == "x86_64" ]]; then
    distro_arch="amd64"
else
    die "Unsupported architecture: ${machine_arch}"
fi

# The base-nvidia and gpu-addon images are carved out of the very same chiseled
# tree as the monolith, so they share the (expensive) driver stage-one with the
# monolithic "nvidia-gpu" build instead of rebuilding it per layout.
nvidia_stage_one_variant() {
	case "${BUILD_VARIANT}" in
		nvidia-gpu-base|nvidia-gpu-addon) echo "nvidia-gpu" ;;
		*) echo "${BUILD_VARIANT}" ;;
	esac
}

readonly stage_one="${BUILD_DIR:?}/rootfs-$(nvidia_stage_one_variant)-stage-one"

# Image layout produced from the chiseled tree:
#   monolith    - the full GPU image (default; unchanged behaviour)
#   base        - driver-agnostic base-nvidia (NVRC init + agent + base libs)
#   gpu-addon   - GPU userspace only, laid out for /run/kata-addons/gpu
nvidia_image_layout() {
	case "${BUILD_VARIANT}" in
		nvidia-gpu-base) echo "base" ;;
		nvidia-gpu-addon) echo "gpu-addon" ;;
		*) echo "monolith" ;;
	esac
}

setup_nvidia-nvrc() {
	# NVRC is built from a pinned git ref by the Kata "nvrc" static-build
	# target (tools/packaging/static-build/nvrc), which produces
	# kata-static-nvrc.tar.zst with the init binary under bin/. Unpack it
	# verbatim into the rootfs (./bin/NVRC-<arch>-unknown-linux-musl).
	local nvrc_tarball="${BUILD_DIR}/kata-static-nvrc.tar.zst"
	[[ -e "${nvrc_tarball}" ]] || \
		die "NVRC tarball not found: ${nvrc_tarball} (build the 'nvrc' target first)"
	# The tarball carries a ./bin/ entry; on a usr-merged rootfs /bin is a
	# symlink to usr/bin, so extract with --keep-directory-symlink to follow it
	# instead of clobbering the symlink with a real (near-empty) directory,
	# which would hide /bin/bash and break the driver chroot below.
	tar --keep-directory-symlink --zstd -xvf "${nvrc_tarball}" -C .
}

setup_nvidia_gpu_rootfs_stage_one() {
	local rootfs_type=${1:-""}

	if [[ -e "${stage_one}.tar.zst" ]]; then
		info "nvidia: GPU rootfs stage one already exists"
		return
	fi

	pushd "${ROOTFS_DIR:?}" >> /dev/null

	info "nvidia: Setup GPU rootfs type=${rootfs_type}"
	cp "${SCRIPT_DIR}/nvidia_chroot.sh" ./nvidia_chroot.sh

	chmod +x ./nvidia_chroot.sh

	setup_nvidia-nvrc

	local appendix=""
	if echo "${NVIDIA_GPU_STACK}" | grep -q '\<dragonball\>'; then
    		appendix="-dragonball-experimental"
	fi

	# Install the precompiled kernel modules shipped with the kernel
	mkdir -p ./lib/modules/
	tar --zstd -xvf "${BUILD_DIR}"/kata-static-kernel-nvidia-gpu"${appendix}"-modules.tar.zst -C ./lib/modules/

	mount --rbind /dev ./dev
	mount --make-rslave ./dev
	mount -t proc /proc ./proc

	local cuda_repo_url cuda_repo_pkg gpu_base_os_version ctk_version
	cuda_repo_url=$(get_package_version_from_kata_yaml "externals.nvidia.cuda.repo.${machine_arch}.url")
	cuda_repo_pkg=$(get_package_version_from_kata_yaml "externals.nvidia.cuda.repo.${machine_arch}.pkg")
	gpu_base_os_version=$(get_package_version_from_kata_yaml "assets.image.architecture.x86_64.nvidia-gpu.version")

	tools_repo_url=$(get_package_version_from_kata_yaml "externals.nvidia.tools.repo.${machine_arch}.url")
	tools_repo_pkg=$(get_package_version_from_kata_yaml "externals.nvidia.tools.repo.${machine_arch}.pkg")

	ctk_version=$(get_package_version_from_kata_yaml "externals.nvidia.ctk.version")

	chroot . /bin/bash -c "/nvidia_chroot.sh ${machine_arch} ${NVIDIA_GPU_STACK} \
		 ${gpu_base_os_version} ${cuda_repo_url} ${cuda_repo_pkg} ${tools_repo_url} ${tools_repo_pkg} ${ctk_version}"

	umount -R ./dev
	umount ./proc

	rm ./nvidia_chroot.sh

	tar cfa "${stage_one}.tar.zst" --remove-files -- *

	popd  >> /dev/null

	pushd "${BUILD_DIR}" >> /dev/null
	curl -LO "https://github.com/upx/upx/releases/download/v4.2.4/upx-4.2.4-${distro_arch}_linux.tar.xz"
	tar xvf "upx-4.2.4-${distro_arch}_linux.tar.xz"
	popd  >> /dev/null
}

chisseled_iptables() {
	echo "nvidia: chisseling iptables"
	cp -a "${stage_one}"/usr/sbin/xtables-nft-multi sbin/.

	ln -s ../sbin/xtables-nft-multi sbin/iptables-restore
	ln -s ../sbin/xtables-nft-multi sbin/iptables-save

	libdir=lib/"${machine_arch}"-linux-gnu
	cp -a "${stage_one}/${libdir}"/libmnl.so.0*      lib/.

	libdir=usr/lib/"${machine_arch}"-linux-gnu
	cp -a "${stage_one}/${libdir}"/libnftnl.so.11*   lib/.
	cp -a "${stage_one}/${libdir}"/libxtables.so.12* lib/.
}

# <= NVLINK4 nv-fabrimanager
# >= NVLINK5 nv-fabricmanager + nvlsm (TODO)
chisseled_nvswitch() {
	echo "nvidia: chisseling NVSwitch"

	mkdir -p usr/share/nvidia/nvswitch

	cp -a "${stage_one}"/usr/bin/nv-fabricmanager	bin/.
	cp -a "${stage_one}"/usr/share/nvidia/nvswitch	usr/share/nvidia/.

	libdir=usr/lib/"${machine_arch}"-linux-gnu
	cp -a "${stage_one}/${libdir}"/libnvidia-nscq.so.* lib/"${machine_arch}"-linux-gnu/.

	# NVLINK SubnetManager dependencies
	local nvlsm=usr/share/nvidia/nvlsm
	mkdir -p "${nvlsm}"

	cp -a "${stage_one}"/opt/nvidia/nvlsm/lib/libgrpc_mgr.so	lib/.
	cp -a "${stage_one}"/opt/nvidia/nvlsm/sbin/nvlsm			sbin/.
	cp -a "${stage_one}/${nvlsm}"/*.conf						"${nvlsm}"/.
	# Redirect all the logs to syslog instead of logging to file
	sed -i 's|^LOG_USE_SYSLOG=.*|LOG_USE_SYSLOG=1|' usr/share/nvidia/nvswitch/fabricmanager.cfg
}

chisseled_dcgm() {
	echo "nvidia: chisseling DCGM"

	mkdir -p etc/dcgm-exporter
	libdir=lib/"${machine_arch}"-linux-gnu

	cp -a "${stage_one}"/usr/"${libdir}"/libdcgm.*     "${libdir}"/.
	cp -a "${stage_one}"/"${libdir}"/libgcc_s.so.1*    "${libdir}"/.
	cp -a "${stage_one}"/usr/bin/nv-hostengine   bin/.
}

# copute always includes utility per default
chisseled_compute() {
	echo "nvidia: chisseling GPU"

	cp -a "${stage_one}"/lib/modules/* lib/modules/.

	libdir="lib/${machine_arch}-linux-gnu"
	cp -a "${stage_one}/${libdir}"/libdl.so.2*        	"${libdir}"/.
	cp -a "${stage_one}/${libdir}"/libz.so.1*         	"${libdir}"/.
	cp -a "${stage_one}/${libdir}"/libpthread.so.0*   	"${libdir}"/.
	cp -a "${stage_one}/${libdir}"/libresolv.so.2*    	"${libdir}"/.
	cp -a "${stage_one}/${libdir}"/libc.so.6*         	"${libdir}"/.
	cp -a "${stage_one}/${libdir}"/libm.so.6*         	"${libdir}"/.
	cp -a "${stage_one}/${libdir}"/librt.so.1*        	"${libdir}"/.
 	# nvidia-persistenced dependencies for CUDA repo and >= 590
	cp -a "${stage_one}/${libdir}"/libtirpc.so.3*    	"${libdir}"/.
	cp -a "${stage_one}/${libdir}"/libgssapi_krb5.so.2*	"${libdir}"/.
	cp -a "${stage_one}/${libdir}"/libkrb5.so.3*		"${libdir}"/.
	cp -a "${stage_one}/${libdir}"/libkrb5support.so.0*	"${libdir}"/.
	cp -a "${stage_one}/${libdir}"/libk5crypto.so.3*	"${libdir}"/.
	cp -a "${stage_one}/${libdir}"/libcom_err.so.2*		"${libdir}"/.
	cp -a "${stage_one}/${libdir}"/libkeyutils.so.1*	"${libdir}"/.
	cp -a "${stage_one}/etc/netconfig"	etc/.

	[[ "${type}" == "confidential" ]] && cp -a "${stage_one}/${libdir}"/libnvidia-pkcs11* 	"${libdir}"/.

	[[ ${machine_arch} == "aarch64" ]] && libdir="lib"
	[[ ${machine_arch} == "x86_64" ]]  && libdir="lib64"

	cp -aL "${stage_one}/${libdir}"/ld-linux-* "${libdir}"/.

	libdir=usr/lib/"${machine_arch}"-linux-gnu
	cp -a "${stage_one}/${libdir}"/libnv*        lib/"${machine_arch}"-linux-gnu/.
	cp -a "${stage_one}/${libdir}"/libcuda.so.*       lib/"${machine_arch}"-linux-gnu/.

	# basic GPU admin tools
	cp -a "${stage_one}"/usr/bin/nvidia-persistenced  bin/.
	cp -a "${stage_one}"/usr/bin/nvidia-smi           bin/.
	cp -a "${stage_one}"/usr/bin/nvidia-ctk           bin/.
	cp -a "${stage_one}"/usr/bin/nvidia-cdi-hook      bin/.
	ln -s ../bin usr/bin
}

chisseled_gpudirect() {
	echo "nvidia: chisseling GPUDirect"
	echo "nvidia: not implemented yet"
	exit 1
}

chisseled_nvat() {
	if [[ "${type}" != "confidential" ]]; then
                return
	fi

	echo "nvidia: chisseling NVAT"

	local libdir="lib/${machine_arch}-linux-gnu"

	# NVAT shared library (bundled via coco-guest-components tarball)
	cp -a "${stage_one}"/usr/local/lib/libnvat.so* "${libdir}"/.

	# NVAT runtime dependencies (per ldd on attestation-agent)
	cp -a "${stage_one}/${libdir}"/libxml2.so.2*     "${libdir}"/.
	cp -a "${stage_one}/${libdir}"/libstdc++.so.6*   "${libdir}"/.
	cp -a "${stage_one}/${libdir}"/liblzma.so.5*     "${libdir}"/.
	cp -a "${stage_one}/${libdir}"/libicuuc.so.*     "${libdir}"/.
	cp -a "${stage_one}/${libdir}"/libicudata.so.*   "${libdir}"/.
}

setup_nvrc_init_symlinks() {
	local nvrc="NVRC-${machine_arch}-unknown-linux-musl"
	# make sure NVRC is the init process for the initrd and image case
	ln -sf /bin/"${nvrc}" init
	ln -sf /bin/"${nvrc}" sbin/init
}

chisseled_init() {
	echo "nvidia: chisseling init"
	tar --zstd -xvf "${BUILD_DIR}"/kata-static-busybox.tar.zst -C .

	# Create bin/ and sbin/ explicitly rather than relying on busybox's
	# `make install` to emit them: busybox only creates a directory when it has
	# an applet living there, and once the nvidia busybox dropped modprobe (its
	# only /sbin applet, now provided by kmod) it stopped emitting sbin/, which
	# broke the `sbin/init` symlink below.
	mkdir -p dev etc proc run/cdi sys tmp usr var lib/modules lib/firmware \
		 usr/share/nvidia lib/"${machine_arch}"-linux-gnu lib64        \
		 bin sbin usr/bin etc/modprobe.d etc/ssl/certs

	ln -sf ../run var/run
	ln -sf ../run var/log
	ln -sf ../run var/cache

	# Needed for various RUST static builds with LIBC=gnu
	libdir=lib/"${machine_arch}"-linux-gnu
	cp -a "${stage_one}"/"${libdir}"/libgcc_s.so.1*    "${libdir}"/.

	local nvrc="NVRC-${machine_arch}-unknown-linux-musl"

	cp -a "${stage_one}/bin/${nvrc}" bin/.
	# Sigstore signature/certificate only exist when NVRC is pulled from the
	# official signed release; a source build (the composable-image path)
	# produces just the binary.
	[[ -e "${stage_one}/bin/${nvrc}.cert" ]] && cp -a "${stage_one}/bin/${nvrc}".cert bin/.
	[[ -e "${stage_one}/bin/${nvrc}.sig" ]] && cp -a "${stage_one}/bin/${nvrc}".sig bin/.

	setup_nvrc_init_symlinks

	cp -a "${stage_one}"/usr/bin/kata-agent   usr/bin/.
	if [[ "${AGENT_POLICY}" == "yes" ]]; then
		cp -a "${stage_one}"/etc/kata-opa etc/.
	fi
	cp -a "${stage_one}"/etc/resolv.conf      etc/.

	cp -a "${stage_one}"/lib/firmware/nvidia  lib/firmware/.
	cp -a "${stage_one}"/sbin/ldconfig.real   sbin/ldconfig

	cp -a "${stage_one}"/etc/ssl/certs/ca-certificates.crt etc/ssl/certs/.

	local conf_file="etc/modprobe.d/0000-nvidia.conf"
	echo 'options nvidia NVreg_DeviceFileMode=0660' > "${conf_file}"
}

compress_rootfs() {
	echo "nvidia: compressing rootfs"

	# For some unobvious reason libc has executable bit set
	# clean this up otherwise the find -executable will not work correctly
	find . -type f -name "*.so.*" | while IFS= read -r file; do
		if ! file "${file}" | grep -q ELF; then
			echo "nvidia: skip stripping file: ${file} ($(file -b "${file}"))"
			continue
		fi
		chmod -x "${file}"
		strip "${file}"
	done

	find . -type f -executable | while IFS= read -r file; do
		# Skip files with setuid/setgid bits (UPX refuses to pack them)
		if [[ -u "${file}" ]] || [[ -g "${file}" ]]; then
			echo "nvidia: skip compressing executable (special permissions): ${file} ($(file -b "${file}"))"
			continue
		fi
		if ! file "${file}" | grep -q ELF; then
			echo "nvidia: skip compressing executable (not ELF): ${file} ($(file -b "${file}"))"
			continue
		fi
		strip "${file}"
		"${BUILD_DIR}"/upx-4.2.4-"${distro_arch}"_linux/upx --best --lzma "${file}"
	done

 	# While I was playing with compression the executable flag on
	# /lib64/ld-linux-x86-64.so.2 was lost...
	# Since this is the program interpreter, it needs to be executable
	# as well.. sigh
	[[ ${machine_arch} == "aarch64" ]] && libdir="lib"
	[[ ${machine_arch} == "x86_64" ]]  && libdir="lib64"

	# The gpu-addon layout ships no program interpreter (it lives in the
	# base-nvidia image), so only fix up the loader when it is actually present.
	if compgen -G "${libdir}/ld-linux-*" > /dev/null; then
		chmod +x "${libdir}"/ld-linux-*
	fi
}

copy_cdh_runtime_deps() {
	local libdir="lib/${machine_arch}-linux-gnu"

	# Shared libraries required by /usr/local/bin/confidential-data-hub.
	cp -a "${stage_one}/${libdir}"/libgcc_s.so.1*          "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libm.so.6*              "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libc.so.6*              "${libdir}/."

	# Shared libraries required by the cryptsetup, mkfs.ext4, and dd binaries
	# used by CDH secure_mount.
	#
	# cryptsetup direct dependencies
	cp -a "${stage_one}/${libdir}"/libcryptsetup.so.12*    "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libpopt.so.0*           "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libuuid.so.1*           "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libblkid.so.1*          "${libdir}/."

	# libcryptsetup transitive dependencies
	cp -a "${stage_one}/${libdir}"/libdevmapper.so.1.02.1* "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libcrypto.so.3*         "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libargon2.so.1*         "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libjson-c.so.5*         "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libselinux.so.1*        "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libudev.so.1*           "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libpcre2-8.so.0*        "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libcap.so.2*            "${libdir}/."

	# e2fsprogs (mke2fs/mkfs.ext4) runtime libs
	cp -a "${stage_one}/${libdir}"/libext2fs.so.2*         "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libcom_err.so.2*        "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libe2p.so.2*            "${libdir}/."

	# cryptsetup, mkfs.ext4, and dd are used by CDH secure_mount.
	mkdir -p sbin etc bin
	cp -a "${stage_one}/sbin/cryptsetup" sbin/.
	cp -a "${stage_one}/sbin/mke2fs" sbin/.
	cp -a "${stage_one}/sbin/mkfs.ext4" sbin/.
	cp -a "${stage_one}/etc/mke2fs.conf" etc/.
	cp -a "${stage_one}/usr/bin/dd" bin/.
}

coco_guest_components() {
	if [[ "${type}" != "confidential" ]]; then
		return
	fi

	info "nvidia: installing the confidential containers guest components tarball"

	local -r coco_bin_dir="usr/local/bin"
	local -r etc_dir="etc"
	local -r pause_dir="pause_bundle"

	mkdir -p "${coco_bin_dir}"
	cp -a "${stage_one}/${coco_bin_dir}"/attestation-agent-nv  "${coco_bin_dir}/attestation-agent"
	cp -a "${stage_one}/${coco_bin_dir}"/api-server-rest       "${coco_bin_dir}/."
	cp -a "${stage_one}/${coco_bin_dir}"/confidential-data-hub "${coco_bin_dir}/."

	cp -a "${stage_one}/${etc_dir}"/ocicrypt_config.json "${etc_dir}/."

	mkdir -p "${pause_dir}/rootfs"
	cp -a "${stage_one}/${pause_dir}"/config.json  "${pause_dir}/."
	cp -a "${stage_one}/${pause_dir}"/rootfs/pause "${pause_dir}/rootfs/."

	copy_cdh_runtime_deps
}

# GPU userspace owned by the gpu addon. Anything not listed here stays in the
# driver-agnostic base-nvidia image. Paths are relative to the chiseled rootfs.
readonly nvidia_gpu_addon_bins=(
	bin/nvidia-smi
	bin/nvidia-ctk
	bin/nvidia-cdi-hook
	bin/nvidia-persistenced
	bin/nv-hostengine
	bin/nv-fabricmanager
	sbin/nvlsm
)

# GPU shared-library globs (inside the multiarch lib dir, plus libgrpc_mgr in
# /lib) owned by the gpu addon.
readonly nvidia_gpu_addon_lib_globs=(
	'libnv*'
	'libcuda.so*'
	'libdcgm.*'
	'libnvidia-nscq.so*'
)

# Lay the GPU userspace out for the addon mount (/run/kata-addons/gpu) and drop
# everything else. Matches how NVRC consumes the addon: bins from <root>/bin and
# <root>/sbin, libraries via LD_LIBRARY_PATH=<root>/usr/lib, kernel modules via
# `modprobe --dirname <root>`, configs from <root>/usr/share/nvidia, and
# <root>/lib/firmware/nvidia bind-mounted onto /lib/firmware/nvidia. Runs inside
# the (full, chiseled) ${stage_two}/${ROOTFS_DIR}.
#
# The GPU shared libraries go under <root>/usr/lib (not a flat <root>/lib) so
# that NVRC can run `nvidia-ctk cdi generate --driver-root=<root>`: nvidia-ctk
# records the in-container mount path as the host path with the driver root
# stripped, so libraries at <root>/usr/lib land at the canonical /usr/lib inside
# the container. Apps that scan /usr for the driver (e.g. NVIDIA NIM, which
# bails with "libnvidia-ml.so.1 not found under /usr") then find it; a flat
# <root>/lib would strip to /lib and hide the driver from those checks.
partition_gpu_addon() {
	echo "nvidia: building gpu addon layout"

	local addon
	addon="$(mktemp -d "${BUILD_DIR}/.nvidia-gpu-addon.XXXX")"
	mkdir -p "${addon}/bin" "${addon}/sbin" "${addon}/usr/lib" \
		 "${addon}/usr/share/nvidia" "${addon}/lib/firmware" "${addon}/lib/modules"

	local f
	for f in "${nvidia_gpu_addon_bins[@]}"; do
		[[ -e "${f}" ]] && install -D -m0755 "${f}" "${addon}/${f}"
	done

	# Collect the GPU shared libraries under <root>/usr/lib so both NVRC's
	# LD_LIBRARY_PATH and `nvidia-ctk --driver-root=<root>` resolve them and the
	# container sees them at /usr/lib (see header); libc/loader stay in the base.
	local md="lib/${machine_arch}-linux-gnu"
	local g
	for g in "${nvidia_gpu_addon_lib_globs[@]}"; do
		find "${md}" -maxdepth 1 -name "${g}" -exec cp -a {} "${addon}/usr/lib/" \;
	done
	[[ -e lib/libgrpc_mgr.so ]] && cp -a lib/libgrpc_mgr.so "${addon}/usr/lib/"

	# Materialize the SONAME symlinks (e.g. libcuda.so.1 -> libcuda.so.595.58.03)
	# inside the lib dir. The addon ships only the versioned files, so without
	# this `nvidia-ctk cdi generate` has no symlink to replicate into the
	# container (it reproduces existing links, it does not synthesize SONAMEs) and
	# the container can't resolve libcuda.so.1 -> it then falls back to the image's
	# older cuda-compat libcuda and CUDA fails with "driver version is insufficient
	# for CUDA runtime version". `ldconfig -n` only creates the versioned symlinks
	# in the given dir (no cache, no chroot), which is all the loader-less addon
	# needs. The monolith gets the same links via the `chroot . ldconfig` below.
	ldconfig -n "${addon}/usr/lib"

	# GPU configs (fabricmanager.cfg, nvlsm.conf, ...).
	[[ -d usr/share/nvidia ]] && cp -a usr/share/nvidia/. "${addon}/usr/share/nvidia/"

	# GPU firmware (GSP, ...); NVRC binds this onto /lib/firmware/nvidia.
	[[ -d lib/firmware/nvidia ]] && cp -a lib/firmware/nvidia "${addon}/lib/firmware/"

	# Ship a self-contained module tree so `modprobe --dirname <root>` resolves
	# the NVIDIA modules and their dependencies.
	cp -a lib/modules/. "${addon}/lib/modules/"
	if command -v depmod >/dev/null 2>&1; then
		local kdir kver
		for kdir in "${addon}"/lib/modules/*/; do
			[[ -d "${kdir}" ]] || continue
			kver="$(basename "${kdir}")"
			depmod -b "${addon}" "${kver}" || true
		done
	fi

	# Replace the rootfs with the addon-only content.
	find . -mindepth 1 -maxdepth 1 -exec rm -rf {} +
	cp -a "${addon}/." .
	rm -rf "${addon}"
}

# Strip the GPU userspace from the chiseled tree, leaving a driver-agnostic
# base-nvidia: NVRC init + kata-agent + busybox + loader/libc. No kernel
# modules are shipped (see below). The empty /lib/firmware/nvidia directory is
# kept as the bind mountpoint NVRC uses for the addon firmware. Runs inside
# ${ROOTFS_DIR}.
partition_base() {
	echo "nvidia: building driver-agnostic base layout"

	local f
	for f in "${nvidia_gpu_addon_bins[@]}"; do
		rm -f "${f}"
	done

	local md="lib/${machine_arch}-linux-gnu"
	local g
	for g in "${nvidia_gpu_addon_lib_globs[@]}"; do
		find "${md}" -maxdepth 1 -name "${g}" -delete
	done
	rm -f lib/libgrpc_mgr.so

	# GPU configs live in the addon; keep usr/share/nvidia as an empty stub.
	rm -rf usr/share/nvidia
	mkdir -p usr/share/nvidia

	# Keep /lib/firmware/nvidia as an empty mountpoint for NVRC's firmware bind.
	rm -rf lib/firmware/nvidia
	mkdir -p lib/firmware/nvidia

	# Ship no kernel modules in the base: the NVIDIA driver modules are
	# GPU-specific and live in the gpu addon (NVRC loads them via
	# `modprobe --dirname <addon>`), and the remaining in-tree dependencies
	# (mlx5, infiniband, ...) are built into the NVIDIA kernel. Keeping
	# /lib/modules empty is what makes the base driver-agnostic and reusable
	# across driver versions.
	rm -rf lib/modules
	mkdir -p lib/modules
}

# NVRC opens every cold-plugged addon (the gpu addon always, and the coco addon
# on confidential guests) as a dm-verity device by exec'ing /usr/sbin/veritysetup
# before mounting it. The base-nvidia image is the one that boots and runs NVRC,
# so it must carry veritysetup and its shared-library closure unconditionally -
# regardless of whether the guest is confidential (copy_cdh_runtime_deps only
# ships cryptsetup, and only on confidential builds). Runs inside ${ROOTFS_DIR}.
chisseled_veritysetup() {
	echo "nvidia: chisseling veritysetup"

	local libdir="lib/${machine_arch}-linux-gnu"

	# NVRC execs the absolute path /usr/sbin/veritysetup; cryptsetup-bin is
	# installed in the (usr-merged) stage-one, so /sbin/veritysetup resolves to
	# the real binary there.
	mkdir -p usr/sbin
	cp -a "${stage_one}/sbin/veritysetup" usr/sbin/.

	# veritysetup -> libcryptsetup runtime closure (same set cryptsetup links).
	cp -a "${stage_one}/${libdir}"/libcryptsetup.so.12*    "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libpopt.so.0*           "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libuuid.so.1*           "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libblkid.so.1*          "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libdevmapper.so.1.02.1* "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libcrypto.so.3*         "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libargon2.so.1*         "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libjson-c.so.5*         "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libselinux.so.1*        "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libudev.so.1*           "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libpcre2-8.so.0*        "${libdir}/."
	cp -a "${stage_one}/${libdir}"/libcap.so.2*            "${libdir}/."
}

# NVRC loads each addon's NVIDIA kernel modules from that addon's self-contained
# module tree via `modprobe --dirname <addon>` (a kmod feature). The base ships
# busybox, whose modprobe is built without long options and has no --dirname, so
# the base must carry the real kmod. This keeps modules composable: every
# module-bearing addon stays independent (its own modules.dep from depmod -b) and
# nothing has to shadow the read-only /lib/modules. kmod is a single multi-call
# binary that embeds libkmod (no libkmod2 to ship); modprobe/insmod/... are
# argv[0] symlinks to it. Runs inside ${ROOTFS_DIR}.
chisseled_kmod() {
	echo "nvidia: chisseling kmod"

	local libdir="lib/${machine_arch}-linux-gnu"

	cp -a "${stage_one}/usr/bin/kmod" usr/bin/.

	# kmod picks its applet from argv[0]. Expose the module tools as symlinks;
	# /sbin/modprobe (NVRC's absolute path) shadows the busybox modprobe applet.
	# Absolute targets so they resolve regardless of whether /sbin is a real dir
	# or a usr-merge symlink.
	local tool
	for tool in modprobe insmod rmmod depmod lsmod; do
		ln -sf /usr/bin/kmod "sbin/${tool}"
	done

	# kmod links libzstd/liblzma unconditionally (compressed-module support);
	# our modules are uncompressed but the NEEDED entries must still resolve.
	# libcrypto/libc are already present (libc from chisseled_compute, libcrypto
	# from chisseled_veritysetup).
	cp -a "${stage_one}/${libdir}"/libzstd.so.1*  "${libdir}/."
	cp -a "${stage_one}/${libdir}"/liblzma.so.5*  "${libdir}/."
}

setup_nvidia_gpu_rootfs_stage_two() {
	readonly stage_two="${ROOTFS_DIR:?}"
	readonly stack="${NVIDIA_GPU_STACK:?}"
	readonly type=${1:-""}
	local layout
	layout="$(nvidia_image_layout)"

	# If devkit flag is set, skip chisseling, use stage_one
	if echo "${stack}" | grep -q '\<devkit\>'; then
		echo "nvidia: devkit mode enabled - skip chisseling"

		tar -C "${stage_two}" -xf "${stage_one}".tar.zst

		pushd "${stage_two}" >> /dev/null

		# Only step needed from stage_two (see chisseled_init)
		setup_nvrc_init_symlinks
	else
		echo "nvidia: chisseling the following stack components: ${stack}"

		[[ -e "${stage_one}" ]] && rm -rf "${stage_one}"
		[[ ! -e "${stage_one}" ]] && mkdir -p "${stage_one}"

		tar -C "${stage_one}" -xf "${stage_one}".tar.zst

		pushd "${stage_two}" >> /dev/null

		# stage-one archives the full base+driver tree with `tar
		# --remove-files`, emptying ${stage_two} on its first run. When
		# stage-one is served from cache that emptying never happens, so the
		# freshly-built distro rootfs is still here. The chisel assembles a
		# minimal tree purely from ${stage_one}, so wipe any leftover distro
		# content first to keep the result deterministic regardless of whether
		# stage-one was cached (otherwise base/monolith layouts bloat with the
		# full Ubuntu rootfs).
		find . -mindepth 1 -maxdepth 1 -exec rm -rf {} +

		chisseled_init
		chisseled_iptables

		IFS=',' read -r -a stack_components <<< "${NVIDIA_GPU_STACK}"

		for component in "${stack_components[@]}"; do
			if [[ "${component}" = "compute" ]]; then
				echo "nvidia: processing \"compute\" component"
				chisseled_compute
			elif [[ "${component}" = "dcgm" ]]; then
				echo "nvidia: processing DCGM component"
				chisseled_dcgm
			elif [[ "${component}" = "nvswitch" ]]; then
				echo "nvidia: processing NVSwitch component"
				chisseled_nvswitch
			elif [[ "${component}" = "gpudirect" ]]; then
				echo "nvidia: processing GPUDirect component"
				chisseled_gpudirect
			fi
		done

		coco_guest_components
		chisseled_nvat

		# Carve the freshly chiseled (monolith) tree into the requested layout.
		# The monolith path is left untouched.
		case "${layout}" in
			base) partition_base; chisseled_veritysetup; chisseled_kmod ;;
			gpu-addon) partition_gpu_addon ;;
		esac
	fi

	compress_rootfs
	# The gpu addon has no loader/ldconfig of its own; its libraries are found
	# via NVRC's LD_LIBRARY_PATH, so skip the ld.so cache rebuild there. Its
	# SONAME symlinks are created with `ldconfig -n` in partition_gpu_addon().
	[[ "${layout}" != "gpu-addon" ]] && chroot . ldconfig

	popd >> /dev/null
}
