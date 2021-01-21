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
	cat "${script_dir}/../../../VERSION"
}

gen_version_file() {
	local branch="$1"
	local kata_version="$2"
	local ref="refs/heads/${branch}"

	if  [ "${kata_version}" == "HEAD" ]; then
		kata_version="${branch}"
		ref="refs/heads/${branch}"
	else
		ref="refs/tags/${kata_version}^{}"
	fi

	qemu_vanilla_branch=$(get_from_kata_deps "assets.hypervisor.qemu.version" "${kata_version}")
	# Check if qemu.version can be used to get the version and hash, otherwise use qemu.tag
	qemu_vanilla_ref="refs/heads/${qemu_vanilla_branch}"
	if ! (git ls-remote --heads "https://github.com/qemu/qemu.git" | grep -q "refs/heads/${qemu_vanilla_branch}"); then
		qemu_vanilla_branch=$(get_from_kata_deps "assets.hypervisor.qemu.tag" "${kata_version}")
		qemu_vanilla_ref="refs/tags/${qemu_vanilla_branch}^{}"
	fi
	qemu_vanilla_version=$(curl -s -L "https://raw.githubusercontent.com/qemu/qemu/${qemu_vanilla_branch}/VERSION")
	qemu_vanilla_hash=$(git ls-remote https://github.com/qemu/qemu.git | grep "${qemu_vanilla_ref}" | awk '{print $1}')

	kernel_version=$(get_from_kata_deps "assets.kernel.version" "${kata_version}")
	#Remove extra 'v'
	kernel_version=${kernel_version#v}

	golang_version=$(get_from_kata_deps "languages.golang.meta.newest-version" "${kata_version}")

	# - is not a valid char for rpmbuild
	# see https://github.com/semver/semver/issues/145
	kata_version=$(get_kata_version)
	kata_version=${kata_version/-/\~}
	cat > "$versions_txt" <<EOT
# This is a generated file from ${script_name}

kata_version=${kata_version}

# Dependencies
kata_osbuilder_version=${kata_version}

qemu_vanilla_version=${qemu_vanilla_version}
qemu_vanilla_hash=${qemu_vanilla_hash}

kernel_version=${kernel_version}

# Golang
go_version=${golang_version}
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
--head            Use <kata-branch>'s head to generate the versions file.
EOT
	exit "${exit_code}"
}

main() {
	local compareOnly=
	local use_head=
	local use_tag=

	case "${1:-}" in
		"-h"|"--help")
			usage
			;;
		--compare)
			compareOnly=1
			shift
			;;
		--head)
			use_head=1
			shift
			;;
		--tag)
			use_tag=1
			shift
			;;
		-*)
			die "Invalid option: ${1:-}" "1"
			shift
			;;
	esac

	local kata_version=
	if [ -n "$use_tag" ]; then
		if [ -n "${use_head}" ]; then
		       die "tag and head options are mutually exclusive"
		fi

		# We are generating versions based on the provided tag
		local tag="${1:-}"
		[ -n "${tag}" ] || die "No tag specified" "1"

		# use the runtime's repository to determine branch information
		local repo="github.com/kata-containers/kata-containers"
		local repo_dir="kata-containers"
		git clone --quiet "https://${repo}.git" "${repo_dir}"
		pushd "${repo_dir}" >> /dev/null
		local branch=$(git branch -r -q --contains "${tag}" | grep -E "master|stable|main" | grep -v HEAD)

		popd >> /dev/null
		rm -rf ${repo_dir}

		[ -n "${branch}" ] || die "branch for tag ${tag} not found"

		# in the event this is on master as well as stable, or multiple stables, just pick the first branch
		# (ie, 1.8.0-alpha0 may live on stable-1.8 as well as master: we'd just use master in this case)
		branch=$(echo ${branch} | awk -F" " '{print $1}')

		# format will be origin/<branch-name> - let's drop origin:
		branch=$(echo ${branch} | awk -F"/" '{print $2}')

		echo "generating versions for tag ${tag} which is on branch ${branch}"
		kata_version=${tag}
	else
		local branch="${1:-}"
		[ -n "${branch}" ] || die "No branch specified" "1"

		if [ -n "${use_head}" ]; then
			kata_version="HEAD"
		else
			kata_version=$(get_kata_version)
		fi
	fi

	if [ -n "$compareOnly" ]; then
		source "./${versions_txt}" || exit 1
		kata_version=${kata_version/\~/-}
		[ -n "${kata_version}" ] || die "${version_file} does not contain a valid kata_version variable"
		# Replacing ~ with -, as - is not a valid char for rpmbuild
		# see https://github.com/semver/semver/issues/145
		[ "$(get_kata_version)" = "${kata_version/\~/-}" ] && compare_result="matches" || compare_result="is different from"
		echo "${kata_version} in ${versions_txt} ${compare_result} the version at branch ${branch}"
		return
	fi

	gen_version_file "${branch}" "${kata_version}"
}

main $@
