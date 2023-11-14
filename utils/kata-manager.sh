#!/usr/bin/env bash
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
readonly containerd_io_releases_url="https://raw.githubusercontent.com/containerd/containerd.io/main/content/releases.md"

readonly docker_slug="moby/moby"
readonly docker_project="Docker (moby)"
readonly docker_releases_url="https://api.github.com/repos/${docker_slug}/releases"

# Directory created when unpacking a binary release archive downloaded from
# $kata_releases_url.
readonly kata_install_dir="${kata_install_dir:-/opt/kata}"

# containerd shim v2 details
readonly kata_runtime_name="kata"
readonly kata_runtime_type="io.containerd.${kata_runtime_name}.v2"
readonly kata_shim_v2="containerd-shim-${kata_runtime_name}-v2"
readonly kata_configuration="configuration"

readonly kata_clh_runtime_name="kata-clh"
readonly kata_clh_runtime_type="io.containerd.${kata_clh_runtime_name}.v2"
readonly kata_clh_shim_v2="containerd-shim-${kata_clh_runtime_name}-v2"
readonly kata_clh_configuration="configuration-clh"

# Systemd unit name for containerd daemon
readonly containerd_service_name="containerd.service"

# Containerd configuration file
readonly containerd_config="/etc/containerd/config.toml"

# Directory in which to create symbolic links
readonly link_dir=${link_dir:-/usr/bin}

readonly tmpdir=$(mktemp -d)

readonly warnings=$(cat <<EOF
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

EOF
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
	local latest
	latest=$(curl -sL "$url" |\
		jq -r '.[].tag_name | select(contains("-") | not)' |\
		sort -t '.' -V |\
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

	# The version, less any leading 'v'
	local version_number
	version_number=${version#v}

	# Create an array to store architecture names
	local arches=()
	local arch=$(uname -m)

	arches+=("$arch")

	case "${arch}" in
		x86_64*)
                        arches+=("amd64") ;;
		aarch64*)
                        arches+=("arm64") ;;
		s390x*)
                        arches+=("s390x") ;;
		ppc64le*)
                        arches+=("ppc64le") ;;
		*)
			die "Unsupported arch. Must be x86_64, arm64, s390x, or ppc64le." ;;
	esac

	# Create a regular expression for matching
	local regex=""
	local arch_regex=$(IFS='|'; echo "${arches[*]}")
	arch_regex=$(printf "(%s)" "$arch_regex")

	case "$url" in
		*kata*)
			regex="kata-static-${version}-${arch_regex}.tar.xz" ;;
		*containerd*)
			regex="containerd-${version_number}-linux-${arch_regex}.tar.gz" ;;
		*) die "invalid url: '$url'" ;;
	esac

	local download_url

	download_url=$(curl -sL "$url" |\
		jq --arg version "$version" \
		-r '.[] |
			select( (.tag_name == $version) or (.tag_name == "v" + $version) ) |
			.assets[].browser_download_url' |\
			grep -E "/${regex}$")

	download_url=$(echo "$download_url" | awk '{print $1}')

	[ -z "$download_url" ] && die "Cannot determine download URL for version $version ($url)"

	# Check to ensure there is only a single matching URL
	local expected_count=1

	local count
	count=$(echo "$download_url" | wc -l)

	[ "$count" -eq "$expected_count" ] || \
		die "expected $expected_count download URL but found $download_url"

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

	local download_url
	download_url=$(github_get_release_file_url \
		"$url" \
		"$version" || true)

	[ -z "$download_url" ] && \
		die "Cannot determine download URL for version $version"

	# Don't specify quiet mode here so the user can observe download
	# progress.
	curl -LO "$download_url"

	local filename
	filename=$(echo "$download_url" | awk -F'/' '{print $NF}')

	ls -d "${PWD}/${filename}"

	popd >/dev/null
}

