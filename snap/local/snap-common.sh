#!/usr/bin/env bash
#
# Copyright (c) 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Description: Idempotent script to be sourced by all parts in a
#   snapcraft config file.

set -o errexit
set -o nounset
set -o pipefail

# XXX: Bash-specific code. zsh doesn't support this option and that *does*
# matter if this script is run sourced... since it'll be using zsh! ;)
[ -n "$BASH_VERSION" ] && set -o errtrace

[ -n "${DEBUG:-}" ] && set -o xtrace

die()
{
	echo >&2 "ERROR: $0: $*"
}

[ -n "${SNAPCRAFT_STAGE:-}" ] ||\
	die "must be sourced from a snapcraft config file"

snap_yq_version=3.4.1

snap_common_install_yq()
{
      export yq="${SNAPCRAFT_STAGE}/bin/yq"

      local yq_pkg
      yq_pkg="github.com/mikefarah/yq"

      local yq_url
      yq_url="https://${yq_pkg}/releases/download/${snap_yq_version}/yq_${goos}_${goarch}"
      curl -o "${yq}" -L "${yq_url}"
      chmod +x "${yq}"
}

# Function that should be called for each snap "part" in
# snapcraft.yaml.
snap_common_main()
{
	# Architecture
	arch="$(uname -m)"

	case "${arch}" in
		aarch64)
			goarch="arm64"
			qemu_arch="${arch}"
			;;

		ppc64le)
			goarch="ppc64le"
			qemu_arch="ppc64"
			;;

		s390x)
			goarch="${arch}"
			qemu_arch="${arch}"
			;;

		x86_64)
			goarch="amd64"
			qemu_arch="${arch}"
			;;

		*) die "unsupported architecture: ${arch}" ;;
	esac

	dpkg_arch=$(dpkg --print-architecture)

	# golang
	#
	# We need the O/S name in golang format, but since we don't
	# know if the godeps part has run, we don't know if golang is
	# available yet, hence fall back to a standard system command.
	goos="$(go env GOOS &>/dev/null || true)"
	[ -z "$goos" ] && goos=$(uname -s|tr '[A-Z]' '[a-z]')

	export GOROOT="${SNAPCRAFT_STAGE}"
	export GOPATH="${GOROOT}/gopath"
	export GO111MODULE="auto"

	mkdir -p "${GOPATH}/bin"
	export PATH="${GOPATH}/bin:${PATH}"

	# Proxy
	export http_proxy="${http_proxy:-}"
	export https_proxy="${https_proxy:-}"

	# Binaries
	mkdir -p "${SNAPCRAFT_STAGE}/bin"

	export PATH="$PATH:${SNAPCRAFT_STAGE}/bin"

	# YAML query tool
	export yq="${SNAPCRAFT_STAGE}/bin/yq"

	# Kata paths
	export kata_dir=$(printf "%s/src/github.com/%s/%s" \
		"${GOPATH}" \
		"${SNAPCRAFT_PROJECT_NAME}" \
		"${SNAPCRAFT_PROJECT_NAME}")

	export versions_file="${kata_dir}/versions.yaml"

	[ -n "${yq:-}" ] && [ -x "${yq:-}" ] || snap_common_install_yq
}

snap_common_main
