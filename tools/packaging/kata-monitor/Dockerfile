# Copyright (c) 2020 Eric Ernst
# Copyright (c) 2026 NVIDIA
# SPDX-License-Identifier: Apache-2.0
#
# Build context: extracted kata-static shim-v2-go tarball root.
#
# Expected file in context:
#   ./opt/kata/bin/kata-monitor
#
# The kata-monitor binary is built inside an Ubuntu (glibc) toolchain
# as part of the shim-v2-go static build, so it is dynamically linked
# against glibc. We assemble its runtime dependencies via `ldd` from
# a glibc base image and copy them into a distroless/static runtime
# image, matching the same pattern used by
# tools/packaging/kata-deploy/Dockerfile.

# Stage 1: discover and copy the glibc libraries the binary needs.
# hadolint ignore=DL3007
ARG IMAGE_REGISTRY=docker.io
FROM ${IMAGE_REGISTRY}/debian:trixie-slim AS runtime-assembler

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

COPY opt/kata/bin/kata-monitor /tmp/kata-monitor

RUN \
	set -eux; \
	mkdir -p /output/lib /output/lib64; \
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
	# lines. Cover all four target architectures:
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
