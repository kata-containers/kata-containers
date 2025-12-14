#!/bin/bash
#
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# This script handles downloading tarballs with ORAS caching.
# It will:
# 1. Check if the tarball version exists in GHCR
# 2. If yes, pull it from GHCR
# 3. If no, download from upstream and optionally push to GHCR
#

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Source lib.sh for common functions (die, info, get_from_kata_deps, arch_to_golang, etc.)
if [[ -f "${script_dir}/lib.sh" ]]; then
	source "${script_dir}/lib.sh"
elif [[ -f "${script_dir}/../../tools/packaging/scripts/lib.sh" ]]; then
	# When sourced from ci/install_libseccomp.sh in the repo
	source "${script_dir}/../../tools/packaging/scripts/lib.sh"
fi

# Path to existing install_oras.sh script (only used when running outside Docker)
install_oras_script="${script_dir}/../kata-deploy/local-build/dockerbuild/install_oras.sh"

# ORAS configuration
ARTEFACT_REGISTRY="${ARTEFACT_REGISTRY:-ghcr.io}"
# Default to upstream kata-containers org to match cached-artefacts pattern
# Result: ghcr.io/kata-containers/cached-tarballs/<component>:<version>
ARTEFACT_REPOSITORY="${ARTEFACT_REPOSITORY:-kata-containers}"
# Reuse PUSH_TO_REGISTRY to control cache pushing
PUSH_TO_REGISTRY="${PUSH_TO_REGISTRY:-no}"

# Credentials for pushing (optional)
ARTEFACT_REGISTRY_USERNAME="${ARTEFACT_REGISTRY_USERNAME:-}"
ARTEFACT_REGISTRY_PASSWORD="${ARTEFACT_REGISTRY_PASSWORD:-}"

#
# Install ORAS using the existing install script
#
ensure_oras_installed() {
	if command -v oras &>/dev/null; then
		info "ORAS is already available"
		return 0
	fi

	if [[ -f "${install_oras_script}" ]]; then
		info "Installing ORAS using existing script"
		if "${install_oras_script}"; then
			# Verify installation succeeded
			if command -v oras &>/dev/null; then
				info "ORAS installed successfully"
				return 0
			else
				warn "ORAS installation completed but command not found in PATH"
				return 1
			fi
		else
			warn "ORAS installation script failed"
			return 1
		fi
	else
		warn "ORAS install script not found at ${install_oras_script}"
		return 1
	fi
}

#
# Login to registry if credentials are provided
#
oras_login() {
	if [[ -n "${ARTEFACT_REGISTRY_USERNAME}" ]] && [[ -n "${ARTEFACT_REGISTRY_PASSWORD}" ]]; then
		echo "${ARTEFACT_REGISTRY_PASSWORD}" | oras login "${ARTEFACT_REGISTRY}" \
			-u "${ARTEFACT_REGISTRY_USERNAME}" --password-stdin
		return 0
	fi
	return 1
}

#
# Logout from registry
#
oras_logout() {
	oras logout "${ARTEFACT_REGISTRY}" 2>/dev/null || true
}

#
# Check if artifact exists in GHCR and pull it
# Returns 0 if successful, 1 if not found
#
pull_from_cache() {
	local artifact_name="$1"
	local version="$2"
	local output_dir="$3"

	local oci_image="${ARTEFACT_REGISTRY}/${ARTEFACT_REPOSITORY}/cached-tarballs/${artifact_name}:${version}"

	info "Checking cache for ${artifact_name} version ${version}"

	mkdir -p "${output_dir}"
	pushd "${output_dir}" > /dev/null

	# Redirect ORAS output to stderr so it doesn't get captured by variable assignments
	if oras pull "${oci_image}" --no-tty >&2; then
		info "Successfully pulled ${artifact_name} from cache"
		popd > /dev/null
		return 0
	fi

	popd > /dev/null
	warn "Failed to pull from cache: ${oci_image}"
	return 1
}

