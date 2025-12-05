# Kata Containers Development Workflow Guide

This guide describes how to build Kata Containers locally, publish images to a registry, and testthem on a single-node Kubernetes VM.

## Main flow

1. Develop and build locally on the host
2. Package artifacts into a tarball or a kata-deploy image
3. Push the image to a registry
4. Install and validate on a single-node Kubernetes (inside a disposable test VM) using Helm

---

## 1. Install kcli and create a VM

Use kcli (a wrapper around libvirt) to quickly create a throwaway VM for testing.

### 1.1 Install prerequisites

Install Libvirt, KVM, genisoimage and other dependencies required by kcli:

```bash
sudo apt install qemu-kvm libvirt-daemon-system genisoimage
sudo usermod -aG qemu,libvirt $(id -un)
sudo newgrp libvirt
sudo systemctl enable --now libvirtd
```

### 1.2 Install kcli

```bash
 curl -s https://raw.githubusercontent.com/karmab/kcli/main/install.sh | bash
```

### 1.3 Configure VM profiles

Define a VM profiles. If the images are missing, kcli will fetch them.

```bash
mkdir -p ${HOME}/.kcli
touch ${HOME}/.kcli/profiles.yml
```

edit {HOME}/.kcli/profiles.yml

```yaml
# ${HOME}/.kcli/profiles.yml

dev-24.04:
  image: ubuntu2404
  numcpus: 4
  memory: 16384
  disks:
    - size: 40
```

### 1.4 Create a VM

Libvirt stores VM disks in storage pools. Create the default pool at /var/lib/libvirt/images and grant your user write access

```bash
sudo kcli create pool -p /var/lib/libvirt/images default
sudo setfacl -m u:$(id -un):rwx /var/lib/libvirt/images
```

Then create a development VM from the profiles above.

```bash
kcli create vm -p dev-24.04 {your_VMname}
```

### 1.5 List VMs

Check that the VM is running and reachable.

```bash
kcli list vm
# (optional) show more details
kcli info vm {your_VMname}
```

### 1.6 Install Docker (for image/rootfs builds)

Docker is required later by osbuilder when building a image.
Install Docker using the [official documentation](https://docs.docker.com/engine/install/ubuntu/#install-using-the-convenience-script).

---

## 2. Configure the VM environment

### 2.1 Start the VM (if needed) and SSH in

Start the VM if it is stopped, then connect. If it’s already running, kcli will report it and do nothing.

```bash
# Power on the VM if not running
kcli start vm {your_VMname}

# (optional) verify state; it should show "running"
kcli list vm

# Connect via SSH (kcli uses the injected key/user from the cloud image)
kcli ssh {your_VMname}
```

Note: It can take a short while after boot for SSH to become available. If SSH fails initially, wait a few seconds and retry.

### 2.2 Set environment variables and aliases

These variables control which container engine and Kubernetes flavor the helper script deploys. The aliases speed up common tasks.
Edit `~/.bashrc` file and add:

```bash
export EDITOR="vim" # fidencio's preferred editor
export CONTAINER_ENGINE="containerd" # whether to use containerd or crio as the container engine -- fidencio's preferred choice is containerd
export CONTAINER_ENGINE_VERSION="v2.2" # the version of the containerd to be used -- fidencio's preferred choice is always the latest released version
export KUBERNETES="vanilla" # it can be vanilla (as in, kubeadm deploted), k3s, rke2, k0s, or microk8s -- fidencio's preferred choise is to use the vanilla option

alias install_utils='sudo apt-get update && sudo apt-get -y install vim tig build-essential'
alias clone_kata='git clone https://github.com/kata-containers/kata-containers $HOME/kata-containers'
alias deploy_k8s='pushd $HOME/kata-containers && bash tests/integration/kubernetes/gha-run.sh deploy-k8s && popd'
alias setup_all='clone_kata && deploy_k8s'
```

Apply the changes:

```bash
source ~/.bashrc
```

### 2.3 Set up Kata Containers and Kubernetes

Run the alias to clone the repo and deploy a single-node Kubernetes via the integration script. This may take several minutes.

```bash
setup_all
```

Quick check:

```bash
kubectl get nodes
```

---

## 3. Host development environment packaging

Run this section on your development machine (the host where you build Kata), not inside the Kubernetes VM.

### 3.1 Build tarballs

Clone the repo and build the tarball(s) for the components you are modifying.

```bash
# Clone kata-containers
git clone https://github.com/kata-containers/kata-containers

# Build tarballs for the components you need
make <component>-tarball
# e.g.: make agent-tarball
```

After the build, tarballs are usually in：`/tools/packaging/kata-deploy/local-build/build`

### 3.2 Build and push the kata-deploy image

You will publish your local artifacts in a kata-deploy image and push it to quay.io.
**Note:** you need a [quay.io](https://quay.io) account to push to quay.io

```bash
docker login quay.io
```

#### a. Build the image

Change to the kata-deploy directory and copy the built tarball(s) into that folder. 

Then build the image:

```bash
cd tools/packaging/kata-deploy
# copy your tarball from tools/packaging/kata-deploy/local-build/build into this directory
docker build --build-arg KATA_ARTIFACTS='<tar name}' -t quay.io/<namespace>/kata-deploy:dev-<tag> .
```

#### b. Push the image to the registry

```bash
docker push quay.io/<namespace>/kata-deploy:dev-<tag>
```

After pushing, you should see the image in your quay.io repository.

---

## 4. Install and validate on Kubernetes using Helm

### 4.1 Install Helm and kata-deploy

#### a. Install Helm

```bash
curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
```

#### b. Install kata-deploy

```bash
export VERSION=$(curl -sSL https://api.github.com/repos/kata-containers/kata-containers/releases/latest | jq .tag_name | tr -d '"')
export CHART="oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy"

helm install kata-deploy "${CHART}" --version "${VERSION}"
```

### 4.2 Update the installation

```bash
# Upgrade in‑place, keeping everything else the same
helm upgrade --install kata-deploy -n kube-system --create-namespace --set env.defaultShim=qemu-runtime-rs "$CHART" --version "$VERSION"
```

### 4.3 Configure a custom image registry

Point the chart to the kata-deploy image you built and pushed to quay.io.

#### a. Edit values.yaml

```bash
cd ~/kata-containers/tools/packaging/kata-deploy/helm-chart/kata-deploy
```

Modify the `values.yaml` in the current directory to point to your quay image:

```yaml
image:
  reference: quay.io/<namespace>/kata-deploy
  tag: <image_tag>
```

#### b. Apply your custom values

```bash
helm upgrade kata-deploy -f values.yaml "${CHART}" --version "${VERSION}" --namespace kube-system
```

#### c. Handle private registry credentials (if needed)

> If your quay.io repository is private, create an image pull secret and reference it in the chart, or make the repository public on the quay.io website.

Create the imagePullSecret:

```bash
kubectl -n kube-system create secret docker-registry quay-cred
--docker-server=quay.io
--docker-username=<quay.io_username>
--docker-password=<quay.io_password>
```

Reference the secret in values.yaml:

```yaml
imagePullSecrets:
  - name: quay-pull-secret
```

Then update the chart：

```bash
helm upgrade kata-deploy -f values.yaml "${CHART}" --version "${VERSION}" --namespace kube-system
```

### 4.4 Verify deployment

Wait for the daemonset to roll out.

```bash
kubectl -n kube-system rollout status ds/kata-deploy
```

At this point the environment is set up and you can start testing the image you pushed earlier.

**Happy hacking!**


