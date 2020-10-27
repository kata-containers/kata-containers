#!/bin/bash
#
# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

[ -n "${DEBUG:-}" ] && set -o xtrace

readonly script_name=${0##*/}

readonly kata_project="Kata Containers"
readonly containerd_project="containerd"

readonly kata_slug="kata-containers/kata-containers"
readonly containerd_slug="containerd/containerd"

readonly kata_project_url="https://github.com/${kata_slug}"
readonly containerd_project_url="https://github.com/${containerd_slug}"

readonly kata_releases_url="https://api.github.com/repos/${kata_slug}/releases"
readonly containerd_releases_url="https://api.github.com/repos/${containerd_slug}/releases"

# Directory created when unpacking a binary release archive downloaded from
# $kata_releases_url.
readonly kata_install_dir="${kata_install_dir:-/opt/kata}"

# containerd shim v2 details
readonly kata_runtime_name="kata"
readonly kata_runtime_type="io.containerd.${kata_runtime_name}.v2"
readonly kata_shim_v2="containerd-shim-${kata_runtime_name}-v2"

# Systemd unit name for containerd daemon
readonly containerd_service_name="containerd.service"

# Directory in which to create symbolic links
readonly link_dir=${link_dir:-/usr/bin}

readonly tmpdir=$(mktemp -d)

readonly warnings=$(cat <<EOT
WARNINGS:

- Use distro-packages where possible

  If your distribution packages $kata_project, you should use these packages rather
  than running this script.

- Packages will **not** be automatically updated

  Since a package manager is not being used, it is **your** responsibility
  to ensure these packages are kept up-to-date when new versions are released
  to ensure you are using a version that includes the latest security and bug fixes.

- Potentially untested versions or version combinations

  This script installs the *newest* versions of $kata_project
  and $containerd_project from binary release packages. These versions may
  not have been tested with your distribution version.

EOT
)

die()
{
	echo -e >&2 "ERROR: $*"
	exit 1
}

info()
{
	echo -e "INFO: $*"
}

cleanup()
{
	[ -d "$tmpdir" ] && rm -rf "$tmpdir"
}

# Determine the latest GitHub release for a project.
#
# Parameter: GitHub API URL for a projects releases.
# Note: Releases are assumed to use semver (https://semver.org) version
#   numbers.
github_get_latest_release()
{
	local url="${1:-}"

	# Notes:
	#
	# - The sort(1) call; none of the standard utilities support semver
	#   so attempt to perform a semver sort manually.
	# - Pre-releases are excluded via the select() call.
	local latest=$(curl -sL "$url" |\
		jq -r '.[].tag_name | select(contains("-") | not)' |\
		sort -t "." -k1,1n -k2,2n -k3,3n |\
		tail -1 || true)

	[ -z "$latest" ] && die "Cannot determine latest release from $url"

	echo "$latest"
}

# Returns the actual version to download based on the specified one; if the
# specified version is blank, return the latest version.
github_resolve_version_to_download()
{
	local url="${1:-}"
	local requested_version="${2:-}"

	local version=""

	if [ -n "$requested_version" ]
	then
		version="$requested_version"
	else
		version=$(github_get_latest_release "$url" || true)
	fi

	echo "$version"
}

# Return the GitHub URL for a particular release's pre-built binary package.
#
# Parameters:
#
# - GitHub API URL for a project's releases.
# - Release version to look for.
github_get_release_file_url()
{
	local url="${1:-}"
	local version="${2:-}"

	download_url=$(curl -sL "$url" |\
		jq --arg version "$version" \
		-r '.[] | select(.tag_name == $version) | .assets[0].browser_download_url' || true)

	[ "$download_url" = null ] && download_url=""
	[ -z "$download_url" ] && die "Cannot determine download URL for version $version ($url)"

	local arch=$(uname -m)

	[ "$arch" = x86_64 ] && arch="($arch|amd64)"
	echo "$download_url" | egrep -q "$arch" || die "No release for '$arch architecture ($url)"

	echo "$download_url"
}

# Download the release file for the specified projects release version and
# return the full path to the downloaded file.
#
# Parameters:
#
# - GitHub API URL for a project's releases.
# - Release version to download.
github_download_release()
{
	local url="${1:-}"
	local version="${2:-}"

	pushd "$tmpdir" >/dev/null

	local download_url=$(github_get_release_file_url \
		"$url" \
		"$version" || true)

	[ -z "$download_url" ] && \
		die "Cannot determine download URL for version $version"

	# Don't specify quiet mode here so the user can observe download
	# progress.
	curl -LO "$download_url"

	local filename=$(echo "$download_url" | awk -F'/' '{print $NF}')

	ls -d "${PWD}/${filename}"

	popd >/dev/null
}

usage()
{
	cat <<EOT
Usage: $script_name [options] [<kata-version> [<containerd-version>]]

Description: Install $kata_project [1] and $containerd_project [2] from GitHub release binaries.

Options:

 -h : Show this help statement.

Notes:

- The version strings must refer to official GitHub releases for each project.
  If not specified or set to "", install the latest available version.

See also:

[1] - $kata_project_url
[2] - $containerd_project_url

$warnings

Advice:

- You can check the latest version of Kata Containers by running:

  $ kata-runtime check --only-list-releases

EOT
}

# Determine if the system only supports cgroups v2.
#
# - Writes "true" to stdout if only cgroups v2 are supported.
# - Writes "false" to stdout if cgroups v1 or v1+v2 are available.
# - Writes a blank string to stdout if cgroups are not available.
only_supports_cgroups_v2()
{
	local v1=$(mount|awk '$5 ~ /^cgroup$/ { print; }' || true)
	local v2=$(mount|awk '$5 ~ /^cgroup2$/ { print; }' || true)

	[ -n "$v1" ] && [ -n "$v2" ] && { echo "false"; return 0; } || true
	[ -n "$v1" ] && { echo "false"; return 0; } || true
	[ -n "$v2" ] && { echo "true"; return 0; } || true

	return 0
}

pre_checks()
{
	info "Running pre-checks"

	command -v "${kata_shim_v2}" &>/dev/null \
		&& die "Please remove existing $kata_project installation"

	command -v containerd &>/dev/null \
		&& die "$containerd_project already installed"

	systemctl list-unit-files --type service |\
		egrep -q "^${containerd_service_name}\>" \
		&& die "$containerd_project already installed"

	local cgroups_v2_only=$(only_supports_cgroups_v2 || true)

	local url="https://github.com/kata-containers/kata-containers/issues/927"

	[ "$cgroups_v2_only" = "true" ] && \
		die "$kata_project does not yet fully support cgroups v2 - see $url"

	return 0
}

check_deps()
{
	info "Checking dependencies"

	# Maps command names to package names using a colon delimiter
	local elems=()

	elems+=("curl:curl")
	elems+=("git:git")
	elems+=("jq:jq")
	elems+=("tar:tar")

	local pkgs_to_install=()

	local elem

	for elem in "${elems[@]}"
	do
		local cmd=$(echo "$elem"|cut -d: -f1)
		local pkg=$(echo "$elem"|cut -d: -f2-)

		command -v "$cmd" &>/dev/null && continue

		pkgs_to_install+=("$pkg")
	done

	[ "${#pkgs_to_install[@]}" -eq 0 ] && return 0

	local packages="${pkgs_to_install[@]}"

	info "Installing packages '$packages'"

	case "$ID" in
		centos|rhel) sudo yum -y install $packages ;;
		debian|ubuntu) sudo apt-get -y install $packages ;;
		fedora) sudo dnf -y install $packages ;;
		opensuse*|sles) sudo zypper install -y $packages ;;
		*) die "Unsupported distro: $ID"
	esac
}

