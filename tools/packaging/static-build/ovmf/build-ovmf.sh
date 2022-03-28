#!/bin/bash
#
# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${script_dir}/../../scripts/lib.sh"

ovmf_repo="${ovmf_repo:-}"
ovmf_dir="edk2"
ovmf_version="${ovmf_version:-}"
kata_version="${kata_version:-}"

if [ -z "$ovmf_repo" ]; then
       info "Get ovmf information from runtime versions.yaml"
       ovmf_url=$(get_from_kata_deps "externals.ovmf.url" "${kata_version}")
       [ -n "$ovmf_url" ] || die "failed to get ovmf url"
       ovmf_repo="${ovmf_url}.git"
fi

[ -n "$ovmf_repo" ] || die "failed to get ovmf repo"

[ -n "$ovmf_version" ] || ovmf_version=$(get_from_kata_deps "externals.ovmf.version" "${kata_version}")
[ -n "$ovmf_version" ] || die "failed to get ovmf version or commit"

info "Build ${ovmf_repo} version: ${ovmf_version}"

[ -d "${ovmf_dir}" ] || git clone ${ovmf_repo}
cd "${ovmf_dir}"
git checkout "${ovmf_version}"
git submodule init
git submodule update

info "Using BaseTools make target"
set +u
make -C BaseTools/
set -u

info "Calling edksetup scipt"
# disabling set -u because edksetup.sh attempts to expands undefined variables
set +u
source edksetup.sh
set -u

info "Creating dummy grub file"
#required for building AmdSev package without grub
touch OvmfPkg/AmdSev/Grub/grub.efi

info "Building ovmf"
build -t GCC5 -a X64 -p OvmfPkg/AmdSev/AmdSevX64.dsc

info "Done Building"
pwd
stat Build/AmdSev/DEBUG_GCC5/FV/OVMF.fd 

info "Install fd to destdir"
mkdir -p ../../destdir/opt/kata/share/ovmf
cp Build/AmdSev/DEBUG_GCC5/FV/OVMF.fd ../../destdir/opt/kata/share/ovmf