usage()
{
	cat <<EOF
Usage: $script_name [options] [<kata-version> [<containerd-version>]]

Description: Install $kata_project [1] (and optionally $containerd_project [2])
  from GitHub release binaries.

Options:

 -c <flavour> : Specify containerd flavour ("lts" | "active" - default: "lts").
                Find more details on LTS and Active versions of containerd on
                https://containerd.io/releases/#support-horizon
 -d           : Enable debug for all components.
 -D           : Install Docker server and CLI tooling (takes priority over '-c').
 -f           : Force installation (use with care).
 -h           : Show this help statement.
 -k <version> : Specify Kata Containers version.
 -K <tarball> : Specify local Kata Containers tarball to install (takes priority over '-k').
 -l           : List installed and available versions only, then exit (uses network).
 -o           : Only install Kata Containers.
 -r           : Don't cleanup on failure (retain files).
 -t           : Disable self test (don't try to create a container after install).
 -T           : Only run self test (do not install anything).

Notes:

- The version strings must refer to official GitHub releases for each project.
  If not specified or set to "", install the latest available version.

See also:

[1] - $kata_project_url
[2] - $containerd_project_url

$warnings

Advice:

- You can check the latest version of Kata Containers by running
  one of the following:

  $ $script_name -l
  $ kata-runtime check --only-list-releases
  $ kata-ctl check only-list-releases

EOF
}

# Return 0 if containerd is already installed, else return 1.
containerd_installed()
{
	command -v containerd &>/dev/null && return 0

	systemctl list-unit-files --type service |\
		grep -Eq "^${containerd_service_name}\>" \
		&& return 0

	return 1
}

pre_checks()
{
	info "Running pre-checks"

	local skip_containerd="${1:-}"
	[ -z "$skip_containerd" ] && die "no skip_containerd value"

	command -v "${kata_shim_v2}" &>/dev/null \
		&& die "Please remove existing $kata_project installation"

	[ "$skip_containerd" = 'true' ] && return 0

	local ret

	{ containerd_installed; ret=$?; } || true

	[ "$ret" -eq 0 ] && die "$containerd_project already installed"

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
		local cmd
		cmd=$(echo "$elem"|cut -d: -f1)

		local pkg
		pkg=$(echo "$elem"|cut -d: -f2-)

		command -v "$cmd" &>/dev/null && continue

		pkgs_to_install+=("$pkg")
	done

	[ "${#pkgs_to_install[@]}" -eq 0 ] && return 0

	local packages
	packages="${pkgs_to_install[@]}"

	info "Installing packages '$packages'"

	case "$ID" in
		centos|rhel) sudo yum -y install $packages ;;
		debian|ubuntu) sudo apt-get update && sudo apt-get -y install $packages ;;
		fedora) sudo dnf -y install $packages ;;
		opensuse*|sles) sudo zypper install -y $packages ;;
		*) die "Cannot automatically install packages on $ID, install $packages manually and re-run"
	esac
}

setup()
{
	local cleanup="${1:-}"
	[ -z "$cleanup" ] && die "no cleanup value"

	local force="${2:-}"
	[ -z "$force" ] && die "no force value"

	local skip_containerd="${3:-}"
	[ -z "$skip_containerd" ] && die "no skip_containerd value"

	[ "$cleanup" = "true" ] && trap cleanup EXIT

	source /etc/os-release || source /usr/lib/os-release

	#these dependencies are needed inside this script, and should be checked regardless of the -f option.
	check_deps

	[ "$force" = "true" ] && return 0

	pre_checks "$skip_containerd"
}

