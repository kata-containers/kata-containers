#!/bin/bash
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

[ -z "${DEBUG}" ] || set -x
set -e
set -o errexit
set -o nounset
set -o pipefail

readonly script_name="$(basename "${BASH_SOURCE[0]}")"
readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly versions_txt="versions.txt"
project="kata-containers"

source "${script_dir}/../scripts/lib.sh"

ARCH=${ARCH:-$(arch_to_golang "$(uname -m)")}

get_kata_version() {
	local branch="$1"
	curl -SsL "https://raw.githubusercontent.com/${project}/runtime/${branch}/VERSION"
}

gen_version_file() {
	local branch="$1"
	[ -n "${branch}" ] || exit 1

	local kata_version=$(get_kata_version "${branch}")
	kata_runtime_hash=$(get_kata_hash_from_tag "runtime" "${kata_version}")
	kata_proxy_hash=$(get_kata_hash_from_tag "proxy" "${kata_version}")
	kata_shim_hash=$(get_kata_hash_from_tag "shim" "${kata_version}")
	kata_agent_hash=$(get_kata_hash_from_tag "agent" "${kata_version}")
	kata_ksm_throttler_hash=$(get_kata_hash_from_tag "ksm-throttler" "${kata_version}")

	qemu_lite_branch=$(get_from_kata_deps "assets.hypervisor.qemu-lite.branch" "${kata_version}")
	qemu_lite_version=$(curl -s -L "https://raw.githubusercontent.com/${project}/qemu/${qemu_lite_branch}/VERSION")
	qemu_lite_hash=$(git ls-remote https://github.com/${project}/qemu.git | grep "refs/heads/${qemu_lite_branch}" | awk '{print $1}')

	qemu_vanilla_branch=$(get_from_kata_deps "assets.hypervisor.qemu.version" "${kata_version}")
	qemu_vanilla_version=$(curl -s -L "https://raw.githubusercontent.com/qemu/qemu/${qemu_vanilla_branch}/VERSION")
	qemu_vanilla_hash=$(git ls-remote https://github.com/qemu/qemu.git | grep "refs/heads/${qemu_vanilla_branch}" | awk '{print $1}')

	kernel_version=$(get_from_kata_deps "assets.kernel.version" "${kata_version}")
	#Remove extra 'v'
	kernel_version=${kernel_version#v}

	golang_version=$(get_from_kata_deps "languages.golang.meta.newest-version" "${kata_version}")
	golang_sha256=$(curl -s -L "https://storage.googleapis.com/golang/go${golang_version}.linux-${ARCH}.tar.gz.sha256")

	# - is not a valid char for rpmbuild
	# see https://github.com/semver/semver/issues/145
	kata_version=${kata_version/-/\~}
	cat > "$versions_txt" <<EOT
# This is a generated file from ${script_name}

kata_version=${kata_version}

kata_runtime_version=${kata_version}
kata_runtime_hash=${kata_runtime_hash}

kata_proxy_version=${kata_version}
kata_proxy_hash=${kata_proxy_hash}

kata_shim_version=${kata_version}
kata_shim_hash=${kata_shim_hash}

kata_agent_version=${kata_version}
kata_agent_hash=${kata_agent_hash}

kata_ksm_throttler_version=${kata_version}
kata_ksm_throttler_hash=${kata_ksm_throttler_hash}

# Dependencies
kata_osbuilder_version=${kata_version}

qemu_lite_version=${qemu_lite_version}
qemu_lite_hash=${qemu_lite_hash}

qemu_vanilla_version=${qemu_vanilla_version}
qemu_vanilla_hash=${qemu_vanilla_hash}

kernel_version=${kernel_version}

# Golang
go_version=${golang_version}
go_checksum=${golang_sha256}
EOT
}

die() {
	local msg="${1:-}"
	local print_usage=$"${2:-}"
	if [ -n "${msg}" ]; then
		echo -e "ERROR: ${msg}\n"
	fi

	[ -n "${print_usage}" ] && usage 1
}

usage() {
	exit_code=$"${1:-0}"
	cat <<EOT
Usage:
${script_name} [--compare | -h | --help] <kata-branch>

Generate a ${versions_txt} file, containing  version numbers and commit hashes
of all the kata components under the git branch <kata-branch>.

Options:

-h, --help        Print this help.
--compare         Only compare the kata version at branch <kata-branch> with the
                  one in ${versions_txt} and leave the file untouched.
EOT
	exit "${exit_code}"
}

main() {
	local compareOnly=

	case "${1:-}" in
		"-h"|"--help")
			usage
			;;
		--compare)
			compareOnly=1
			shift
			;;
		-*)
			die "Invalid option: ${1:-}" "1"
			shift
			;;
	esac
	local branch="${1:-}"
	[ -n "${branch}" ] || die "No branch specified" "1"

	if [ -n "$compareOnly" ]; then
		source "./${versions_txt}" || exit 1
		kata_version=${kata_version/\~/-}
		[ -n "${kata_version}" ] || die "${version_file} does not contain a valid kata_version variable"
		# Replacing ~ with -, as - is not a valid char for rpmbuild
		# see https://github.com/semver/semver/issues/145
		[ "$(get_kata_version $branch)" = "${kata_version/\~/-}" ] && compare_result="matches" || compare_result="is different from"
		echo "${kata_version} in ${versions_txt} ${compare_result} the version at branch ${branch}"
		return
	fi
	gen_version_file "${branch}"
}

main $@
