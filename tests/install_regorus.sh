#!/usr/bin/env bash
#
# Copyright (c) Edgeless Systems GmbH
#
# SPDX-License-Identifier: Apache-2.0

[[ -n "${DEBUG}" ]] && set -o xtrace

test_dir=$(realpath "$(dirname "${BASH_SOURCE[0]}")")
source "${test_dir}/common.bash"

install_regorus_oras()
{
    local version
    version=$1

    if ! command -v oras &>/dev/null; then
        warn "oras is not installed. Please install oras to install regorus."
        return 1
    fi

    local image
    image="${ARTEFACT_REGISTRY:-ghcr.io}/${ARTEFACT_REPOSITORY:-kata-containers/kata-containers}/cached-artefacts/regorus:${version}"

    if ! oras pull "${image}" --no-tty; then
        warn "Failed to pull regorus from oras cache"
        return 1
    fi
    info "Successfully pulled regorus from oras cache"

    if ! mv regorus "${HOME}/.cargo/bin/regorus"; then
        warn "Failed to move regorus binary"
        return 1
    fi

    if ! chmod +x "${HOME}/.cargo/bin/regorus"; then
        warn "Failed to make regorus binary executable"
        return 1
    fi
}

install_regorus_cargo()
{
    local version
    version=$1

    if ! cargo install regorus --version "${version}" --example regorus --locked; then
        warn "Failed to cargo install regorus"
        return 1
    fi
    info "Successfully installed regorus using cargo"

    # Cache the installed binary using oras, so we don't have to build it again.
    if [[ -z "${ARTEFACT_REGISTRY_PASSWORD}" ]]; then
        warn "ARTEFACT_REGISTRY_PASSWORD is not set. Skipping caching of regorus binary."
        return 0
    fi

    if [[ -z "${ARTEFACT_REGISTRY_USERNAME}" ]]; then
        warn "ARTEFACT_REGISTRY_USERNAME is not set. Skipping caching of regorus binary."
        return 0
    fi

    if ! echo "${ARTEFACT_REGISTRY_PASSWORD}" | oras login "${ARTEFACT_REGISTRY:-ghcr.io}" -u "${ARTEFACT_REGISTRY_USERNAME}" --password-stdin; then
        warn "Failed to login to oras registry"
        return 1
    fi

    local image
    image="${ARTEFACT_REGISTRY:-ghcr.io}/${ARTEFACT_REPOSITORY:-kata-containers/kata-containers}/cached-artefacts/regorus:${version}"

    if ! (cd "${HOME}/.cargo/bin/" && oras push "${image}" --no-tty regorus); then
        warn "Failed to push regorus binary to oras cache"
        return 1
    fi
    info "Successfully pushed regorus binary to oras cache as ${image}"
}

install_regorus()
{
    command -v cargo &>/dev/null \
        || die "cargo is not installed. Please install rust toolchain to install regorus."
    command -v git &>/dev/null \
        || die "git is not installed. Please install git."

    # Get the regorus version from Cargo.toml of the agent policy crate instad of versions.yaml
    # so we test the version we are actually using.
    local cargo_toml="${test_dir}/../src/agent/policy/Cargo.toml"
    [[ -f "${cargo_toml}" ]] \
        || die "Cargo.toml not found at ${cargo_toml}"
    local version
    version=$(
        cargo tree -i regorus --edges normal --prefix none --manifest-path "${cargo_toml}" |
            head -n1 |
            cut -d' ' -f2 |
            sed 's/v//'
    ) || die "Failed to get regorus version from cargo.toml"

    if regorus --version 2>/dev/null | grep -q "${version}"; then
        info "regorus version ${version} is already installed"
        return 0
    fi
    info "Installing regorus version ${version}"

    if install_regorus_oras "${version}"; then
        :
    elif install_regorus_cargo "${version}"; then
        :
    else
        die "Failed to install regorus"
    fi

    if ! echo "${PATH}" | grep -q "${HOME}/.cargo/bin"; then
        export PATH="${PATH}:${HOME}/.cargo/bin"
    fi

    info "Successfully installed regorus version ${version}"
}

install_regorus