# Download the requested version of the specified project.
#
# Returns the resolve version number and the full path to the downloaded file
# separated by a colon.
github_download_package()
{
	local releases_url="${1:-}"
	local requested_version="${2:-}"

	# Only used for error message
	local project="${3:-}"

	[ -z "$releases_url" ] && die "need releases URL"
	[ -z "$project" ] && die "need project URL"

	local version
	version=$(github_resolve_version_to_download \
		"$releases_url" \
		"$requested_version" || true)

	[ -z "$version" ] && die "Unable to determine $project version to download"

	local file
	file=$(github_download_release \
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

	local results
	results=$(github_download_package \
		"$containerd_releases_url" \
		"$requested_version" \
		"$project")

	[ -z "$results" ] && die "Cannot download $project release file"

	local version
	version=$(echo "$results"|cut -d: -f1)

	local file
	file=$(echo "$results"|cut -d: -f2-)

	[ -z "$version" ] && die "Cannot determine $project resolved version"
	[ -z "$file" ] && die "Cannot determine $project release file"

	info "Installing $project release $version from $file"

	sudo tar -C /usr/local -xvf "${file}"

	for file in \
		/usr/local/bin/containerd \
		/usr/local/bin/ctr
		do
			sudo ln -sf "$file" "${link_dir}"
		done

	info "$project installed\n"
}

configure_containerd()
{
	local enable_debug="${1:-}"
	[ -z "$enable_debug" ] && die "no enable debug value"

	local project="$containerd_project"

	info "Configuring $project"

	local systemd_unit_dir="/etc/systemd/system"
	sudo mkdir -p "$systemd_unit_dir"

	local dest="${systemd_unit_dir}/${containerd_service_name}"

	if [ ! -f "$dest" ]
	then
		pushd "$tmpdir" >/dev/null

		local service_url
		service_url=$(printf "%s/%s/%s/%s" \
			"https://raw.githubusercontent.com" \
			"${containerd_slug}" \
			"main" \
			"${containerd_service_name}")

		curl -LO "$service_url"

		printf "# %s: Service installed for Kata Containers\n" \
			"$(date -Iseconds)" |\
			tee -a "$containerd_service_name"

		sudo cp "${containerd_service_name}" "${dest}"
		sudo systemctl daemon-reload

		info "Installed ${dest}"

		popd >/dev/null
	fi

	# Backup the original containerd configuration:
	sudo mkdir -p "$(dirname $containerd_config)"

	sudo test -e "$containerd_config" || {
		sudo touch "$containerd_config"
		info "Created $containerd_config"
	}

	local original
	original="${containerd_config}-pre-kata-$(date -I)"

	sudo grep -q "$kata_runtime_type" "$containerd_config" || {
		sudo cp "$containerd_config" "${original}"
		info "Backed up containerd config file '$containerd_config' to '$original'"
	}

	local modified="false"

	# Add the Kata Containers configuration details:

	local comment_text
	comment_text=$(printf "%s: Added by %s\n" \
		"$(date -Iseconds)" \
		"$script_name")

	sudo grep -q "$kata_runtime_type" "$containerd_config" || {
		cat <<-EOF | sudo tee -a "$containerd_config"
		# $comment_text
		[plugins]
		  [plugins."io.containerd.grpc.v1.cri"]
		    [plugins."io.containerd.grpc.v1.cri".containerd]
		      default_runtime_name = "${kata_runtime_name}"
		      [plugins."io.containerd.grpc.v1.cri".containerd.runtimes]
		        [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.${kata_runtime_name}]
		          runtime_type = "${kata_runtime_type}"
			  privileged_without_host_devices = true
			  [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.${kata_runtime_name}.options]
			    ConfigPath = "/opt/kata/share/defaults/kata-containers/${kata_configuration}.toml"
		        [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.${kata_clh_runtime_name}]
		          runtime_type = "${kata_clh_runtime_type}"
			  privileged_without_host_devices = true
			  [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.${kata_clh_runtime_name}.options]
			    ConfigPath = "/opt/kata/share/defaults/kata-containers/${kata_clh_configuration}.toml"
		EOF

		modified="true"
	}

	if [ "$enable_debug" = "true" ]
	then
		local debug_enabled
		debug_enabled=$(awk -v RS='' '/\[debug\]/' "$containerd_config" |\
			grep -E "^\s*\<level\>\s*=\s*.*\<debug\>" || true)

		[ -n "$debug_enabled" ] || {
			cat <<-EOF | sudo tee -a "$containerd_config"
			# $comment_text
			[debug]
				level = "debug"
			EOF
		}

		modified="true"
	fi

	[ "$modified" = "true" ] && info "Modified containerd config file '$containerd_config'"
	sudo systemctl enable containerd
	sudo systemctl start containerd

	local msg="disabled"
	[ "$enable_debug" = "true" ] && msg="enabled"

	info "Configured $project (debug $msg)\n"
}

install_kata()
{
	local requested_version="${1:-}"
	local kata_tarball="${2:-}"

	local project="$kata_project"

	local version=""
	if [ -z "$kata_tarball" ]
	then
		local version_desc="latest version"
		[ -n "$requested_version" ] && version_desc="version $requested_version"
	
		info "Downloading $project release ($version_desc)"
	
		local results
		results=$(github_download_package \
			"$kata_releases_url" \
			"$requested_version" \
			"$project")
	
		[ -z "$results" ] && die "Cannot download $project release file"
	
		version=$(echo "$results"|cut -d: -f1)

		[ -z "$version" ] && die "Cannot determine $project resolved version"

		local file
		file=$(echo "$results"|cut -d: -f2-)
	else
		file="$kata_tarball"
	fi

	[ -z "$file" ] && die "Cannot determine $project release file"

	# Allow the containerd service to find the Kata shim and users to find
	# important commands:
	local create_links_for=()

	create_links_for+=("$kata_shim_v2")
	create_links_for+=("kata-collect-data.sh")
	create_links_for+=("kata-runtime")

	local from_dir
	from_dir=$(printf "%s/bin" "$kata_install_dir")

	# Since we're unpacking to the root directory, perform a sanity check
	# on the archive first.
	local unexpected
	unexpected=$(tar -tf "${file}" |\
		grep -Ev "^(\./$|\./opt/$|\.${kata_install_dir}/)" || true)

	[ -n "$unexpected" ] && die "File '$file' contains unexpected paths: '$unexpected'"

	if [ -n "$kata_tarball" ]
	then
		info "Installing $project release from $file"
	else
		info "Installing $project release $version from $file"
	fi

	sudo tar -C / -xvf "${file}"

	[ -d "$from_dir" ] || die "$project does not exist in expected directory $from_dir"

	for file in "${create_links_for[@]}"
	do
		local from_path
		from_path=$(printf "%s/%s" "$from_dir" "$file")
		[ -e "$from_path" ] || die "File $from_path not found"

		sudo ln -sf "$from_path" "$link_dir"
	done

	info "$project installed\n"
}

configure_kata()
{
	local enable_debug="${1:-}"
	[ -z "$enable_debug" ] && die "no enable debug value"

	[ "$enable_debug" = "false" ] && \
		info "Using default $kata_project configuration" && \
		return 0

	local config_file='configuration.toml'
	local kata_dir='/etc/kata-containers'

	sudo mkdir -p "$kata_dir"

	local cfg_from
	local cfg_to

	cfg_from="${kata_install_dir}/share/defaults/kata-containers/${config_file}"
	cfg_to="${kata_dir}/${config_file}"

	[ -e "$cfg_from" ] || die "cannot find $kata_project configuration file"

	sudo install -o root -g root -m 0644 "$cfg_from" "$cfg_to"

	sudo sed -i \
		-e 's/^# *\(enable_debug\).*=.*$/\1 = true/g' \
		-e 's/^kernel_params = "\(.*\)"/kernel_params = "\1 agent.log=debug"/g' \
		"$cfg_to"

	info "Configured $kata_project for full debug (delete '$cfg_to' to use pristine $kata_project configuration)"
}

handle_kata()
{
	local version="${1:-}"
	local tarball="${2:-}"

	local enable_debug="${3:-}"
	[ -z "$enable_debug" ] && die "no enable debug value"

	install_kata "$version" "$tarball"

	configure_kata "$enable_debug"

	kata-runtime --version
}

containerd_version_number()
{
	local flavour="${1:-}"
	[ -z "$flavour" ] && die "need containerd flavour"

	base_version="$(curl -fsSL $containerd_io_releases_url | \
		grep -iE "\[d+.d+\].*| $flavour" | \
		grep -oE "\[[0-9]+.[0-9]+\]" | \
		grep -oE "[0-9]+.[0-9]+")"

	curl --silent ${containerd_releases_url} | \
		jq -r .[].tag_name | \
		grep "^v${base_version}.[0-9]*$" -m1
}

handle_containerd()
{
	local flavour="${1:-}"
	[ -z "$flavour" ] && die "need containerd flavour"

	local force="${2:-}"
	[ -z "$force" ] && die "need force value"

	local enable_debug="${3:-}"
	[ -z "$enable_debug" ] && die "no enable debug value"

	local version="$(containerd_version_number "$flavour")"
	local ret

	if [ "$force" = "true" ]
	then
		install_containerd "$version"
	else
		{ containerd_installed; ret=$?; } || true

		if [ "$ret" -eq 0 ]
		then
			info "Using existing containerd installation"
		else
			install_containerd "$version"
		fi
	fi

	configure_containerd "$enable_debug"

	containerd --version
}

handle_docker()
{
	{ containerd_installed; ret=$?; } || true
	if [ "$ret" -eq 0 ]
	then
		info "Backing up previous $containerd_project configuration"
		[ -e "$containerd_config" ] && sudo mv $containerd_config $containerd_config.system-$(date -Iseconds)
	fi

	containerd_installed

	local filename='get-docker.sh'

	local file
	file="$tmpdir/$filename"

	curl -fsSL https://get.docker.com -o "$file"
	sudo sh "$file"

	rm -rf "$file"

	sudo systemctl enable --now docker

	configure_containerd "$enable_debug"

	containerd --version
	docker --version
}

test_installation()
{
	local tool="${1:-}"
	[ -z "$tool" ] && die "The tool to test $kata_project with was not specified"

	info "Testing $kata_project\n"

	sudo kata-runtime check -v

	local image="docker.io/library/busybox:latest"
	sudo $tool image pull "$image"

	local container_name="${script_name/./-}-test-kata"

	# Used to prove that the kernel in the container
	# is different to the host kernel.
	cmd="sudo $tool run --runtime "$kata_runtime_type" --rm"
	case "$tool" in
		docker)
			# docker takes the container name as `--name
			# $container_name`, passed to the run option.
		       	cmd+=" --name $container_name" ;;
	esac
	cmd+=" $image"
	case "$tool" in
		ctr)
			# ctr takes the container name as a mandatory
			# argument after the image name
			cmd+=" $container_name" ;;
	esac
	cmd+=" uname -r"

	info "Running \"$cmd\""
	container_kernel=$(eval "$cmd" || true)

	[ -z "$container_kernel" ] && die "Failed to test $kata_project"

	local host_kernel
	host_kernel=$(uname -r)

	info "Test successful:\n"

	info "  Host kernel version      : $host_kernel"
	info "  Container kernel version : $container_kernel"
	echo
}

