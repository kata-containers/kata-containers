# Install Kata Containers on CentOS

1. Install the Kata Containers components with the following commands:

   ```bash
   $ source /etc/os-release
   $ cat <<EOF | sudo -E tee /etc/yum.repos.d/advanced-virt.repo
     [advanced-virt]
     name=Advanced Virtualization
     baseurl=http://mirror.centos.org/\$contentdir/\$releasever/virt/\$basearch/advanced-virtualization
     enabled=1
     gpgcheck=1
     skip_if_unavailable=1
     EOF
   $ cat <<EOF | sudo -E tee /etc/yum.repos.d/kata-containers.repo
     [kata-containers]
     name=Kata Containers
     baseurl=http://mirror.centos.org/\$contentdir/\$releasever/virt/\$basearch/kata-containers
     enabled=1
     gpgcheck=1
     skip_if_unavailable=1
     EOF
   $ sudo -E dnf module disable -y virt:rhel
   $ sudo -E dnf install -y kata-runtime
   ```

2. Decide which container manager to use and select the corresponding link that follows:
   - [Kubernetes](../Developer-Guide.md#run-kata-containers-with-kubernetes)