setup()
{
	trap cleanup EXIT
	source /etc/os-release || source /usr/lib/os-release

	pre_checks
	check_deps
}

# Download the requested version of the specified project.
#
# Returns the resolve version number and the full path to the downloaded file
# separated by a colon.
github_download_package()
{
	local releases_url="${1:-}"
	local requested_version="${2:-}"
	local project="${3:-}"

	[ -z "$releases_url" ] && die "need releases URL"
	[ -z "$project" ] && die "need project URL"

	local version=$(github_resolve_version_to_download \
		"$releases_url" \
		"$version" || true)

	[ -z "$version" ] && die "Unable to determine $project version to download"

	local file=$(github_download_release \
		"$releases_url" \
		"$version")

	echo "${version}:${file}"
}

install_containerd()
{
	local requested_version="${1:-}"

	local project="$containerd_project"

	local version_desc="latest version"
	[ -n "$requested_version" ] && version_desc="version $requested_version"

	info "Downloading $project release ($version_desc)"

	local results=$(github_download_package \
		"$containerd_releases_url" \
		"$requested_version" \
		"$project")

	[ -z "$results" ] && die "Cannot download $project release file"

	local version=$(echo "$results"|cut -d: -f1)
	local file=$(echo "$results"|cut -d: -f2-)

	[ -z "$version" ] && die "Cannot determine $project resolved version"
	[ -z "$file" ] && die "Cannot determine $project release file"

	info "Installing $project release $version from $file"

	sudo tar -C /usr/local -xvf "${file}"

	sudo ln -s /usr/local/bin/ctr "${link_dir}"

	info "$project installed\n"
}

