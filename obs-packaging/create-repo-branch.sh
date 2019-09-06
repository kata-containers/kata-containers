#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
[ -z "${DEBUG}" ] || set -o xtrace

set -o errexit
set -o nounset
set -o pipefail

script_dir=$(cd $(dirname "${BASH_SOURCE[0]}") && pwd)
script_name="$(basename "${BASH_SOURCE[0]}")"

# shellcheck source=./../scripts/lib.sh
source "${script_dir}/../scripts/lib.sh"

# shellcheck source=scripts/obs-docker.sh
source "${script_dir}/scripts/obs-docker.sh"

readonly home_project="home:katacontainers"
readonly template_pkg="kata-pkg-template"
arch_target=${ARCH:-$(uname -m)}

# shellcheck source=scripts/obs-docker.sh
source "${script_dir}/scripts/obs-pkgs.sh"

pkg_exist() {
	local project="$1"
	local pkg="$2"

	docker_run osc list "${project}" | grep "${pkg}" || return 1
	return 0

}

# Array of repositories.
#
# Each element is comprised of multiple parts in the form:
#
#   name::project::repository
#
typeset -a repos
read_repos(){
	while read -r p; do
		[[ "$p" != "#"* ]] || continue
		repos+=("${p}")
		echo "Adding distro: ${p}"
	done < "${script_dir}/distros_${arch_target}"
}

# Array of maintainers
#
# Each element is comprised of multiple parts in the form:
#
#   userid::role
#
typeset -a maintainers

read_maintainers(){
	while read -r p; do
		[[ "$p" != "#"* ]] || continue
		maintainers+=("${p}::maintainer")
		echo "Adding maintainer: ${p}"
	done < "${script_dir}/maintainers"
}

create_repos_xml_nodes() {
	for entry in "${repos[@]}"; do
		[ -z "$entry" ] && die "found empty entry"

		local name
		local project
		local repositories
		name=$(echo "$entry" | awk -F"::" '{print $1;}')
		project=$(echo "$entry" | awk -F"::" '{print $2;}')
		repositories=$(echo "$entry" | awk -F"::" '{print $3;}')

		[ -z "$name" ] && die "no name for entry '$entry'"
		[ -z "$project" ] && die "no project for entry '$entry'"
		[ -z "$repositories" ] && die "no repository for entry '$entry'"

		echo "  <repository name=\"${name}\">"

		echo "${repositories}"| tr ',' '\n' | while read -r repository; do
			echo "    <path project=\"${project}\" repository=\"${repository}\"/>"
		done

		arch_target_obs=${arch_target}
		if [ "$arch_target" == "ppc64" ]; then
			arch_target_obs="ppc64le"
		fi
		echo "    <arch>${arch_target_obs}</arch>"
		echo "  </repository>"
	done
}

create_maintainers_xml_nodes() {
	for entry in "${maintainers[@]}"; do
		[ -z "$entry" ] && die "found empty entry"
		local userid=$(echo "$entry" | awk -F"::" '{print $1;}')
		local role=$(echo "$entry" | awk -F"::" '{print $2;}')
		[ -z "$userid" ] && die "no userid for entry '$entry'"
		[ -z "$role" ] && die "no role for entry '$entry'"
		echo "  <person userid=\"${userid}\" role=\"${role}\"/>"
	done
}

create_meta_xml() {
	project="${1:-}"
	branch="${2:-}"
	[ -n "${project}" ] || die "project is empty"
	[ -n "${branch}" ] || die "branch is empty"

	read_maintainers
	read_repos
	cat >meta_project.xml <<EOT
<project name="${project}">
  <title>Branch project for Kata Containers branch ${branch}</title>
  <description>This project is the Kata Containers branch ${branch}</description>
$(create_maintainers_xml_nodes)
$(create_repos_xml_nodes)
</project>
EOT
}

usage() {
	msg="${1:-}"
	exit_code=$"${2:-0}"
	cat <<EOT
${msg}
Usage:
${script_name} <kata-branch>
EOT
	exit "${exit_code}"
}

main() {
	case "${1:-}" in
		"-h"|"--help")
			usage Help
			;;
		--ci)
			create_ci_subproject=true
			shift
			;;
		-*)
			die "Invalid option: ${1:-}"
			;;
	esac
	local branch="${1:-}"
	[ -n "${branch}" ] || usage "missing branch" "1"
	if [ "${create_ci_subproject:-false}" == "true" ];then
		release_type="ci"
	elif [ "$arch_target" == "ppc64le" ]; then
		release_type="alpha"
	else
		release_type="releases"
	fi

	project_branch="${home_project}:${release_type}:${arch_target}:${branch}"
	create_meta_xml "${project_branch}" "${branch}"
	info "Creating/Updating project with name ${project_branch}"
	# Update /Create project metadata.
	docker_run osc meta prj "${project_branch}" -F meta_project.xml
	for pkg in "${OBS_PKGS_PROJECTS[@]}"; do
		if ! pkg_exist "${project_branch}" "${pkg}"; then
			echo "Package ${pkg} does not exit in ${project_branch}, creating ..."
			docker_run osc branch "${home_project}" "${template_pkg}" "${project_branch}" "${pkg}"
		fi
		pkg_dir="${project_branch}/${pkg}"
		[ -d "${pkg_dir}/.osc" ] || docker_run osc co "${pkg_dir}"
	done
}

main $@
