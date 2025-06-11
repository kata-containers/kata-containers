#!/usr/bin/env bash
#
# Copyright (c) Edgeless Systems GmbH
#
# SPDX-License-Identifier: Apache-2.0

[[ -n "${DEBUG}" ]] && set -o xtrace

test_dir=$(realpath "$(dirname "${BASH_SOURCE[0]}")")
source "${test_dir}/common.bash"

install_regorus()
{
    command -v cargo &>/dev/null \
        || die "cargo is not installed. Please install rust toolchain to install regorus."
    command -v git &>/dev/null \
        || die "git is not installed. Please install git."

    if regorus --version 2>/dev/null | grep -q "${version}"; then
        info "regorus version ${version} is already installed"
        return 0
    fi

    # Get the regorus version from Cargo.toml of the agent policy crate instad of versions.yaml
    # so we test the version we are actually using.
    local cargo_toml="${test_dir}/../src/agent/policy/Cargo.toml"
    [[ -f "${cargo_toml}" ]] \
        || die "Cargo.toml not found at ${cargo_toml}"

    version=$(
        cargo tree -i regorus --edges normal --prefix none --manifest-path "${cargo_toml}" |
            head -n1 |
            cut -d' ' -f2 |
            sed 's/v//'
    ) || die "Failed to get regorus version from cargo.toml"

    info "Installing regorus version ${version}"

    cargo install regorus --version "${version}" --example regorus \
        || die "Failed to cargo install regorus"

    if ! echo "$PATH" | grep -q "${HOME}/.cargo/bin"; then
        export PATH="${PATH}:${HOME}/.cargo/bin"
    fi

    info "Successfully installed regorus version ${version}"
}

install_regorus
