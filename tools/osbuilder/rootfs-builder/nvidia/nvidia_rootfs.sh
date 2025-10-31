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
VARIANT=${VARIANT:?VARIANT must be set}
ARCH=${ARCH:?ARCH must be set}

machine_arch="${ARCH}"

if [[ "${machine_arch}" == "aarch64" ]]; then
    distro_arch="arm64"
elif [[ "${machine_arch}" == "x86_64" ]]; then
    distro_arch="amd64"
else
    die "Unsupported architecture: ${machine_arch}"
fi

readonly stage_one="${BUILD_DIR:?}/rootfs-${VARIANT:?}-stage-one"

setup_nvidia-nvrc() {
	local rootfs_type=${1:-""}

	BIN="NVRC${rootfs_type:+"-${rootfs_type}"}"
	TARGET=${machine_arch}-unknown-linux-musl
	URL=$(get_package_version_from_kata_yaml "externals.nvrc.url")
	VER=$(get_package_version_from_kata_yaml "externals.nvrc.version")

	local DL="${URL}/${VER}"
	curl -fsSL -o "${BUILD_DIR}/${BIN}-${TARGET}.tar.xz" "${DL}/${BIN}-${TARGET}.tar.xz"
	curl -fsSL -o "${BUILD_DIR}/${BIN}-${TARGET}.tar.xz.sig" "${DL}/${BIN}-${TARGET}.tar.xz.sig"
	curl -fsSL -o "${BUILD_DIR}/${BIN}-${TARGET}.tar.xz.cert" "${DL}/${BIN}-${TARGET}.tar.xz.cert"

	ID="^https://github.com/NVIDIA/nvrc/.github/workflows/.+@refs/heads/main$"
	OIDC="https://token.actions.githubusercontent.com"

	# Only allow releases from the NVIDIA/nvrc main branch and build by github actions
	cosign verify-blob                                          \
	  --rekor-url https://rekor.sigstore.dev                    \
	  --certificate "${BUILD_DIR}/${BIN}-${TARGET}.tar.xz.cert" \
	  --signature   "${BUILD_DIR}/${BIN}-${TARGET}.tar.xz.sig"  \
	  --certificate-identity-regexp "${ID}"                     \
	  --certificate-oidc-issuer "${OIDC}"                       \
	  "${BUILD_DIR}/${BIN}-${TARGET}.tar.xz"
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

	local BIN="NVRC${rootfs_type:+"-${rootfs_type}"}"
	local TARGET=${machine_arch}-unknown-linux-musl
	if [[ ! -e  "${BUILD_DIR}/${BIN}-${TARGET}.tar.xz" ]]; then
		setup_nvidia-nvrc "${rootfs_type}"
	fi
	tar -xvf "${BUILD_DIR}/${BIN}-${TARGET}.tar.xz" -C ./bin/

	local appendix="${rootfs_type:+"-${rootfs_type}"}"
	if echo "${NVIDIA_GPU_STACK}" | grep -q '\<dragonball\>'; then
    		appendix="-dragonball-experimental"
	fi

	# We need the kernel packages for building the drivers cleanly will be
	# deinstalled and removed from the roofs once the build finishes.
	tar --zstd -xvf "${BUILD_DIR}"/kata-static-kernel-nvidia-gpu"${appendix}"-headers.tar.zst -C .

	# If we find a local downloaded run file build the kernel modules
	# with it, otherwise use the distribution packages. Run files may have
	# more recent drivers available then the distribution packages.
	local run_file_name="nvidia-driver.run"
	if [[ -f ${BUILD_DIR}/${run_file_name} ]]; then
		cp -L "${BUILD_DIR}"/"${run_file_name}" ./"${run_file_name}"
	fi

	local run_fm_file_name="nvidia-fabricmanager.run"
	if [[ -f ${BUILD_DIR}/${run_fm_file_name} ]]; then
		cp -L "${BUILD_DIR}"/"${run_fm_file_name}" ./"${run_fm_file_name}"
	fi

	mount --rbind /dev ./dev
	mount --make-rslave ./dev
	mount -t proc /proc ./proc

	chroot . /bin/bash -c "/nvidia_chroot.sh $(uname -r) ${run_file_name} \
		${run_fm_file_name} ${machine_arch} ${NVIDIA_GPU_STACK} ${KBUILD_SIGN_PIN}"

	umount -R ./dev
	umount ./proc

	rm ./nvidia_chroot.sh
	rm ./*.deb

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

	cp -a "${stage_one}"/usr/bin/nv-fabricmanager 	bin/.
	cp -a "${stage_one}"/usr/share/nvidia/nvswitch usr/share/nvidia/.

	libdir=usr/lib/"${machine_arch}"-linux-gnu

	cp -a "${stage_one}/${libdir}"/libnvidia-nscq.so.* lib/"${machine_arch}"-linux-gnu/.

	# Logs will be redirected to console(stderr)
	# if the specified log file can't be opened or the path is empty.
	# LOG_FILE_NAME=/var/log/fabricmanager.log -> setting to empty for stderr -> kmsg
	sed -i 's|^LOG_FILE_NAME=.*|LOG_FILE_NAME=|' usr/share/nvidia/nvswitch/fabricmanager.cfg
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

	cp -a "${stage_one}"/nvidia_driver_version .
	cp -a "${stage_one}"/lib/modules/* lib/modules/.

	libdir="lib/${machine_arch}-linux-gnu"
	cp -a "${stage_one}/${libdir}"/libdl.so.2*        	"${libdir}"/.
	cp -a "${stage_one}/${libdir}"/libz.so.1*         	"${libdir}"/.
	cp -a "${stage_one}/${libdir}"/libpthread.so.0*   	"${libdir}"/.
	cp -a "${stage_one}/${libdir}"/libresolv.so.2*    	"${libdir}"/.
	cp -a "${stage_one}/${libdir}"/libc.so.6*         	"${libdir}"/.
	cp -a "${stage_one}/${libdir}"/libm.so.6*         	"${libdir}"/.
	cp -a "${stage_one}/${libdir}"/librt.so.1*        	"${libdir}"/.

	[[ "${type}" == "confidential" ]] && cp -a "${stage_one}/${libdir}"/libnvidia-pkcs11* 	"${libdir}"/.

	[[ ${machine_arch} == "aarch64" ]] && libdir="lib"
	[[ ${machine_arch} == "x86_64" ]]  && libdir="lib64"

	cp -aL "${stage_one}/${libdir}"/ld-linux-* "${libdir}"/.

	libdir=usr/lib/"${machine_arch}"-linux-gnu
	cp -a "${stage_one}/${libdir}"/libnvidia-ml.so.*  lib/"${machine_arch}"-linux-gnu/.
	cp -a "${stage_one}/${libdir}"/libcuda.so.*       lib/"${machine_arch}"-linux-gnu/.
	cp -a "${stage_one}/${libdir}"/libnvidia-cfg.so.* lib/"${machine_arch}"-linux-gnu/.

	# basich GPU admin tools
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

chisseled_init() {
	local rootfs_type=${1:-""}

	echo "nvidia: chisseling init"
	tar --zstd -xvf "${BUILD_DIR}"/kata-static-busybox.tar.zst -C .

	mkdir -p dev etc proc run/cdi sys tmp usr var lib/modules lib/firmware \
		 usr/share/nvidia lib/"${machine_arch}"-linux-gnu lib64        \
		 usr/bin etc/modprobe.d etc/ssl/certs

	ln -sf ../run var/run

	# Needed for various RUST static builds with LIBC=gnu
	libdir=lib/"${machine_arch}"-linux-gnu
	cp -a "${stage_one}"/"${libdir}"/libgcc_s.so.1*    "${libdir}"/.

	bin="NVRC${rootfs_type:+"-${rootfs_type}"}"
	target=${machine_arch}-unknown-linux-musl

	cp -a "${stage_one}/bin/${bin}-${target}"      bin/.
	cp -a "${stage_one}/bin/${bin}-${target}".cert bin/.
	cp -a "${stage_one}/bin/${bin}-${target}".sig  bin/.

	# make sure NVRC is the init process for the initrd and image case
	ln -sf  /bin/"${bin}-${target}" init
	ln -sf  /bin/"${bin}-${target}" sbin/init

	cp -a "${stage_one}"/usr/bin/kata-agent   usr/bin/.
	if [[ "${AGENT_POLICY}" == "yes" ]]; then
		cp -a "${stage_one}"/etc/kata-opa etc/.
	fi
	cp -a "${stage_one}"/etc/resolv.conf      etc/.
	cp -a "${stage_one}"/supported-gpu.devids .

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
		chmod -x "${file}"
		strip "${file}"
	done

	find . -type f -executable | while IFS= read -r file; do
		strip "${file}"
		"${BUILD_DIR}"/upx-4.2.4-"${distro_arch}"_linux/upx --best --lzma "${file}"
	done

 	# While I was playing with compression the executable flag on
	# /lib64/ld-linux-x86-64.so.2 was lost...
	# Since this is the program interpreter, it needs to be executable
	# as well.. sigh
	[[ ${machine_arch} == "aarch64" ]] && libdir="lib"
	[[ ${machine_arch} == "x86_64" ]]  && libdir="lib64"

	chmod +x "${libdir}"/ld-linux-*

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
	cp -a "${stage_one}/${coco_bin_dir}"/attestation-agent     "${coco_bin_dir}/."
	cp -a "${stage_one}/${coco_bin_dir}"/api-server-rest       "${coco_bin_dir}/."
	cp -a "${stage_one}/${coco_bin_dir}"/confidential-data-hub "${coco_bin_dir}/."

	cp -a "${stage_one}/${etc_dir}"/ocicrypt_config.json "${etc_dir}/."

	mkdir -p "${pause_dir}/rootfs"
	cp -a "${stage_one}/${pause_dir}"/config.json  "${pause_dir}/."
	cp -a "${stage_one}/${pause_dir}"/rootfs/pause "${pause_dir}/rootfs/."

	info "TODO: nvidia: luks-encrypt-storage is a bash script, we do not have a shell!"
}

toggle_debug() {
	if echo "${NVIDIA_GPU_STACK}" | grep -q '\<debug\>'; then
		export DEBUG="true"
	fi
}

setup_nvidia_gpu_rootfs_stage_two() {
	readonly stage_two="${ROOTFS_DIR:?}"
	readonly stack="${NVIDIA_GPU_STACK:?}"

	readonly type=${1:-""}

	echo "nvidia: chisseling the following stack components: ${stack}"


	[[ -e "${stage_one}" ]] && rm -rf "${stage_one}"
	[[ ! -e "${stage_one}" ]] && mkdir -p "${stage_one}"

	tar -C "${stage_one}" -xf "${stage_one}".tar.zst


	pushd "${stage_two}" >> /dev/null

	toggle_debug
	chisseled_init "${type}"
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

	compress_rootfs

	chroot . ldconfig

	popd  >> /dev/null
}
