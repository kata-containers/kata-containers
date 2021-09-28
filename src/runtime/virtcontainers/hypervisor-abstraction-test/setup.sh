#!/bin/bash

if ! command -v cloud-localds &> /dev/null
then 
   echo "cloud-localdscould not be found. Please install. On Debian" 
   echo "sudo apt-get install cloud-image-util"
fi

VMN=${VMN:=1}
IMAGE=${IMAGE:='focal-server-cloudimg-amd64.img'}
FIRMWARE=${FIRMWARE:='hypervisor-fw'}
CONFIG_DRIVE=${CONFIG_DRIVE:='config-drive.img'}

echo "Image: " $IMAGE
echo "Format: " $FORMAT
echo "Config drive: " $CONFIG_DRIVE

if [ ! -f "$IMAGE" ]; then
    >&2 echo "Can't find image file \"$IMAGE\". Downloading..."
    wget https://cloud-images.ubuntu.com/focal/current/focal-server-cloudimg-amd64.img
fi

if [ ! -f "$FIRMWARE" ]; then
    >&2 echo "Can't find firmware file \"$FIRMWARE\". Downloading..."
    wget https://github.com/cloud-hypervisor/rust-hypervisor-firmware/releases/download/0.3.1/hypervisor-fw
fi

if [ ! -f "cloud-hypervisor-static" ]; then
    wget https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/v15.0/cloud-hypervisor-static
fi

ssh_key=$(cat ~/.ssh/id_rsa.pub)
cat << EOF > cloud-init-config.cfg
#cloud-config
users:
  - name: ubuntu
    sudo: ALL=(ALL) NOPASSWD:ALL
    groups: users, admin
    home: /home/ubuntu
    shell: /bin/bash
    lock_passwd: false
    ssh-authorized-keys:
      - $ssh_key
# only cert auth via ssh (console access can still login)
ssh_pwauth: false
disable_root: false
chpasswd:
  list: |
     ubuntu:ubuntu
  expire: False

# written to /var/log/cloud-init-output.log
final_message: "The system is finally up, after $UPTIME seconds"
EOF

cat << EOF > network-config-1.cfg
version: 1
config:
  - type: physical
    name: ens3
    mac_address: "52:54:00:12:34:00"
    subnets:
      - type: static
        address: 10.0.2.15
        netmask: 255.255.255.0
        gateway: 10.0.2.1
EOF

cat << EOF > network-config-2.cfg
version: 2
ethernets:
  ens3:
     match:
         mac_address: "52:54:00:12:34:00"
     set-name: eth0
     addresses:
     - 10.0.2.15/24
     gateway4: 10.0.2.1
EOF

rm -f focal-server-cloudimg-amd64.raw
qemu-img convert -p -f qcow2 -O raw focal-server-cloudimg-amd64.img focal-server-cloudimg-amd64.raw
rm -f "$CONFIG_DRIVE"
cloud-localds -v "$CONFIG_DRIVE" --network-config=network-config-1.cfg cloud-init-config.cfg
chmod +x ./cloud-hypervisor-static
