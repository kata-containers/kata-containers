#!/usr/bin/env bash
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# Create Docker config for genpolicy so it can authenticate to nvcr.io when
# pulling image manifests (avoids "UnauthorizedError" from genpolicy's registry pull).
# Genpolicy uses docker_credential::get_credential() in registry.rs build_auth();
# the crate reads $DOCKER_CONFIG/config.json (see docker_credential crate).
#
# Arguments:
#   $1  Parent directory; config is written to ${1}/.docker-genpolicy/config.json
#
# Exports: DOCKER_CONFIG, REGISTRY_AUTH_FILE (latter is not used by genpolicy's
# docker_credential path; some other tools honor REGISTRY_AUTH_FILE).
#
setup_genpolicy_registry_auth() {
	if [[ -z "${NGC_API_KEY:-}" ]]; then
		return 0
	fi
	local -r parent_dir="${1:?setup_genpolicy_registry_auth: parent directory required}"
	local auth_dir
	auth_dir="${parent_dir}/.docker-genpolicy"
	mkdir -p "${auth_dir}"
	# Docker config format: auths -> registry -> auth (base64 of "user:password")
	echo -n "{\"auths\":{\"nvcr.io\":{\"username\":\"\$oauthtoken\",\"password\":\"${NGC_API_KEY}\",\"auth\":\"$(echo -n "\$oauthtoken:${NGC_API_KEY}" | base64 -w0)\"}}}" \
		> "${auth_dir}/config.json"
	export DOCKER_CONFIG="${auth_dir}"
	export REGISTRY_AUTH_FILE="${auth_dir}/config.json"
}
