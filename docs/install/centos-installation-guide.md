# Install Kata Containers on CentOS

1. Install the Kata Containers components with the following commands:

   ```bash
   $ sudo -E dnf install -y centos-release-advanced-virtualization
   $ sudo -E dnf module disable -y virt:rhel
   $ source /etc/os-release
   $ cat <<EOF | sudo -E tee /etc/yum.repos.d/kata-containers.repo
     [kata-containers]
     name=Kata Containers
     baseurl=http://mirror.centos.org/\$contentdir/\$releasever/virt/\$basearch/kata-containers
     enabled=1
     gpgcheck=1
     skip_if_unavailable=1
     EOF
   $ sudo -E dnf install -y kata-containers
   ```

2. Decide which container manager to use and select the corresponding link that follows:
   - [Kubernetes](../Developer-Guide.md#run-kata-containers-with-kubernetes)