handle_installation()
{
	local cleanup="${1:-}"
	[ -z "$cleanup" ] && die "no cleanup value"

	local force="${2:-}"
	[ -z "$force" ] && die "no force value"

	local skip_containerd="${3:-}"
	[ -z "$skip_containerd" ] && die "no only Kata value"

	local enable_debug="${4:-}"
	[ -z "$enable_debug" ] && die "no enable debug value"

	local disable_test="${5:-}"
	[ -z "$disable_test" ] && die "no disable test value"

	local only_run_test="${6:-}"
	[ -z "$only_run_test" ] && die "no only run test value"

	# These params can be blank
	local kata_version="${7:-}"
	local containerd_flavour="${8:-}"
	local install_docker="${9:-}"
	[ -z "$install_docker" ] && die "no install docker value"

	local kata_tarball="${10:-}"
	# The tool to be testing the installation with
	local tool="ctr"

	if [ "$install_docker" = "true" ]
	then
		if [ "$skip_containerd" = "false" ]
		then
			# The script provided by docker already takes care
			# of properly installing containerd
			skip_containerd="true"
			info "Containerd will be installed during the Docker installation ('-c' option ignored)"
		fi
		tool="docker"
	fi

	[ "$only_run_test" = "true" ] && test_installation "$tool"  && return 0

	setup "$cleanup" "$force" "$skip_containerd"

	handle_kata "$kata_version" "$kata_tarball" "$enable_debug"

	[ "$skip_containerd" = "false" ] && \
		handle_containerd \
		"$containerd_flavour" \
		"$force" \
		"$enable_debug"

	[ "$install_docker" = "true" ] && handle_docker

	[ "$disable_test" = "false" ] && test_installation "$tool"

	if [ "$skip_containerd" = "true" ] && [ "$install_docker" = "false" ]
	then
		info "$kata_project is now installed"
	else
		local extra_projects="containerd"
		[ "$install_docker" = "true" ] && extra_projects+=" and docker"
		info "$kata_project and $extra_projects are now installed"
	fi

	echo -e "\n${warnings}\n"
}