#
# Push artifact to GHCR cache
#
push_to_cache() {
	local artifact_name="$1"
	local version="$2"
	local tarball_path="$3"

	if [[ "${PUSH_TO_REGISTRY}" != "yes" ]]; then
		info "PUSH_TO_REGISTRY is not set to 'yes', skipping cache push"
		return 0
	fi

	if ! oras_login; then
		warn "Cannot push to cache: no credentials provided"
		return 1
	fi

	local oci_image="${ARTEFACT_REGISTRY}/${ARTEFACT_REPOSITORY}/cached-tarballs/${artifact_name}:${version}"

	# Check if this version already exists in cache (avoid race conditions with parallel builds)
	if oras manifest fetch "${oci_image}" &>/dev/null; then
		info "Version ${version} of ${artifact_name} already exists in cache, skipping push"
		oras_logout
		return 0
	fi

	local tarball_name
	tarball_name=$(basename "${tarball_path}")
	local tarball_dir
	tarball_dir=$(dirname "${tarball_path}")

	info "Pushing ${tarball_name} to cache as ${oci_image}"

	pushd "${tarball_dir}" > /dev/null

	# Collect files to push: tarball + any verification files (sha256, sig, gpg-keyring)
	local files_to_push=("${tarball_name}")
	if [[ -f "${tarball_name}.sha256" ]]; then
		files_to_push+=("${tarball_name}.sha256")
		info "Including SHA256 checksum file in cache"
	fi
	if [[ -f "${tarball_name}.sig" ]]; then
		files_to_push+=("${tarball_name}.sig")
		info "Including GPG signature file in cache"
	fi
	if [[ -f "${tarball_name}.gpg-keyring" ]]; then
		files_to_push+=("${tarball_name}.gpg-keyring")
		info "Including GPG public key in cache"
	fi

	# Push tarball and verification files (redirect to stderr to avoid stdout pollution)
	oras push "${oci_image}" "${files_to_push[@]}" --no-tty >&2

	popd > /dev/null

	oras_logout

	info "Successfully pushed ${artifact_name} version ${version} to cache"
	return 0
}

#
# Download tarball from upstream URL with verification files
# Verification files are kept for caching
#
download_upstream() {
	local url="$1"
	local output_path="$2"
	local checksum_url="${3:-}"
	local gpg_sig_url="${4:-}"

	local tarball_name
	tarball_name=$(basename "${output_path}")
	local output_dir
	output_dir=$(dirname "${output_path}")

	info "Downloading from upstream: ${url}"
	curl -sSL -o "${output_path}" "${url}"

	# Download and verify using SHA256 checksum if available
	if [[ -n "${checksum_url}" ]]; then
		local checksum_file="${output_dir}/${tarball_name}.sha256"
		if curl -sSL -o "${checksum_file}" "${checksum_url}" 2>/dev/null; then
			info "Verifying SHA256 checksum..."
			pushd "${output_dir}" > /dev/null
			sha256sum -c "${tarball_name}.sha256" >&2
			popd > /dev/null
			info "SHA256 checksum verified"
			# Keep the checksum file for caching
		else
			warn "Could not download checksum file from ${checksum_url}"
		fi
	fi

	# Download and verify using GPG signature if available
	if [[ -n "${gpg_sig_url}" ]]; then
		local sig_file="${output_dir}/${tarball_name}.sig"
		if curl -sSL -o "${sig_file}" "${gpg_sig_url}" 2>/dev/null; then
			info "Verifying GPG signature..."
			# Import GPG key from keyserver
			gpg --keyserver hkps://keyserver.ubuntu.com --recv-keys C9E9416F76E610DBD09D040F47B70C55ACC9965B >&2 2>/dev/null || true
			pushd "${output_dir}" > /dev/null
			if gpg --verify "${tarball_name}.sig" "${tarball_name}" >&2 2>/dev/null; then
				info "GPG signature verified"
				# Export the GPG key to cache alongside the signature for offline verification
				gpg --export C9E9416F76E610DBD09D040F47B70C55ACC9965B > "${tarball_name}.gpg-keyring" 2>/dev/null || true
				info "Exported GPG public key for caching"
			else
				warn "GPG signature verification failed"
			fi
			popd > /dev/null
			# Keep the sig file for caching
		else
			warn "Could not download GPG signature from ${gpg_sig_url}"
		fi
	fi

	info "Downloaded: ${output_path}"
}

