#/bin/bash

set -euo pipefail;

STATE_DISK_SIZE=300
VM_MEMORY=128
VM_CPU=8

export DISTRO="ubuntu"
SCRIPT_DIR="$( cd "$( dirname "$0" )" && pwd )"
export ROOTFS_DIR="${SCRIPT_DIR}/build/rootfs"

PROVIDER_CONFIG_DST="${1:-/sp}"
export PROVIDER_CONFIG_DST

KERNEL_NAME=nvidia-gpu-confidential

pushd "${SCRIPT_DIR}/tools/packaging/kata-deploy/local-build"
./kata-deploy-binaries-in-docker.sh --build="kernel-${KERNEL_NAME}"
popd

rm -rf "${SCRIPT_DIR}/build"
mkdir -p "${SCRIPT_DIR}/build/rootfs/opt/deb"

pushd "${SCRIPT_DIR}/build/rootfs/opt/deb"
find "${SCRIPT_DIR}/tools/packaging/kata-deploy/local-build/build/kernel-${KERNEL_NAME}/builddir/" -name "*.deb" -exec cp {} . \;
mkdir nvidia
cd nvidia
wget "https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2404/x86_64/cuda-keyring_1.1-1_all.deb"
popd

pushd "${SCRIPT_DIR}/tools/osbuilder/rootfs-builder"
script -fec 'sudo -E USE_DOCKER=true PROVIDER_CONFIG_DST="${PROVIDER_CONFIG_DST}" CONFIDENTIAL_GUEST=yes MEASURED_ROOTFS=yes EXTRA_PKGS="init openssh-server netplan.io curl htop open-iscsi cryptsetup ca-certificates gnupg2 kmod" ./rootfs.sh "${DISTRO}"'
popd

pushd "${SCRIPT_DIR}/tools/osbuilder/image-builder"
script -fec 'sudo -E USE_DOCKER=true MEASURED_ROOTFS=yes ./image_builder.sh "${ROOTFS_DIR}"'
popd

cp "${SCRIPT_DIR}/tools/osbuilder/image-builder/kata-containers.img" "${SCRIPT_DIR}/build/rootfs.img"
cp "${SCRIPT_DIR}/tools/osbuilder/image-builder/root_hash.txt" "${SCRIPT_DIR}/build/"
cp -L "${SCRIPT_DIR}/tools/packaging/kata-deploy/local-build/build/kernel-${KERNEL_NAME}/destdir/opt/kata/share/kata-containers/vmlinuz-${KERNEL_NAME}.container" "${SCRIPT_DIR}/build/vmlinuz"

pushd "${SCRIPT_DIR}/build"
qemu-img create -f qcow2 state.qcow2 ${STATE_DISK_SIZE}G

# temporarily store the firmware in the repository
cp "${SCRIPT_DIR}/tools/osbuilder/rootfs-builder/ubuntu/superprotocol"/{OVMF.fd,OVMF_AMD.fd} "${SCRIPT_DIR}/build"

ROOT_HASH=$(grep 'Root hash' root_hash.txt | awk '{print $3}')

PWD_COMMAND='SCRIPT_DIR=$( cd "$( dirname "$0" )" && pwd )'
NVIDIA_PASSTHROUGH="-object iommufd,id=iommufd0 \\
-device pcie-root-port,id=pci.1,bus=pcie.0 \\
-device vfio-pci,host=2a:00.0,bus=pci.1,iommufd=iommufd0 -fw_cfg name=opt/ovmf/X-PciMmio64,string=262144"

QEMU_COMMAND="
qemu-system-x86_64 \\
-enable-kvm \\
-append \"root=/dev/vda1 console=ttyS0 clearcpuid=mtrr systemd.log_level=trace systemd.log_target=log rootfs_verity.scheme=dm-verity rootfs_verity.hash=${ROOT_HASH}\" \\
-drive file=\$SCRIPT_DIR/rootfs.img,if=virtio,format=raw \\
-drive file=\$SCRIPT_DIR/state.qcow2,if=virtio,format=qcow2 \\
-kernel \$SCRIPT_DIR/vmlinuz \\
-smp cores=${VM_CPU} \\
-m ${VM_MEMORY}G \\
-cpu host \\
-object '{\"qom-type\":\"tdx-guest\",\"id\":\"tdx\",\"quote-generation-socket\":{\"type\": \"vsock\", \"cid\":\"2\",\"port\":\"4050\"}}' \\
-netdev user,id=n1,ipv6=off,hostfwd=tcp:127.0.0.1:2222-:22 \\
-device virtio-net-pci,netdev=n1 \\
-nographic \\
-object memory-backend-ram,id=mem0,size=${VM_MEMORY}G \\
-machine q35,kernel-irqchip=split,confidential-guest-support=tdx,memory-backend=mem0 \\
-bios \$SCRIPT_DIR/OVMF.fd \\
-vga none \\
-nodefaults \\
-serial stdio \\
${NVIDIA_PASSTHROUGH} \\
-device vhost-vsock-pci,guest-cid=3 \\
"

if [ -n "${PROVIDER_CONFIG_DST}" ]; then
    PROVIDER_CONFIG_SRC="\${SCRIPT_DIR}/config"
    QEMU_COMMAND+=" -fsdev local,security_model=passthrough,id=fsdev0,path=${PROVIDER_CONFIG_SRC} \\
                  -device virtio-9p-pci,fsdev=fsdev0,mount_tag=sharedfolder"
fi

echo "${PWD_COMMAND}" > run_vm.sh
echo "${QEMU_COMMAND}" >> run_vm.sh

chmod +x run_vm.sh
popd
