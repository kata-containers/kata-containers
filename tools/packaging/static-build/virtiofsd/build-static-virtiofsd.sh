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

   export RUSTFLAGS='-C target-feature=+crt-static -C link-self-contained=yes'
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

pull_virtiofsd_released_binary || build_virtiofsd_from_source