configure_containerd()
{
	local project="$containerd_project"

	info "Configuring $project"

	local cfg="/etc/containerd/config.toml"

	pushd "$tmpdir" >/dev/null

	local service_url=$(printf "%s/%s/%s/%s" \
		"https://raw.githubusercontent.com" \
		"${containerd_slug}" \
		"master" \
		"${containerd_service_name}")

	curl -LO "$service_url"

	printf "# %s: Service installed for Kata Containers\n" \
		"$(date -Iseconds)" |\
		tee -a "$containerd_service_name"

	local systemd_unit_dir="/etc/systemd/system"
	sudo mkdir -p "$systemd_unit_dir"

	local dest="${systemd_unit_dir}/${containerd_service_name}"

	sudo cp "${containerd_service_name}" "${dest}"
	sudo systemctl daemon-reload

	info "Installed ${dest}"

	popd >/dev/null

	# Backup the original containerd configuration:
	sudo mkdir -p "$(dirname $cfg)"

	sudo test -e "$cfg" || {
		sudo touch "$cfg"
		info "Created $cfg"
	}

	local original="${cfg}-pre-kata-$(date -I)"

	sudo grep -q "$kata_runtime_type" "$cfg" || {
		sudo cp "$cfg" "${original}"
		info "Backed up $cfg to $original"
	}

	# Add the Kata Containers configuration details:

	sudo grep -q "$kata_runtime_type" "$cfg" || {
		cat <<-EOT | sudo tee -a "$cfg"
		[plugins]
		    [plugins.cri]
		        [plugins.cri.containerd]
		        default_runtime_name = "${kata_runtime_name}"
		    [plugins.cri.containerd.runtimes.${kata_runtime_name}]
		        runtime_type = "${kata_runtime_type}"
EOT

		info "Modified $cfg"
	}

	sudo systemctl start containerd

	info "Configured $project\n"
}

install_kata()
{
	local requested_version="${1:-}"

	local project="$kata_project"

	local version_desc="latest version"
	[ -n "$requested_version" ] && version_desc="version $requested_version"

	info "Downloading $project release ($version_desc)"

	local results=$(github_download_package \
		"$kata_releases_url" \
		"$requested_version" \
		"$project")

	[ -z "$results" ] && die "Cannot download $project release file"

	local version=$(echo "$results"|cut -d: -f1)
	local file=$(echo "$results"|cut -d: -f2-)

	[ -z "$version" ] && die "Cannot determine $project resolved version"
	[ -z "$file" ] && die "Cannot determine $project release file"

	# Allow the containerd service to find the Kata shim and users to find
	# important commands:
	local create_links_for=()

	create_links_for+=("$kata_shim_v2")
	create_links_for+=("kata-collect-data.sh")
	create_links_for+=("kata-runtime")

	local from_dir=$(printf "%s/bin" "$kata_install_dir")

	# Since we're unpacking to the root directory, perform a sanity check
	# on the archive first.
	local unexpected=$(tar -tf "${file}" |\
		egrep -v "^(\./opt/$|\.${kata_install_dir}/)" || true)

	[ -n "$unexpected" ] && die "File '$file' contains unexpected paths: '$unexpected'"

	info "Installing $project release $version from $file"

	sudo tar -C / -xvf "${file}"

	[ -d "$from_dir" ] || die "$project does not exist in expected directory $from_dir"

	for file in "${create_links_for[@]}"
	do
		local from_path=$(printf "%s/%s" "$from_dir" "$file")
		[ -e "$from_path" ] || die "File $from_path not found"

		sudo ln -sf "$from_path" "$link_dir"
	done

	info "$project installed\n"
}

handle_kata()
{
	local version="${1:-}"

	install_kata "$version"

	kata-runtime --version
}

handle_containerd()
{
	local version="${1:-}"

	install_containerd "$version"

	configure_containerd

	containerd --version
}

test_installation()
{
	info "Testing $kata_project\n"

	local image="docker.io/library/busybox:latest"
	sudo ctr image pull "$image"

	local container_name="test-kata"

	# Used to prove that the kernel in the container
	# is different to the host kernel.
	local container_kernel=$(sudo ctr run \
		--runtime "$kata_runtime_type" \
		--rm \
		"$image" \
		"$container_name" \
		uname -r || true)

	[ -z "$container_kernel" ] && die "Failed to test $kata_project"

	local host_kernel=$(uname -r)

	info "Test successful:\n"

	info "  Host kernel version      : $host_kernel"
	info "  Container kernel version : $container_kernel"
	echo
}

handle_installation()
{
	local kata_version="${1:-}"
	local containerd_version="${2:-}"

	setup

	handle_kata "$kata_version"
	handle_containerd "$containerd_version"

	test_installation

	info "$kata_project and $containerd_project are now installed"

	echo -e "\n${warnings}\n"
}

handle_args()
{
	case "${1:-}" in
		-h|--help|help) usage; exit 0;;
	esac

	local kata_version="${1:-}"
	local containerd_version="${2:-}"

	handle_installation \
		"$kata_version" \
		"$containerd_version"
}

main()
{
	handle_args "$@"
}

main "$@"