validate_containerd_flavour()
{
	local flavour="${1:-}"
	local flavours_regex='lts|active'

	grep -qE "$flavours_regex" <<< "$flavour" || die "expected flavour to match '$flavours_regex', found '$flavour'"
}

list_versions()
{
	local -r not_installed='<not installed>'

	# The latest available checks will hit the network so inform the
	# user what we are doing in case of network delays.
	info "Getting version details"

	local installed_kata
	installed_kata=$("$kata_shim_v2" --version 2>/dev/null ||\
		echo "$not_installed")

	local installed_containerd
	installed_containerd=$(containerd --version 2>/dev/null ||\
		echo "$not_installed")

	local installed_docker
	installed_docker=$(docker --version 2>/dev/null ||\
		echo "$not_installed")

	local latest_kata
	latest_kata=$(github_get_latest_release "$kata_releases_url" || true)
	[ -z "$latest_kata" ] && \
		die "cannot determine latest version of $project"

	local latest_containerd
	latest_containerd=$(github_get_latest_release "$containerd_releases_url" || true)
	[ -z "$latest_containerd" ] && \
		die "cannot determine latest version of $containerd_project"

	local latest_docker
	latest_docker=$(github_get_latest_release "$docker_releases_url" || true)
	[ -z "$latest_docker" ] && \
		die "cannot determine latest version of $docker_project"

	info "$kata_project: installed version: $installed_kata"
	info "$kata_project: latest version: $latest_kata"

	echo

	info "$containerd_project: installed version: $installed_containerd"
	info "$containerd_project: latest version: $latest_containerd"

	echo

	info "$docker_project: installed version: $installed_docker"
	info "$docker_project: latest version: $latest_docker"
}

