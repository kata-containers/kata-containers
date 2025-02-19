#!/bin/bash
#
# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# Run the .ci/jenkins_job_build.sh script in a VM
# that supports VFIO, then run VFIO functional tests

set -o xtrace
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

cidir=$(readlink -f $(dirname "$0"))

source /etc/os-release || source /usr/lib/os-release
# <CHANGES HERE>
source "${cidir}/../../common.bash"
export WORKSPACE="${WORKSPACE:-${HOME}}"
export GIT_URL="https://github.com/kata-containers/kata-containers.git"
export KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"
# </CHANGES>

http_proxy=${http_proxy:-}
https_proxy=${https_proxy:-}
vm_ip="127.0.15.1"
vm_port="10022"
# Don't save data in /tmp, we need it after rebooting the system
data_dir="${HOME}/functional-vfio-test"
ssh_key_file="${data_dir}/key"
arch=$(uname -m)
artifacts_dir="${WORKSPACE}/artifacts"

kill_vms() {
	sudo killall -9 qemu-system-${arch}
}

cleanup() {
	mkdir -p ${artifacts_dir}
	sudo chown -R ${USER} ${artifacts_dir}
	scp_vm ${artifacts_dir}/* ${artifacts_dir} || true
	kill_vms

	echo "::group::L2 journal"
	cat "${artifacts_dir}/journal.log"
	echo "::endgroup::"

	echo "::group::L1 dmesg"
	sudo dmesg
	echo "::endgroup::"
}

create_ssh_key() {
	rm -f "${ssh_key_file}"
	ssh-keygen -f "${ssh_key_file}" -t rsa -N ""
}

create_meta_data() {
	file="$1"
	cat <<EOF > "${file}"
{
  "uuid": "d1b4aafa-5d75-4f9c-87eb-2ceabe110c39",
  "hostname": "test"
}
EOF
}

create_user_data() {
	file="$1"
	ssh_pub_key_file="$2"

	ssh_pub_key="$(cat "${ssh_pub_key_file}")"
	dnf_proxy=""
	service_proxy=""
	docker_user_proxy="{}"
	environment=$(env | grep -E "ghprb|WORKSPACE|KATA|GIT|JENKINS|_PROXY|_proxy" | \
	                    sed -e "s/'/'\"'\"'/g" \
	                        -e "s/\(^[[:alnum:]_]\+\)=/\1='/" \
	                        -e "s/$/'/" \
	                        -e 's/^/    export /')

	if [ -n "${http_proxy}" ] && [ -n "${https_proxy}" ]; then
		dnf_proxy="proxy=${http_proxy}"
		service_proxy='[Service]
    Environment="HTTP_PROXY='${http_proxy}'" "HTTPS_PROXY='${https_proxy}'" "NO_PROXY='${no_proxy}'"'
		docker_user_proxy='{"proxies": { "default": {
    "httpProxy": "'${http_proxy}'",
    "httpsProxy": "'${https_proxy}'",
    "noProxy": "'${no_proxy}'"
    } } }'
	fi

	cat <<EOF > "${file}"
#cloud-config
package_upgrade: false
runcmd:
- chown -R ${USER}:${USER} /home/${USER}
- touch /.done
users:
- gecos: User
  gid: "1000"
  lock-passwd: true
  name: ${USER}
  shell: /bin/bash
  ssh-authorized-keys:
  - ${ssh_pub_key}
  sudo: ALL=(ALL) NOPASSWD:ALL
  uid: "1000"
write_files:
- content: |
    [main]
    fastestmirror=True
    gpgcheck=1
    max_parallel_downloads=10
    installonly_limit=2
    clean_requirements_on_remove=True
    keepcache=True
    ip_resolve=4
  path: /etc/dnf/dnf.conf
- content: |
${environment}
  path: /etc/environment
- content: |
    ${service_proxy}
  path: /etc/systemd/system/docker.service.d/http-proxy.conf
- content: |
    ${service_proxy}
  path: /etc/systemd/system/containerd.service.d/http-proxy.conf
- content: |
    ${docker_user_proxy}
  path: ${HOME}/.docker/config.json
- content: |
    ${docker_user_proxy}
  path: /root/.docker/config.json
- content: |
    set -x
    set -o errexit
    set -o nounset
    set -o pipefail
    set -o errtrace
    . /etc/environment
    . /etc/os-release

    [ "\$ID" = "fedora" ] || (echo >&2 "$0 only supports Fedora"; exit 1)

    echo "${dnf_proxy}" | sudo tee -a /etc/dnf/dnf.conf

    for i in \$(seq 1 50); do
        [ -f /.done ] && break
        echo "waiting for cloud-init to finish"
        sleep 5;
    done

    export DEBUG=true
    export GOPATH=\${WORKSPACE}/go
    export PATH=\${GOPATH}/bin:/usr/local/go/bin:/usr/sbin:\${PATH}
    export GOROOT="/usr/local/go"

    # Make sure the packages were installed
    # Sometimes cloud-init is unable to install them
    sudo dnf install -y git wget pciutils driverctl

    git config --global user.email "foo@bar"
    git config --global user.name "Foo Bar"

    sudo mkdir -p /workspace
    sudo mount -t 9p -o access=any,trans=virtio,version=9p2000.L workspace /workspace
    mkdir -p ${artifacts_dir}
    trap "cd /workspace; sudo journalctl -b0 > ${artifacts_dir}/journal.log || true; sudo chown -R \${USER} ${artifacts_dir}" EXIT

    pushd /workspace
    source tests/common.bash
    ensure_yq
    cri_containerd=\$(get_from_kata_deps ".externals.containerd.lts")
    cri_tools=\$(get_from_kata_deps ".externals.critools.latest")
    install_cri_containerd \${cri_containerd}
    install_cri_tools \${cri_tools}

    kata_tarball_dir="kata-artifacts"
    install_kata

    sudo /workspace/tests/functional/vfio/run.sh -s false -p \${KATA_HYPERVISOR} -m q35 -i image
    sudo /workspace/tests/functional/vfio/run.sh -s true -p \${KATA_HYPERVISOR} -m q35 -i image

  path: /home/${USER}/run.sh
  permissions: '0755'
EOF
}

create_config_iso() {
	iso_file="$1"
	ssh_pub_key_file="${ssh_key_file}.pub"
	iso_data_dir="${data_dir}/d"
	meta_data_file="${iso_data_dir}/openstack/latest/meta_data.json"
	user_data_file="${iso_data_dir}/openstack/latest/user_data"

	mkdir -p $(dirname "${user_data_file}")

	create_meta_data "${meta_data_file}"
	create_user_data "${user_data_file}" "${ssh_pub_key_file}"

	[ -f "${iso_file}" ] && rm -f "${iso_file}"

	xorriso -as mkisofs -R -V config-2 -o "${iso_file}" "${iso_data_dir}"
}

pull_fedora_cloud_image() {
	fedora_img="$1"
	fedora_version=38
	# Add a version to the image cache, otherwise the tests are going to
	# use always the same image without rebuilding it, regardless the version
	# set in fedora_version
	fedora_img_cache="${fedora_img}.cache.${fedora_version}"
	fedora_img_url="https://download.fedoraproject.org/pub/fedora/linux/releases/${fedora_version}/Cloud/${arch}/images/Fedora-Cloud-Base-${fedora_version}-1.6.${arch}.raw.xz"

	if [ ! -f "${fedora_img_cache}" ]; then
		curl -sL ${fedora_img_url} -o "${fedora_img_cache}.xz"
		xz -f -d "${fedora_img_cache}.xz"
	fi

	cp -a "${fedora_img_cache}" "${fedora_img}"

	# setup cloud image
	sudo losetup -D
	loop=$(sudo losetup --show -Pf "${fedora_img}")
	sudo mount "${loop}p2" /mnt

	# add intel_iommu=on to the guest kernel command line
	kernelopts="intel_iommu=on iommu=pt selinux=0 mitigations=off idle=poll kvm.tdp_mmu=0"
	entries=$(sudo ls /mnt/loader/entries/)
	for entry in ${entries}; do
		sudo sed -i '/^options /  s/$/ '"${kernelopts}"' /g' /mnt/loader/entries/"${entry}"
	done
	sudo sed -i 's|kernelopts="|kernelopts="'"${kernelopts}"'|g' /mnt/grub2/grub.cfg
	sudo sed -i 's|kernelopts=|kernelopts='"${kernelopts}"'|g' /mnt/grub2/grubenv

	# cleanup
	sudo umount -R /mnt/
	sudo losetup -d "${loop}"

	qemu-img resize -f raw "${fedora_img}" +20G
}

reload_kvm() {
	# TDP_MMU is buggy on Hyper-V until v6.3/v6.4
	sudo rmmod kvm-intel kvm-amd kvm || true
	sudo modprobe kvm tdp_mmu=0
	sudo modprobe kvm-intel || true
	sudo modprobe kvm-amd || true
}

run_vm() {
	image="$1"
	config_iso="$2"
	disable_modern="off"
	hostname="$(hostname)"
	memory="8192M"
	cpus=2
	machine_type="q35"

	reload_kvm

	sudo /usr/bin/qemu-system-${arch} -m "${memory}" -smp cpus="${cpus}" \
	   -cpu host,host-phys-bits \
	   -machine ${machine_type},accel=kvm,kernel_irqchip=split \
	   -device intel-iommu,intremap=on,caching-mode=on,device-iotlb=on \
	   -drive file=${image},if=virtio,aio=threads,format=raw \
	   -drive file=${config_iso_file},if=virtio,media=cdrom \
	   -daemonize -enable-kvm -device virtio-rng-pci -display none -vga none \
	   -netdev user,hostfwd=tcp:${vm_ip}:${vm_port}-:22,hostname="${hostname}",id=net0 \
	   -device virtio-net-pci,netdev=net0,disable-legacy=on,disable-modern="${disable_modern}",iommu_platform=on,ats=on \
	   -netdev user,id=net1 \
	   -device virtio-net-pci,netdev=net1,disable-legacy=on,disable-modern="${disable_modern}",iommu_platform=on,ats=on \
	   -fsdev local,path=${repo_root_dir},security_model=passthrough,id=fs0 \
	   -device virtio-9p-pci,fsdev=fs0,mount_tag=workspace

}

ssh_vm() {
	cmd=$@
	ssh -q -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no -o IdentitiesOnly=yes -i "${ssh_key_file}" -p "${vm_port}" "${USER}@${vm_ip}" "${cmd}"
}

scp_vm() {
	guest_src=$1
	host_dest=$2
	scp -q -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no -o IdentitiesOnly=yes -i "${ssh_key_file}" -P "${vm_port}" ${USER}@${vm_ip}:${guest_src} ${host_dest}
}

wait_for_vm() {
	for i in $(seq 1 30); do
		if ssh_vm true; then
			return 0
		fi
		info "waiting for VM to start"
		sleep 5
	done
	return 1
}

main() {
	trap cleanup EXIT

	config_iso_file="${data_dir}/config.iso"
	fedora_img="${data_dir}/image.img"

	mkdir -p "${data_dir}"

	create_ssh_key

	create_config_iso "${config_iso_file}"

	for i in $(seq 1 5); do
		pull_fedora_cloud_image "${fedora_img}"
		run_vm "${fedora_img}" "${config_iso_file}"
		if wait_for_vm; then
			break
		fi
		info "Couldn't connect to the VM. Stopping VM and starting a new one."
		kill_vms
	done

	ssh_vm "/home/${USER}/run.sh"
}

main $@
