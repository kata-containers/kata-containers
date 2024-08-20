#/bin/bash

STATE_DISK_SIZE=300
VM_MEMORY=32
VM_CPU=12

export DISTRO="ubuntu"
SCRIPT_DIR="$( cd "$( dirname "$0" )" && pwd )"
export ROOTFS_DIR="${SCRIPT_DIR}/build/rootfs"
#export PROVIDER_CONFIG_DST="/provider_config"
#PROVIDER_CONFIG_SRC="/etc"

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
wget "https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2204/x86_64/cuda-keyring_1.1-1_all.deb"
popd

pushd "${SCRIPT_DIR}/tools/osbuilder/rootfs-builder"
script -fec 'sudo -E USE_DOCKER=true PROVIDER_CONFIG_DST="${PROVIDER_CONFIG_DST}" CONFIDENTIAL_GUEST=yes MEASURED_ROOTFS=yes EXTRA_PKGS="openssh-server netplan.io curl htop open-iscsi ubuntu-minimal dmsetup ca-certificates" ./rootfs.sh "${DISTRO}"'
popd

pushd "${SCRIPT_DIR}/tools/osbuilder/image-builder"
script -fec 'sudo -E USE_DOCKER=true MEASURED_ROOTFS=yes ./image_builder.sh "${ROOTFS_DIR}"'
popd

cp "${SCRIPT_DIR}/tools/osbuilder/image-builder/kata-containers.img" "${SCRIPT_DIR}/build/rootfs.img"
cp "${SCRIPT_DIR}/tools/osbuilder/image-builder/root_hash.txt" "${SCRIPT_DIR}/build/"
cp -L "${SCRIPT_DIR}/tools/packaging/kata-deploy/local-build/build/kernel-${KERNEL_NAME}/destdir/opt/kata/share/kata-containers/vmlinuz-${KERNEL_NAME}.container" "${SCRIPT_DIR}/build/vmlinuz"

pushd "${SCRIPT_DIR}/build"
#qemu-img create -f qcow2 state.qcow2 ${STATE_DISK_SIZE}G

ROOT_HASH=$(grep 'Root hash' root_hash.txt | awk '{print $3}')

PWD_COMMAND='SCRIPT_DIR=$( cd "$( dirname "$0" )" && pwd )'
NVIDIA_PASSTHROUGH=" -device pcie-root-port,id=pci.1,bus=pcie.0 -device \
                     vfio-pci,host=2a:00.0,bus=pci.1 -fw_cfg name=opt/ovmf/X-PciMmio64,string=262144"
QEMU_COMMAND="
qemu-system-x86_64 \
-accel kvm \
-append \"root=/dev/vda1 console=ttyS0 systemd.log_level=trace systemd.log_target=log rootfs_verity.scheme=dm-verity rootfs_verity.hash=${ROOT_HASH}\" \
-bios /usr/share/qemu/OVMF.fd \
-chardev stdio,id=mux,mux=on,logfile=\$SCRIPT_DIR/vm_log_\$(date +\"%FT%H%M\").log \
-cpu host,-kvm-steal-time,pmu=off \
${NVIDIA_PASSTHROUGH} \
-device vhost-vsock-pci,guest-cid=3 \
-device virtio-net-pci,netdev=nic0_td -netdev user,id=nic0_td,hostfwd=tcp::2222-:22 \
-drive file=\$SCRIPT_DIR/rootfs.img,if=virtio,format=raw \
-drive file=\$SCRIPT_DIR/state.qcow2,if=virtio,format=qcow2 \
-kernel \$SCRIPT_DIR/vmlinuz \
-smp ${VM_CPU} -m ${VM_MEMORY}G -vga none \
-machine q35,kernel_irqchip=split,confidential-guest-support=tdx,memory-backend=ram1 \
-monitor chardev:mux -serial chardev:mux -nographic \
-monitor pty \
-name process=tdxvm,debug-threads=on \
-no-hpet -nodefaults -nographic \
-object memory-backend-memfd-private,id=ram1,size=${VM_MEMORY}G \
-object tdx-guest,sept-ve-disable=on,id=tdx \
"
if [ -n "${PROVIDER_CONFIG_SRC}" ] && [ -n "${PROVIDER_CONFIG_DST}" ]; then
    QEMU_COMMAND+=" -fsdev local,security_model=passthrough,id=fsdev0,path=${PROVIDER_CONFIG_SRC} \
                  -device virtio-9p-pci,fsdev=fsdev0,mount_tag=sharedfolder"
fi

echo "${PWD_COMMAND}" > run_vm.sh
echo "${QEMU_COMMAND}" >> run_vm.sh

chmod +x run_vm.sh
popd
