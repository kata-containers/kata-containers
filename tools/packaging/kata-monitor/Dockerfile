# Copyright (c) 2020 Eric Ernst
# Copyright (c) 2026 NVIDIA
# SPDX-License-Identifier: Apache-2.0
#
# Build context: extracted kata-static shim-v2-go tarball root.
#
# Expected file in context:
#   ./opt/kata/bin/kata-monitor
#
# The kata-monitor binary can be either statically or dynamically linked
# depending on the arch it was built for (see src/runtime/Makefile):
#
#   * amd64, arm64, riscv64: fully static, cgo-free (STATIC=yes default).
#   * s390x, ppc64le:        cgo + PIE, glibc-linked (STATIC=no default).
#
# For the static case there are no runtime dependencies to assemble, and
# distroless/static already provides everything the binary needs. For the
# glibc case we use `ldd` to discover the required shared libraries and
# the dynamic linker, and copy them into the runtime image, matching the
# pattern used by tools/packaging/kata-deploy/Dockerfile.

# Stage 1: discover and copy the glibc libraries the binary needs
# (skipped when the binary is statically linked).
# hadolint ignore=DL3007
FROM debian:trixie-slim AS runtime-assembler

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

COPY opt/kata/bin/kata-monitor /tmp/kata-monitor

RUN \
	set -eux; \
	mkdir -p /output/lib /output/lib64; \
	# `ldd` on a static ELF prints "not a dynamic executable" to stderr and
	# exits 1. Capture both streams into a variable so the detection does
	# not go through a pipeline (with `set -o pipefail` a failing ldd would
	# otherwise mask a matching grep and the check would silently miss the
	# static case).
	ldd_output="$(ldd /tmp/kata-monitor 2>&1 || true)"; \
	if [[ "${ldd_output}" == *"not a dynamic executable"* ]]; then \
		echo "kata-monitor is statically linked on $(uname -m); no glibc libraries or dynamic linker to copy"; \
		exit 0; \
	fi; \
	echo "Libraries needed by kata-monitor on $(uname -m):"; \
	ldd /tmp/kata-monitor || true; \
	# Copy each shared library reported by ldd ("=>" lines).
	ldd /tmp/kata-monitor 2>/dev/null | grep "=>" | awk '{print $3}' | sort -u | \
	while read -r lib; do \
		if [ -n "${lib}" ] && [ -f "${lib}" ]; then \
			dest_dir="/output$(dirname "${lib}")"; \
			mkdir -p "${dest_dir}"; \
			cp -Ln "${lib}" "${dest_dir}/" || true; \
			echo "  Copied lib: ${lib}"; \
		fi; \
	done; \
	# Copy the dynamic linker too: ldd does not include it in the "=>"
	# lines. Cover all glibc target architectures: s390x and ppc64le
	# under the default settings, plus amd64/arm64 for opt-out
	# STATIC=no builds.
	#   x86_64  -> /lib64/ld-linux-x86-64.so.2
	#   aarch64 -> /lib/ld-linux-aarch64.so.1
	#   s390x   -> /lib/ld64.so.1
	#   ppc64le -> /lib64/ld64.so.2
	for ld in /lib*/ld-linux-*.so.* /lib*/ld64.so.*; do \
		[ -f "${ld}" ] || continue; \
		dest_dir="/output$(dirname "${ld}")"; \
		mkdir -p "${dest_dir}"; \
		cp -Ln "${ld}" "${dest_dir}/" || true; \
		echo "  Copied linker: ${ld}"; \
	done

# Stage 2: final distroless image.
#
# We deliberately track the rolling `latest` tag rather than pinning a
# digest. distroless/static-debian13 publishes no semver tags and is
# rebuilt frequently to pick up base-image CVE fixes, so following
# `latest` keeps the kata-monitor runtime on the newest patched base.
# The image only carries the handful of glibc libraries we copy in plus
# the kata-monitor binary, so the blast radius of an unexpected base
# bump is tiny. hadolint's "pin the version" check is therefore not
# something we want here.
# hadolint ignore=DL3007
FROM gcr.io/distroless/static-debian13:latest

COPY --from=runtime-assembler /output/lib/ /lib/
COPY --from=runtime-assembler /output/lib64/ /lib64/
COPY opt/kata/bin/kata-monitor /usr/bin/kata-monitor

EXPOSE 8090

ENTRYPOINT ["/usr/bin/kata-monitor"]
CMD ["--help"]
