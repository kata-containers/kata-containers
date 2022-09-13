#!/bin/bash
# (C) Copyright IBM Corp. 2022.
# SPDX-License-Identifier: Apache-2.0

set -o errexit -o pipefail -o nounset

cd "$(dirname "${BASH_SOURCE[0]}")/.."

protoc --gogottrpc_out=protocols/hypervisor \
	--gogottrpc_opt=plugins=ttrpc+fieldpath,paths=source_relative \
	-Iprotocols/hypervisor \
	-I../libs/protocols/protos/gogo/protobuf \
	protocols/hypervisor/hypervisor.proto
