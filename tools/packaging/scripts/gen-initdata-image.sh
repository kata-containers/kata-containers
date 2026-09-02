#!/usr/bin/env bash
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# gen-initdata-image.sh - Pack an initdata document into an initdata disk image.
#
# A node can hand Kata its init data either as a plain TOML document or as a
# packed disk image, via `initdata_path` in the hypervisor configuration. This
# script produces the packed form, which the runtime attaches to the guest
# exactly as it sits on disk rather than repacking it for every sandbox.
#
# The image layout is:
#
#   offset 0    the 8-byte magic "initdata"
#   offset 8    the gzipped payload length, 8 bytes, little-endian
#   offset 16   the gzipped initdata document
#   ...         zero padding up to a 512-byte sector boundary
#
# The document itself is what gets digested and bound into the TEE launch
# measurement, so the compression here has no bearing on attestation: any
# gzip stream carrying the same document produces the same digest.
#
# This script is deliberately standalone (it sources nothing) so it can be
# copied to a build machine on its own.
#

set -o errexit
set -o nounset
set -o pipefail

readonly MAGIC="initdata"
readonly HEADER_LEN=16
readonly SECTOR_SIZE=512

script_name="${0##*/}"
workdir=""

cleanup() {
	# Keep the last status zero: an EXIT trap that ends on a failed command
	# overrides the status the script was exiting with.
	[[ -z "${workdir}" ]] || rm -rf "${workdir}"
}
trap cleanup EXIT

# Set ${workdir} to a scratch directory, created on first use so that -d and -h
# leave nothing behind. Not a command substitution: that would run in a subshell
# and the assignment would be lost.
make_workdir() {
	[[ -n "${workdir}" ]] || workdir="$(mktemp -d)"
}

die() {
	echo "${script_name}: ERROR: $*" >&2
	exit 1
}

info() {
	echo "${script_name}: $*"
}

usage() {
	cat <<-EOF
	Pack an initdata document into an initdata disk image.

	Usage:
	  ${script_name} [-o OUTPUT] DOCUMENT
	  ${script_name} -d IMAGE
	  ${script_name} -h

	Options:
	  -o OUTPUT  Write the image to OUTPUT. Defaults to DOCUMENT with its
	             extension replaced by '.img'.
	  -d IMAGE   Dump the document packed in IMAGE to stdout, so an image can
	             be inspected or compared against its source.
	  -h         Show this help.

	DOCUMENT may be '-' to read the document from stdin.

	Examples:
	  ${script_name} -o /opt/kata/share/kata-containers/initdata.img initdata.toml
	  ${script_name} -d initdata.img | diff -u initdata.toml -
	EOF
}

# Emit a 64-bit little-endian integer as raw bytes.
emit_le64() {
	local value="$1"
	local i byte

	for ((i = 0; i < 8; i++)); do
		byte=$(( (value >> (8 * i)) & 0xff ))
		# Two levels of printf: the inner one renders the byte as an octal
		# escape, and '%b' makes the outer one turn that escape back into the
		# byte itself.
		printf '%b' "\\0$(printf '%03o' "${byte}")"
	done
}

# Reject anything that is obviously not an initdata document.
#
# This is a light check only -- the runtime parses and validates the document
# for real when it loads its configuration. It exists to catch the common
# mistake of passing one of the files carried *inside* init data (an aa.toml,
# say) in place of the init data document itself.
validate_document() {
	local document="$1"

	grep -Eq '^[[:space:]]*version[[:space:]]*=' "${document}" ||
		die "${document} declares no 'version': this is not an initdata document"
	grep -Eq '^[[:space:]]*algorithm[[:space:]]*=' "${document}" ||
		die "${document} declares no 'algorithm': this is not an initdata document"
}

pack() {
	local document="$1"
	local output="$2"
	local payload size total outdir

	validate_document "${document}"

	outdir="${output%/*}"
	[[ "${outdir}" == "${output}" ]] && outdir="."
	[[ -d "${outdir}" ]] || die "cannot write ${output}: no such directory as ${outdir}"
	[[ -w "${outdir}" ]] || die "cannot write ${output}: ${outdir} is not writable"

	make_workdir
	payload="${workdir}/payload.gz"

	# -n leaves the source name and timestamp out of the gzip header, so the
	# same document always packs to the same bytes.
	gzip -n -9 -c < "${document}" > "${payload}"

	size="$(wc -c < "${payload}")"

	{
		printf '%s' "${MAGIC}"
		emit_le64 "${size}"
		cat "${payload}"
	} > "${output}"

	# Pad to a whole number of sectors. truncate extends with zeros.
	total=$(( ((HEADER_LEN + size + SECTOR_SIZE - 1) / SECTOR_SIZE) * SECTOR_SIZE ))
	truncate -s "${total}" "${output}"

	info "wrote ${output} (${total} bytes, ${size} bytes of compressed document)"
}

dump() {
	local image="$1"
	local size

	# Compare with cmp rather than reading the bytes into a variable: the head
	# of an arbitrary file may hold NUL bytes, which a shell cannot carry.
	printf '%s' "${MAGIC}" | cmp -s -n "${#MAGIC}" - "${image}" ||
		die "${image} does not start with the initdata magic: not an initdata image"

	size="$(od --address-radix=n --format=u8 --endian=little \
		--skip-bytes="${#MAGIC}" --read-bytes=8 "${image}" | tr -d '[:space:]')"

	tail -c "+$((HEADER_LEN + 1))" "${image}" | head -c "${size}" | gzip -d
}

main() {
	local output="" dump_image="" document=""

	while getopts ':o:d:h' opt; do
		case "${opt}" in
			o) output="${OPTARG}" ;;
			d) dump_image="${OPTARG}" ;;
			h) usage; exit 0 ;;
			:) die "option -${OPTARG} requires an argument" ;;
			*) usage >&2; die "unknown option -${OPTARG}" ;;
		esac
	done
	shift $((OPTIND - 1))

	if [[ -n "${dump_image}" ]]; then
		[[ $# -eq 0 ]] || die "-d takes no further arguments"
		[[ -z "${output}" ]] || die "-d and -o are mutually exclusive"
		[[ -r "${dump_image}" ]] || die "cannot read ${dump_image}"
		dump "${dump_image}"
		return
	fi

	[[ $# -eq 1 ]] || { usage >&2; die "expected exactly one document"; }
	document="$1"

	if [[ "${document}" == "-" ]]; then
		local stdin_document
		[[ -n "${output}" ]] || die "-o is required when reading from stdin"
		make_workdir
		stdin_document="${workdir}/document.toml"
		cat > "${stdin_document}"
		pack "${stdin_document}" "${output}"
		return
	fi

	[[ -r "${document}" ]] || die "cannot read ${document}"
	[[ -n "${output}" ]] || output="${document%.*}.img"

	pack "${document}" "${output}"
}

main "$@"
