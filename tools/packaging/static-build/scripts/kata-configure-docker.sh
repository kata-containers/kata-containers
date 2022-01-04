#!/bin/bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# Description: Script to configure Docker for the static
#   version of Kata Containers.

[ -z "${DEBUG}" ] || set -x
set -o errexit
set -o nounset
set -o pipefail

docker_config_dir="/etc/docker"
docker_config_file="${docker_config_file:-${docker_config_dir}/daemon.json}"

# The static version of Kata Containers is entirely contained within
# this directory.
readonly static_base_dir="/opt/kata"

# Path to runtime in static archive file
readonly runtime_path="${static_base_dir}/bin/kata-runtime"

die()
{
	local msg="$*"
	echo >&2 "ERROR: $msg"
	exit 1
}

info()
{
	local msg="$*"
	echo >&2 "INFO: $msg"
}

configure_docker()
{
	local file="$1"
	[ -z "$file" ] && die "need file"

	mkdir -p "${docker_config_dir}"

	if [ -e "$docker_config_file" ]
	then
		local today=$(date '+%Y-%m-%d')
		local backup="${docker_config_file}.${today}"

		info "Backing up original Docker config file '$docker_config_file' to '$backup'"

		sudo cp "${docker_config_file}" "${docker_config_file}.${today}"
	else
		# Create a minimal valid JSON document
		echo "{}" > "${docker_config_file}"
	fi

	local config_files=$(tar tvf "$file" |\
		grep "/configuration-.*\.toml" |\
		grep -v -- '->' |\
		awk '{print $NF}' |\
		sed 's/^\.//g' || true)

	[ -z "$config_files" ] && die "cannot find any configuration files in '$file'"

	local config
	local -a runtimes

	for config in $(echo "$config_files" | tr '\n' ' ')
	do
		local runtime
		runtime=$(echo "$config" |\
			awk -F \/ '{print $NF}' |\
			sed -e 's/configuration/kata/g' -e 's/\.toml//g')

		runtimes+=("$runtime")

		local result
		result=$(cat "$docker_config_file" |\
			jq \
			--arg config "$config" \
			--arg runtime "$runtime" \
			--arg runtime_path "$runtime_path" \
			'.runtimes[$runtime] = {path: $runtime_path, "runtimeArgs": ["--config", $config]}')

		echo "$result" > "$docker_config_file"
	done

	info "Validating $docker_config_file"

	jq -S . "$docker_config_file" &>/dev/null

	info "Restarting Docker to apply new configuration"

	$chronic sudo systemctl restart docker

	info "Docker configured for the following additional runtimes: ${runtimes[@]}"
}

setup()
{
	source "/etc/os-release" || source "/usr/lib/os-release"

	# Used to manipulate $docker_config_file
	local pkg="jq"

	case "$ID" in
		opensuse*) distro="opensuse" ;;
		*)         distro="$ID" ;;
	esac

	# Use chronic(1) if available
	chronic=
	command -v chronic && chronic=chronic

	if command -v "$pkg" &>/dev/null
	then
		return 0
	fi

	info "Cannot find $pkg command so installing package"

	case "$distro" in
		centos|rhel) $chronic sudo -E yum -y install "$pkg" ;;
		debian|ubuntu) $chronic sudo -E apt-get --no-install-recommends install -y "$pkg" ;;
		fedora) $chronic sudo -E dnf -y install "$pkg" ;;
		opensuse|sles) $chronic sudo -E zypper -y install "$pkg" ;;
		*) die "do not know how to install command $pkg' for distro '$distro'" ;;
	esac
}

main()
{
	local file="$1"
	[ -z "$file" ] && die "need full path to Kata Containers static archive file"

	echo "$file" | grep -q "^kata-static-.*\.tar.xz" || die "invalid file: '$file'"

	[ $(id -u) -eq 0 ] || die "must be run as root"

	setup

	configure_docker "$file"
}

main "$@"