handle_args()
{
	local cleanup="true"
	local force="false"
	local skip_containerd="false"
	local disable_test="false"
	local only_run_test="false"
	local enable_debug="false"
	local install_docker="false"
	local list_versions='false'

	local opt

	local kata_version=""
	local containerd_flavour="lts"
	local kata_tarball=""

	while getopts "c:dDfhk:K:lortT" opt "$@"
	do
		case "$opt" in
			c) containerd_flavour="$OPTARG" ;;
			d) enable_debug="true" ;;
			D) install_docker="true" ;;
			f) force="true" ;;
			h) usage; exit 0 ;;
			k) kata_version="$OPTARG" ;;
			K) kata_tarball="$OPTARG" ;;
			l) list_versions='true' ;;
			o) skip_containerd="true" ;;
			r) cleanup="false" ;;
			t) disable_test="true" ;;
			T) only_run_test="true" ;;

			*) die "invalid option: '$opt'" ;;
		esac
	done

	shift $[$OPTIND-1]

	[ "$list_versions" = 'true' ] && list_versions && exit 0

	[ -z "$kata_version" ] && kata_version="${1:-}" || true
	[ -z "$containerd_flavour" ] && containerd_flavour="${2:-}" || true

	validate_containerd_flavour "$containerd_flavour"

	handle_installation \
		"$cleanup" \
		"$force" \
		"$skip_containerd" \
		"$enable_debug" \
		"$disable_test" \
		"$only_run_test" \
		"$kata_version" \
		"$containerd_flavour" \
		"$install_docker" \
		"$kata_tarball"
}

main()
{
	handle_args "$@"
}

main "$@"
