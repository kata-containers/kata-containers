#!/bin/bash
#
# Copyright (c) 2022 Intel
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${script_dir}/../../scripts/lib.sh"

tdshim_repo="${tdshim_repo:-}"
DESTDIR=${DESTDIR:-${PWD}}
PREFIX="${PREFIX:-/opt/kata}"

[ -n "${tdshim_repo}" ] || die "Failed to get TD-shim repo"
[ -n "${tdshim_version}" ] || die "Failed to get TD-shim version or commit"

info "Build ${tdshim_repo} version: ${tdshim_version}"

source ${HOME}/.cargo/env

build_root=$(mktemp -d)
pushd ${build_root}
git clone --single-branch "${tdshim_repo}"
pushd td-shim
git checkout "${tdshim_version}"
bash sh_script/build_final.sh boot_kernel

install_dir="${DESTDIR}/${PREFIX}/share/td-shim"
mkdir -p ${install_dir}
install target/x86_64-unknown-uefi/release/final-boot-kernel.bin ${install_dir}/td-shim.bin
popd #td-shim
popd #${build_root}

local_dir=${PWD}
pushd ${DESTDIR}
tar -czvf "${local_dir}/td-shim.tar.gz" "./$PREFIX"
rm -rf $(dirname ./$PREFIX)
popd #${DESTDIR}