#
# Main function: download with cache
# Usage: download_with_cache <artifact_name> <version> <upstream_url> <output_dir> [checksum_url] [gpg_sig_url]
#
# Verification files (SHA256 or GPG sig) are stored in cache alongside the tarball
# Returns the path to the downloaded tarball via stdout (last line)
#
download_with_cache() {
	local artifact_name="$1"
	local version="$2"
	local upstream_url="$3"
	local output_dir="$4"
	local checksum_url="${5:-}"
	local gpg_sig_url="${6:-}"

	local tarball_name
	tarball_name=$(basename "${upstream_url}")
	local tarball_path="${output_dir}/${tarball_name}"

	# Try to use ORAS cache if available
	if ensure_oras_installed; then
		# Try to pull from cache first
		if pull_from_cache "${artifact_name}" "${version}" "${output_dir}"; then
			# Verify the file exists
			if [[ -f "${tarball_path}" ]]; then
				pushd "${output_dir}" > /dev/null
				# Verify using upstream verification file from cache (no internet access)
				if [[ -f "${tarball_name}.sha256" ]]; then
					# SHA256 verification (busybox style) - works offline
					if sha256sum -c "${tarball_name}.sha256" >&2; then
						info "SHA256 checksum verified for cached ${artifact_name}"
						popd > /dev/null
						echo "${tarball_path}"
						return 0
					else
						warn "SHA256 verification failed for cached ${artifact_name}, downloading from upstream"
						popd > /dev/null
					fi
				elif [[ -f "${tarball_name}.sig" ]]; then
					# GPG signature file exists - import cached key if available
					if [[ -f "${tarball_name}.gpg-keyring" ]]; then
						# Import GPG key from cached keyring (no internet needed)
						gpg --import "${tarball_name}.gpg-keyring" >&2 2>/dev/null || true
						info "Imported GPG key from cache"
					fi
					# Verify if key is now available
					if gpg --list-keys C9E9416F76E610DBD09D040F47B70C55ACC9965B &>/dev/null; then
						if gpg --verify "${tarball_name}.sig" "${tarball_name}" >&2 2>/dev/null; then
							info "GPG signature verified for cached ${artifact_name}"
							popd > /dev/null
							echo "${tarball_path}"
							return 0
						else
							warn "GPG verification failed for cached ${artifact_name}, downloading from upstream"
							popd > /dev/null
						fi
					else
						# Key not available (no cached keyring and not imported locally)
						warn "GPG key not available, cannot verify ${artifact_name}"
						popd > /dev/null
						# Fall through to download from upstream
					fi
				else
					# No verification file, trust the cache
					info "No verification file in cache for ${artifact_name}, trusting cache integrity"
					popd > /dev/null
					echo "${tarball_path}"
					return 0
				fi
			fi
		fi
		
		# Cache miss or verification failed - download from upstream
		info "Downloading ${artifact_name} from upstream..."
		download_upstream "${upstream_url}" "${tarball_path}" "${checksum_url}" "${gpg_sig_url}"
		
		# Push to cache for future use (include verification files)
		push_to_cache "${artifact_name}" "${version}" "${tarball_path}"
	else
		info "ORAS not available, downloading directly from upstream"
		download_upstream "${upstream_url}" "${tarball_path}" "${checksum_url}" "${gpg_sig_url}"
	fi

	echo "${tarball_path}"
	return 0
}

