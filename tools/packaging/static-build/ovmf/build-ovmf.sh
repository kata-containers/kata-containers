#!/bin/bash
#
# Copyright (c) 2022 IBM
# Copyright (c) 2022 Intel
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${script_dir}/../../scripts/lib.sh"

# disabling set -u because scripts attempt to expand undefined variables
set +u
ovmf_build="${ovmf_build:-x86_64}"
ovmf_repo="${ovmf_repo:-}"
ovmf_version="${ovmf_version:-}"
ovmf_package="${ovmf_package:-}"
package_output_dir="${package_output_dir:-}"
DESTDIR=${DESTDIR:-${PWD}}
PREFIX="${PREFIX:-/opt/kata}"
architecture="${architecture:-X64}"
toolchain="${toolchain:-GCC5}"
build_target="${build_target:-RELEASE}"

[ -n "$ovmf_repo" ] || die "failed to get ovmf repo"
[ -n "$ovmf_version" ] || die "failed to get ovmf version or commit"
[ -n "$ovmf_package" ] || die "failed to get ovmf package or commit"
[ -n "$package_output_dir" ] || die "failed to get ovmf package or commit"

ovmf_dir="${ovmf_repo##*/}"

info "Build ${ovmf_repo} version: ${ovmf_version}"

build_root=$(mktemp -d)
pushd $build_root
git clone --single-branch --depth 1 -b "${ovmf_version}" "${ovmf_repo}"
cd "${ovmf_dir}"
git submodule init
git submodule update

info "Using BaseTools make target"
make -C BaseTools/

info "Calling edksetup script"
source edksetup.sh

if [ "${ovmf_build}" == "sev" ]; then
	info "Creating dummy grub file"
	#required for building AmdSev package without grub
	touch OvmfPkg/AmdSev/Grub/grub.efi
fi

info "Building ovmf"
build_cmd="build -b ${build_target} -t ${toolchain} -a ${architecture} -p ${ovmf_package}"
if [ "${ovmf_build}" == "tdx" ]; then
	build_cmd+=" -D SECURE_BOOT_ENABLE=TRUE"
fi

eval "${build_cmd}"

info "Done Building"

build_path_target_toolchain="Build/${package_output_dir}/${build_target}_${toolchain}"
build_path_fv="${build_path_target_toolchain}/FV"
stat "${build_path_fv}/OVMF.fd"
if [ "${ovmf_build}" == "tdx" ]; then
	build_path_arch="${build_path_target_toolchain}/X64"
	stat "${build_path_fv}/OVMF_CODE.fd"
	stat "${build_path_fv}/OVMF_VARS.fd"
fi

#need to leave tmp dir
popd

info "Install fd to destdir"
install_dir="${DESTDIR}/${PREFIX}/share/ovmf"

mkdir -p "${install_dir}"
if [ "${ovmf_build}" == "sev" ]; then
	install $build_root/$ovmf_dir/"${build_path_fv}"/OVMF.fd "${install_dir}/AMDSEV.fd"
else
	install $build_root/$ovmf_dir/"${build_path_fv}"/OVMF.fd "${install_dir}"
fi
if [ "${ovmf_build}" == "tdx" ]; then
	install $build_root/$ovmf_dir/"${build_path_fv}"/OVMF_CODE.fd ${install_dir}
	install $build_root/$ovmf_dir/"${build_path_fv}"/OVMF_VARS.fd ${install_dir}
fi

local_dir=${PWD}
pushd $DESTDIR
tar -czvf "${local_dir}/${ovmf_dir}-${ovmf_build}.tar.gz" "./$PREFIX"
rm -rf $(dirname ./$PREFIX)
popd
