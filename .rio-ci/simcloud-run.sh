#!/bin/bash -e

function simcloud_execute {
  TEST_SCRIPT=${1:-"run-test.sh"}

  CPU="cpus=8,sockets=1,cores=8"
  MEMORY="12G"
  DISK_SIZE="25G"

  PREFIX=/kata-containers

  USERNAME=ubuntu
  PASSWORD=asdfqwer
  PORT=2225

  SERVER_IMAGE="ubuntu-20.04-server-cloudimg-amd64.img"
  IMAGE_SOURCE="https://cloud-images.ubuntu.com/releases/focal/release/ubuntu-20.04-server-cloudimg-amd64.img"

  echo ""
  echo "..........................................................................."
  echo " Installing qemu-kvm and cloud-image-utils ... "
  echo "..........................................................................."
  echo ""

  sudo apt-get update
  sudo apt-get install -y sshpass qemu-kvm cloud-image-utils
  echo "ok"

  echo ""
  echo "..........................................................................."
  echo " Running pre-execution cleanup ... "
  echo "..........................................................................."
  echo ""

  rm -rf "${HOME}/.ssh/known_hosts"
  mkdir -p ${PREFIX}/simcloud
  cd ${PREFIX}/simcloud || exit 1
  echo "ok"

  echo ""
  echo "..........................................................................."
  echo " Loading virtual cloud server image ..."
  echo " Resizing disk to ${DISK_SIZE}"
  echo "..........................................................................."
  echo ""
  echo "Downloading ${IMAGE_SOURCE} ..."
  if ! [ -f ${SERVER_IMAGE} ]; then
    curl -L -f -s -o ${SERVER_IMAGE} ${IMAGE_SOURCE}
  fi
  echo "ok"
  echo "Resizing..."
  qemu-img resize ${SERVER_IMAGE} "+${DISK_SIZE}"

  echo ""
  echo "..........................................................................."
  echo " Writing cloud-init configuration and user-data scripts ..."
  echo "..........................................................................."
  echo ""
  cat >cloud-config.txt <<EOT
#cloud-config
password: ${PASSWORD}
chpasswd: { expire: False }
ssh_pwauth: True
package_update: false
packages:
- docker.io
- git
- gcc
- libseccomp-dev
- make
- python3-pip
- runc
mounts:
 - [ hostshare, ${PREFIX}, "9p", "defaults,nofail,trans=virtio", "0", "0" ]
EOT

  cat >user-script.sh <<EOT
#!/bin/bash
if ! cloud-init status; then
  echo "Cloud-init failed with \$?"
  mkdir -p "${PREFIX}/logs"
  journalctl > "${PREFIX}/logs/system-journal.log" || true
  init 0
fi

cd ${PREFIX}

echo "ready" >vm_ready

echo "Waiting for execution"

while [ ! -f test_result ]; do
  sleep 1
done

init 0

EOT

  write-mime-multipart -o user-data cloud-config.txt:text/cloud-config user-script.sh:text/x-shellscript
  cloud-localds user-data.img user-data
  echo "ok"

  echo ""
  echo "..........................................................................."
  echo " Starting qemu-kvm virtual machine ... "
  echo "..........................................................................."
  echo ""
  qemu-system-x86_64 \
    -machine type=q35,accel=kvm \
    -m ${MEMORY} \
    -cpu host \
    -smp ${CPU} \
    -drive file=${PREFIX}/simcloud/${SERVER_IMAGE},format=qcow2 \
    -drive file=${PREFIX}/simcloud/user-data.img,format=raw \
    -nographic \
    -show-cursor \
    -netdev user,id=mynet0,hostfwd=tcp::${PORT}-:22, -device e1000,netdev=mynet0 \
    -fsdev local,security_model=passthrough,id=fsdev0,path=${PREFIX} \
    -device virtio-9p-pci,id=fs0,fsdev=fsdev0,mount_tag=hostshare \
    -object rng-random,id=rng0,filename=/dev/urandom -device virtio-rng-pci,rng=rng0 \
    &
  echo "ok"

  echo ""
  echo "..........................................................................."
  echo " Waiting qemu-kvm virtual machine to become ready ... "
  echo "..........................................................................."
  echo ""

  TIMEOUT=300

  counter=0
  while [ ! -f ${PREFIX}/vm_ready ]; do
    sleep 5
    counter=$((counter + 5))
    echo "Waiting ${counter} seconds for VM ..."
    if [ "${counter}" -gt ${TIMEOUT} ]
    then
      echo "The VM never became ready, sorry!"
      exit 1
    fi
  done
  echo "ok"

  echo ""
  echo "..........................................................................."
  echo " Running test ... "
  echo "..........................................................................."
  echo ""

  sshpass -p ${PASSWORD} ssh -o StrictHostKeyChecking=no -p ${PORT} ${USERNAME}@localhost "sudo bash -c '${PREFIX}/simcloud/${TEST_SCRIPT}'"
  echo "ok"

  echo ""
  echo "..........................................................................."
  echo " Collecting results of testing ... "
  echo "..........................................................................."
  echo ""

  if [ ! -f ${PREFIX}/test_result ]; then
    echo "missing test result!"
    exit 1
  fi

  RESULT=$(cat "${PREFIX}/test_result")
  echo "Result code of test: ${RESULT}"
  if [[ "${RESULT}" != "0" ]]; then
    echo "test failed!"
    tar -zcvf ${PREFIX}/logs.tar.gz ${PREFIX}/logs
    exit 1
  fi

  touch ${PREFIX}/logs.tar.gz
  echo "test run succeeded!"
  exit 0
}
