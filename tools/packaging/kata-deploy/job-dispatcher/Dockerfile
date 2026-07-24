# Copyright (c) 2026 Kata Contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Minimal image for the job-mode dispatcher (kata-deploy-job-dispatcher).
#
# Unlike the kata-deploy image, this dispatcher never touches the host: it only
# talks to the Kubernetes API (lists nodes, creates/watches per-node Jobs). It
# therefore needs nothing but the dispatcher binary and CA certificates.
#
# The binary is produced by the shared rust-builder stage and packaged into
# kata-deploy-static-kata-deploy-job-dispatcher.tar.zst (see Dockerfile.components and
# local-build/kata-deploy-build-components-tarballs.sh). Build from the repo
# root so the tarball path resolves:
#   docker build -f tools/packaging/kata-deploy/job-dispatcher/Dockerfile .
#
# Linking note: the binary is only *statically* linked on amd64/arm64 (musl).
# On ppc64le/s390x the rust-builder falls back to the glibc target
# (…-unknown-linux-gnu), so the binary is dynamically linked and needs the glibc
# loader + shared libraries at runtime. distroless/static ships neither, so - as
# the main kata-deploy image already does for its own binary - we assemble the
# exact runtime dependencies with ldd and copy them into the final image on
# those architectures. amd64/arm64 stay fully static and copy nothing.

#### Extract the dispatcher binary from its tarball
FROM alpine:3.22 AS extract-stage

ARG KATA_ARTIFACTS_DIR=tools/packaging/kata-deploy/kata-artifacts

SHELL ["/bin/ash", "-eo", "pipefail", "-c"]

RUN apk add --no-cache zstd

COPY ${KATA_ARTIFACTS_DIR}/kata-deploy-static-kata-deploy-job-dispatcher.tar.zst /tmp/dispatcher.tar.zst

RUN \
	mkdir -p /opt/dispatcher && \
	zstd -dc /tmp/dispatcher.tar.zst | tar -xf - -C /opt/dispatcher ./usr/bin/kata-deploy-job-dispatcher

#### Assemble runtime dependencies (glibc loader + shared libraries)
# On ppc64le/s390x the dispatcher is a dynamically-linked glibc binary; collect
# exactly what it needs with ldd. On amd64/arm64 it is musl-static, so this is a
# no-op that leaves /output/lib and /output/lib64 empty.
FROM debian:trixie-slim AS runtime-assembler

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

# Copy the dispatcher binary so we can inspect its dependencies.
COPY --from=extract-stage /opt/dispatcher/usr/bin/kata-deploy-job-dispatcher /tmp/kata-deploy-job-dispatcher

RUN mkdir -p /output/lib /output/lib64

RUN \
	HOST_ARCH="$(uname -m)"; \
	echo "Preparing dispatcher runtime dependencies for ${HOST_ARCH}"; \
	case "${HOST_ARCH}" in \
		"ppc64le"|"s390x") \
			echo "Using glibc - copying libraries based on ldd output"; \
			echo "Libraries needed by kata-deploy-job-dispatcher:"; \
			ldd /tmp/kata-deploy-job-dispatcher || echo "ldd failed"; \
			\
			# Copy the directly-referenced (=>) shared libraries. \
			ldd /tmp/kata-deploy-job-dispatcher 2>/dev/null | grep "=>" | awk '{print $3}' | sort -u | while read -r lib; do \
				if [ -n "${lib}" ] && [ -f "${lib}" ]; then \
					dest_dir="/output$(dirname "${lib}")"; \
					mkdir -p "${dest_dir}"; \
					cp -Ln "${lib}" "${dest_dir}/" 2>/dev/null || true; \
					echo "  Copied lib: ${lib}"; \
				fi; \
			done; \
			\
			# Copy the dynamic linker (not reported with "=>" by ldd). \
			echo "Copying dynamic linker:"; \
			cp -Ln /lib/ld64.so* /output/lib/ 2>/dev/null || true; \
			cp -Ln /lib64/ld64.so* /output/lib64/ 2>/dev/null || true; \
			\
			# glibc loads the NSS/resolver backends via dlopen, so ldd never lists \
			# them. The dispatcher resolves the apiserver endpoint at startup; in \
			# cluster that is usually an IP (no lookup), but it may be a hostname. \
			# These modules live under /lib/<triplet>, which the final stage already \
			# copies, so add them there (distroless ships /etc/nsswitch.conf and \
			# glibc has sane built-in defaults, so no /etc changes are needed). \
			echo "Copying NSS/resolver libraries:"; \
			for libdir in /lib/*-linux-gnu /lib64; do \
				[ -d "${libdir}" ] || continue; \
				for lib in "${libdir}"/libnss_files.so* "${libdir}"/libnss_dns.so* "${libdir}"/libresolv.so*; do \
					[ -f "${lib}" ] || continue; \
					dest_dir="/output$(dirname "${lib}")"; \
					mkdir -p "${dest_dir}"; \
					cp -Ln "${lib}" "${dest_dir}/" 2>/dev/null || true; \
					echo "  Copied lib: ${lib}"; \
				done; \
			done; \
			;; \
		*) \
			echo "amd64/arm64: dispatcher is musl-static, nothing to assemble"; \
			;; \
	esac

#### Dispatcher image
# distroless does not publish pinned/versioned tags - only rolling ones
# (latest, nonroot, debug) - so :latest is the intended way to consume it.
# hadolint ignore=DL3007
FROM gcr.io/distroless/static-debian13:latest

COPY --from=extract-stage /opt/dispatcher/usr/bin/kata-deploy-job-dispatcher /usr/bin/kata-deploy-job-dispatcher

# glibc loader + shared libraries for ppc64le/s390x. On amd64/arm64 these
# directories are empty (musl-static binary), so BuildKit copies nothing and the
# final image is left untouched.
COPY --from=runtime-assembler /output/lib/ /lib/
COPY --from=runtime-assembler /output/lib64/ /lib64/

ENTRYPOINT ["/usr/bin/kata-deploy-job-dispatcher"]
