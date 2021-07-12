#!/bin/bash
#
# Copyright 2021 Sony Group Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit

cidir=$(dirname "$0")
source "${cidir}/lib.sh"

clone_tests_repo

source "${tests_repo_dir}/.ci/lib.sh"

# Variables for libseccomp
arch=$(uname -m)
libseccomp_version=$(get_version "externals.libseccomp.version")
libseccomp_url=$(get_version "externals.libseccomp.url")
libseccomp_tarball="libseccomp-${libseccomp_version}.tar.gz"
libseccomp_tarball_url="${libseccomp_url}/releases/download/v${libseccomp_version}/${libseccomp_tarball}"
libseccomp_install_dir="$1"
cflags="-O2"

# Variables for gperf
gperf_version=$(get_version "externals.gperf.version")
gperf_url=$(get_version "externals.gperf.url")
gperf_tarball="gperf-${gperf_version}.tar.gz"
gperf_tarball_url="${gperf_url}/${gperf_tarball}"
gperf_install_dir="$2"

# We need to build the libseccomp library from sources to create a static library for the musl libc.
# However, ppc64le and s390x have no musl targets in Rust. Hence, we do not set cflags for the musl libc.
if ([ "${arch}" != "ppc64le" ] && [ "${arch}" != "s390x" ]); then
    # Set FORTIFY_SOURCE=1 because the musl-libc does not have some functions about FORTIFY_SOURCE=2
    cflags="-U_FORTIFY_SOURCE -D_FORTIFY_SOURCE=1 -O2"
fi

finish() {
    rm -rf "${libseccomp_tarball}" "libseccomp-${libseccomp_version}" "${gperf_tarball}" "gperf-${gperf_version}"
}

trap finish EXIT

build_and_install_gperf() {
    echo "Build and install gperf version ${gperf_version}"
    sudo mkdir -p "${gperf_install_dir}"
    curl -sLO "${gperf_tarball_url}"
    tar -xf "${gperf_tarball}"
    pushd "gperf-${gperf_version}"
    ./configure --prefix="${gperf_install_dir}"
    make
    sudo make install
    export PATH=$PATH:"${gperf_install_dir}"/bin
    popd
    echo "Gperf installed successfully"
}

build_and_install_libseccomp() {
    echo "Build and install libseccomp version ${libseccomp_version}"
    sudo mkdir -p "${libseccomp_install_dir}"
    curl -sLO "${libseccomp_tarball_url}"
    tar -xf "${libseccomp_tarball}"
    pushd "libseccomp-${libseccomp_version}"
    ./configure --prefix="${libseccomp_install_dir}" CFLAGS="${cflags}" --enable-static
    make
    sudo make install
    popd
    echo "Libseccomp installed successfully"
}

main() {
    # gperf is required for building the libseccomp.
    build_and_install_gperf
    build_and_install_libseccomp
}

main
