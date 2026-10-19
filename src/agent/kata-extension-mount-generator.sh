#!/bin/bash
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# systemd system generator that instantiates the kata-extension-mount@.service
# template for every guest extension image declared on the kernel command line.
#
# The runtime emits one "kata.extension.<name>.verity_params=..." entry per
# configured guest_extension_images, so this generator enables exactly the
# extensions the VM is configured with -- without the rootfs build needing to
# know any extension name in advance. Adding a new extension therefore requires
# no change to the image build: it is wired up purely from the kernel cmdline.
# When the AppArmor extension is configured, the agent must also require its
# mount to succeed before starting, so policy loading cannot race the mount.
#
# systemd invokes generators with three directory arguments; $1 is the "normal"
# output directory, which is where dependency symlinks belong.

set -eu
# Split cmdline words without expanding malformed names as filesystem globs.
set -f

normal_dir="${1:-/tmp}"
unit="/usr/lib/systemd/system/kata-extension-mount@.service"
target="kata-containers.target"
wants_dir="${normal_dir}/${target}.wants"
agent_dropin_dir="${normal_dir}/kata-agent.service.d"

# Tests can supply a cmdline fixture without accessing the running system.
cmdline_file="${KATA_EXTENSION_CMDLINE_FILE:-/proc/cmdline}"
# The cmdline is a single line. Tolerate a missing/empty file and a fixture
# without a trailing newline.
cmdline=""
read -r cmdline < "${cmdline_file}" || [[ -n "${cmdline}" ]] || exit 0

for param in ${cmdline}; do
	# An extension may carry verity params ("...verity_params=<...>") or none
	# at all -- an unmeasured extension (e.g. on s390x) renders as a bare
	# "...verity_params" with no value. Match both forms so the extension is
	# activated either way; the mount helper decides verity vs. raw from the
	# parameter value.
	case "${param}" in
		kata.extension.*.verity_params=*)
			name="${param#kata.extension.}"
			name="${name%%.verity_params=*}"
			;;
		kata.extension.*.verity_params)
			name="${param#kata.extension.}"
			name="${name%.verity_params}"
			;;
		*)
			continue
			;;
	esac

	# Only accept names safe to use as a systemd instance and as a filename.
	# A stray space or '=' would mean a malformed cmdline entry; skip it rather
	# than create a bogus unit instance.
	[[ "${name}" =~ ^[a-zA-Z0-9_-]+$ ]] || continue
	instance="kata-extension-mount@${name}.service"
	# The complete instance must also fit in a single filesystem component.
	[[ "${#instance}" -le 255 ]] || continue

	mkdir -p "${wants_dir}"
	ln -sf "${unit}" "${wants_dir}/${instance}"

	if [[ "${name}" == "apparmor" ]]; then
		# Requires together with After prevents the agent from starting if the
		# AppArmor mount fails. Other extensions retain the generic Wants.
		# The mount template retains its OnFailure=poweroff.target handling.
		mkdir -p "${agent_dropin_dir}"
		cat > "${agent_dropin_dir}/50-apparmor-extension.conf" <<EOF
[Unit]
Requires=kata-extension-mount@apparmor.service
After=kata-extension-mount@apparmor.service
EOF
	fi
done

exit 0
