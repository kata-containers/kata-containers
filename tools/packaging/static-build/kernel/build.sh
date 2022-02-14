#!/usr/bin/env bash
#
# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly repo_root_dir="$(cd "${script_dir}/../../../.." && pwd)"
readonly kernel_builder="${repo_root_dir}/tools/packaging/kernel/build-kernel.sh"


DESTDIR=${DESTDIR:-${PWD}}
PREFIX=${PREFIX:-/opt/kata}
container_image="kata-kernel-builder"

sudo docker build -t "${container_image}" "${script_dir}"

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	"${container_image}" \
	bash -c "${kernel_builder} $* setup"

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	"${container_image}" \
	bash -c "${kernel_builder} $* build"

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env DESTDIR="${DESTDIR}" --env PREFIX="${PREFIX}" \
	"${container_image}" \
	bash -c "${kernel_builder} $* install"