#
# Generic function to download a component from versions.yaml
# Arguments:
#   $1: component name (e.g., "gperf", "busybox")
#   $2: output directory (default: current directory)
# Environment variables (optional, to avoid needing yq):
#   ${COMPONENT}_VERSION - Override version (e.g., BUSYBOX_VERSION, GPERF_VERSION)
#   ${COMPONENT}_URL     - Override base URL (e.g., BUSYBOX_URL, GPERF_URL)
# Returns: path to the downloaded tarball
#
download_component() {
	local component="${1}"
	local output_dir="${2:-.}"

	if [[ -z "${component}" ]]; then
		die "Component name is required"
	fi

	# Convert component name to uppercase for environment variable lookup
	local component_upper
	component_upper=$(echo "${component}" | tr '[:lower:]' '[:upper:]')
	
	# Get version and URL from environment variables or versions.yaml
	local version_var="${component_upper}_VERSION"
	local url_var="${component_upper}_URL"
	local version="${!version_var:-}"
	local base_url="${!url_var:-}"
	
	# Fall back to versions.yaml if environment variables not set
	if [[ -z "${version}" ]]; then
		if command -v yq &>/dev/null; then
			version=$(get_from_kata_deps ".externals.${component}.version")
		else
			die "Component version not provided and yq not available. Set ${component_upper}_VERSION environment variable."
		fi
	fi
	
	if [[ -z "${base_url}" ]]; then
		if command -v yq &>/dev/null; then
			base_url=$(get_from_kata_deps ".externals.${component}.url")
		else
			die "Component URL not provided and yq not available. Set ${component_upper}_URL environment variable."
		fi
	fi

	if [[ -z "${version}" ]] || [[ -z "${base_url}" ]]; then
		die "Component '${component}' not found in versions.yaml and environment variables not set"
	fi

	# Component-specific configuration
	# Each component has different verification: gperf uses GPG sig, busybox uses SHA256
	local tarball_url checksum_url gpg_sig_url
	case "${component}" in
		gperf)
			tarball_url="${base_url}/gperf-${version}.tar.gz"
			checksum_url=""  # gperf doesn't provide SHA256
			gpg_sig_url="${base_url}/gperf-${version}.tar.gz.sig"
			;;
		busybox)
			tarball_url="${base_url}/busybox-${version}.tar.bz2"
			checksum_url="${base_url}/busybox-${version}.tar.bz2.sha256"
			gpg_sig_url=""  # busybox provides SHA256, we'll use that
			;;
		*)
			die "Unknown component: ${component}"
			;;
	esac

	download_with_cache "${component}" "${version}" "${tarball_url}" "${output_dir}" "${checksum_url}" "${gpg_sig_url}"
}

#
# Convenience function for gperf (backward compatibility)
#
download_gperf() {
	download_component "gperf" "${1:-.}"
}

#
# Convenience function for busybox (backward compatibility)
#
download_busybox() {
	download_component "busybox" "${1:-.}"
}

# If script is executed directly (not sourced), run the main function
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
	case "${1:-}" in
		gperf|busybox)
			component="$1"
			shift
			download_component "${component}" "$@"
			;;
		*)
			echo "Usage: $0 <component> [output_dir]"
			echo ""
			echo "Arguments:"
			echo "  component   - Component name from versions.yaml (e.g., gperf, busybox)"
			echo "  output_dir  - Directory to download tarball to (default: current directory)"
			echo ""
			echo "Environment variables:"
			echo "  ARTEFACT_REGISTRY          - Registry to use (default: ghcr.io)"
			echo "  ARTEFACT_REPOSITORY        - Repository org/path (default: kata-containers)"
			echo "  PUSH_TO_REGISTRY           - Set to 'yes' to push new artifacts to cache"
			echo "  ARTEFACT_REGISTRY_USERNAME - Username for registry (required for push)"
			echo "  ARTEFACT_REGISTRY_PASSWORD - Password for registry (required for push)"
			echo ""
			echo "Supported components: gperf, busybox"
			echo ""
			echo "Example:"
			echo "  $0 gperf /tmp"
			exit 1
			;;
	esac
fi
