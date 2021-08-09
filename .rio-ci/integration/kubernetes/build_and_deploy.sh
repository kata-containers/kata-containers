#!/bin/bash
set -e

function build_agent() {
  local script_dir=$1

  echo "Building kata-agent"
  pushd "$script_dir/../../../src/agent"
  export PATH=$HOME/.cargo/bin:$PATH
  make SECCOMP=no
  popd
}

function build_runtime() {
  local script_dir=$1

  echo "Building kata-runtime"
  pushd "$script_dir/../../../src/runtime"
  export PATH=$PATH:/usr/local/go/bin
  make
  popd
}

script_dir=$(dirname "$(readlink -f "$0")")
hostname=$1

SHOULD_TEARDOWN="${SHOULD_TEARDOWN:-true}"

if [ "${SHOULD_TEARDOWN}" = "true" ]; then
  trap '${script_dir}/teardown.sh' ERR
fi

export HOST_PASSWORD="${HOST_PASSWORD:-Rekur\$i0n}"
export KATA_SHIM_PATH="${KATA_SHIM_PATH:-/usr/bin/containerd-shim-kata-v2}"
export KATA_RUNTIME_PATH="${KATA_RUNTIME_PATH:-/usr/bin/kata-runtime}"
export KATA_CONFIG_QEMU_PATH="${KATA_CONFIG_QEMU_PATH:-/etc/kata-containers/configuration-qemu.toml}"
export KATA_CONFIG_CLH_PATH="${KATA_CONFIG_CLH_PATH:-/etc/kata-containers/configuration-clh.toml}"
export AGENT_TARGET="${AGENT_TARGET:-x86_64-unknown-linux-musl}"

build_runtime "${script_dir}"
build_agent "${script_dir}"

echo "Updating kata-runtime"
pushd "$script_dir/../../../src/runtime"
sshpass -p "${HOST_PASSWORD}" scp containerd-shim-kata-v2 "${hostname}":"${KATA_SHIM_PATH}"
sshpass -p "${HOST_PASSWORD}" scp kata-runtime "${hostname}":"${KATA_RUNTIME_PATH}"
sshpass -p "${HOST_PASSWORD}" scp config/configuration-qemu.toml "${hostname}":"${KATA_CONFIG_QEMU_PATH}"
sshpass -p "${HOST_PASSWORD}" scp config/configuration-clh.toml "${hostname}":"${KATA_CONFIG_CLH_PATH}"
popd

echo "Updating kata-agent"
pushd "$script_dir/../../../src/agent"
sshpass -p "${HOST_PASSWORD}" scp "target/${AGENT_TARGET}/release/kata-agent" "${hostname}":/usr/bin/kata-agent
popd

mount_path="/agent-mnt"
sshpass -p "${HOST_PASSWORD}" ssh "${hostname}" "mkdir -p ${mount_path} && \
mount -o loop,offset=\$((512*6144)) /usr/share/kata-containers/kata-containers.img ${mount_path} && \
mv -f /usr/bin/kata-agent ${mount_path}/bin/kata-agent && \
umount ${mount_path}"

# HACK:
sshpass -p "${HOST_PASSWORD}" ssh "${hostname}" "sudo yum install -y kata-qemu kata-qemu-data kata-qemu-bin"
