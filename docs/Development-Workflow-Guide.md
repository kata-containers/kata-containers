# Kata Containers Development Workflow Guide

This guide describes how to build Kata Containers locally, publish images to a registry, and test them on a single-node Kubernetes VM.

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

edit ${HOME}/.kcli/profiles.yml

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

### 2.2 Set environment variables(optional)

Those vars are read by tests/integration/kubernetes/gha-run.sh when it deploys the single‑node cluster in 2.3
Edit `~/.bashrc` file and add:

```bash
export CONTAINER_ENGINE="containerd" # Example: Choose container engine (containerd or crio)
export CONTAINER_ENGINE_VERSION="v2.2" # Example: Set your desired containerd version
export KUBERNETES="vanilla" # Example: Kubernetes distribution (vanilla, k3s, rke2, k0s, microk8s)
```

Apply the changes:

```bash
source ~/.bashrc
```

### 2.3 Set up Kata Containers and Kubernetes

Clone the repo, then run the integration script to deploy a single‑node Kubernetes cluster (this may take several minutes).

```bash
# Clone the Kata repo
git clone https://github.com/kata-containers/kata-containers $HOME/kata-containers

# Deploy K8s using the integration script (uses the env vars you set in 2.2)
cd $HOME/kata-containers
bash tests/integration/kubernetes/gha-run.sh deploy-k8s
```

Quick check:

```bash
kubectl get nodes
```

---

## 3. Host development environment packaging

Run this section on your development machine (the host where you build Kata), not inside the Kubernetes VM.

### 3.1 Build tarballs

Clone the repo and build the tarball(s) for the components you are modifying. Kata Containers provide an easy way to build a component, which is simply calling make $component-tarball from the top root dir.

The Components section in the [kata-containers README](https://github.com/kata-containers/kata-containers?tab=readme-ov-file#components) explains each role and how they fit together, and you can check this [local-build Makefile](https://github.com/kata-containers/kata-containers/blob/04c7d116892351abac691b093da8ff82cfcbe2b1/tools/packaging/kata-deploy/local-build/Makefile#L95-L231) to see every supported tarball target you can build locally.

```bash
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

Change to the kata-deploy directory and copy the built tarball(s) into that folder. Set `KATA_ARTIFACTS` to the exact tarball filename you built earlier (the file you copied into this directory); the Dockerfile unpacks it into the image. Replace `<namespace>` with your Quay organization or username so you can push to that repository. Choose a `<tag>` that uniquely identifies this build (e.g., a date or short Git SHA); the dev- prefix is just a convention for non‑release images, and you’ll reuse the same tag later in Helm (image.tag).

Then build the image:

```bash
cd tools/packaging/kata-deploy
# copy your tarball from tools/packaging/kata-deploy/local-build/build into this directory
docker build --build-arg KATA_ARTIFACTS='<tar name>' -t quay.io/<namespace>/kata-deploy:dev-<tag> .
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
  - name: quay-cred
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


