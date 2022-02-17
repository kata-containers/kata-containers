#!/bin/bash

set -o errexit -o pipefail -o nounset

cd "$(dirname "${BASH_SOURCE[0]}")/.."

protoc --gogottrpc_out=protocols/hypervisor \
	--gogottrpc_opt=plugins=ttrpc+fieldpath,paths=source_relative \
	-Iprotocols/hypervisor \
	-I../agent/protocols/protos/gogo/protobuf \
	protocols/hypervisor/hypervisor.proto
