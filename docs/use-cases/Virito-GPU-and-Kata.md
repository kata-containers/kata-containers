# GPU virtualization in kata containers using API-forwarding via virgl and virglrenderer

## I. Introduction

This article introduces API-forwarding with GPU virtualization in kata. the solution involves the utilization of virtio-gpu and virglrenderer. virtio-gpu introduces a virtual OpenGL device called VirGL, using the Gallium3D interface, It allows customers to supply OpenGL commands and GLSL IR（universal OpenGL shader intermediate language） which are redirected from Mesa to virtio-gpu driver on the guest. On the backend, transmitted to QEMU on the host, These commands are then channeled into virglrenderer, and finally, rendering through the host GPU. This solution offers the following significant advantages, It offers high flexibility and compatibility with a wide range of GPU cards. GPU resources can be shared between multiple virtual machines, and includes GPU software emulation as a fallback. It is not limited to a specific architecture and can be applied to the X86 architecture as well, making it a versatile and adaptable GPU solution.  

![Architecture](https://user-images.githubusercontent.com/29703493/249054479-f7de0607-791b-4882-880c-0d49e1517a59.PNG "Architecture")
## II. Deployment

### Build Kata Containers kernel with virtio gpu support
The default guest kernel provided with Kata Containers lacks GPU support. To enable GPU support, certain kernel configurations must be activated:

```sh
CONFIG_DRM=y
CONFIG_DRM_VIRTIO_GPU=y
```

To build the Kata Containers kernel with these configurations, follow the instructions outlined in the "Building Kata Containers Kernel" section. For detailed guidance on building and installing guest kernels, see [the developer guide](https://github.com/kata-containers/kata-containers/blob/main/docs/Developer-Guide.md#install-guest-kernel-images).

```sh
# Prepare (download guest kernel source, generate .config)
./build-kernel.sh -v $KERNEL_VERSION -g virgl -f -d setup

# Build guest kernel
./build-kernel.sh -v $KERNEL_VERSION -g virgl -f -d build

# Install guest kernel
./build-kernel.sh -v $KERNEL_VERSION -g virgl -f -d install
```

An easier way to build a guest kernel that supports virtio-gpu is as follows:

```sh
# Prepare (download guest kernel source, generate .config)
make kernel-virtio-gpu-tarball
```

Before utilizing the new guest kernel, ensure that you update the kernel parameters in configuration.toml:

```sh
kernel = "/usr/shr/kata-containers/vmlinux-virtio-gpu-gpu"
```

### Build Kata Containers Qemu hypervisor with virtio gpu support

To build and install the Kata Containers QEMU hypervisor, refer to [the developer guide](https://github.com/kata-containers/kata-containers/blob/main/docs/Developer-Guide.md). Here are the steps for building QEMU with virtio GPU support:

```sh
$ source kata-containers/tools/packaging/scripts/lib.sh
$ qemu_version="$(get_from_kata_deps "assets.hypervisor.qemu.version")"
$ echo "${qemu_version}"

#Get source from the matching branch of QEMU:

$ git clone -b "${qemu_version}" https://github.com/qemu/qemu.git
$ your_qemu_directory="$(realpath qemu)"
$ packaging_dir="$(realpath kata-containers/tools/packaging)"

#Apply qemu patches
$ "$packaging_dir/scripts/apply_patches.sh" "$packaging_dir/qemu/patches/$qemu_version/"

$ pushd "$your_qemu_directory"
$ "$packaging_dir/scripts/configure-hypervisor.sh" -g kata-qemu | xargs sudo -E ./configure
$ make -j $(nproc --ignore=1)

# Optional
$ sudo -E make install
$ popd
```

> **Note**:
> Please note that certain dependency libraries must be installed. Ensure that you have the following packages:
> libepoxy-dev  libvirglrenderer-dev python3-venv libgdm-dev libgtk-3-dev libsdl2-dev librbd-dev librados-dev libgbm-dev

Before using the new Qemu Hypervisor, please update these parameters in configuration.toml.

```sh
[hypervisor.qemu]
path = "$qemu/destdir/opt/kata/bin/qemu-system-aarch64"
virtio_gpu = "virtio-gpu-gl-pci"
display = "egl-headless"
```

### Build and install the Kata Containers runtime

For further details on building and installing Kata Containers runtime, please see [the developer guide](https://github.com/kata-containers/kata-containers/blob/main/docs/Developer-Guide.md). The following steps provide a general overview:


```sh
cd $KATA/src/runtime
make && sudo make install
```

## III. Use-Cases

### Prepare kata-image

```sh
$ cat Dockerfile.ubuntu-xfce1
FROM ubuntu:22.04

MAINTAINER Sven Nierlein "sven@consol.de"
ENV REFRESHED_AT 2023-01-27

LABEL io.k8s.description="Headless VNC Container with Xfce window manager, firefox and chromium" \
      io.k8s.display-name="Headless VNC Container based on Debian" \
      io.openshift.expose-services="6901:http,5901:xvnc" \
      io.openshift.tags="vnc, debian, xfce" \
      io.openshift.non-scalable=true

## Connection ports for controlling the UI:
# VNC port:5901
# noVNC webport, connect via http://IP:6901/?password=vncpassword
ENV DISPLAY=:1 \
    VNC_PORT=:5901

EXPOSE $VNC_PORT

### Envrionment config

ENV HOME=/headless \
    TERM=xterm \
    STARTUPDIR=/dockerstartup \
    INST_SCRIPTS=/headless/install \
    NO_VNC_HOME=/headless/noVNC \
    DEBIAN_FRONTEND=noninteractive \
    VNC_COL_DEPTH=24 \
    VNC_RESOLUTION=1280x1024 \
    VNC_PW=vncpassword \
    VNC_VIEW_ONLY=false
WORKDIR $HOME

### Add all install scripts for further steps
ADD ./src/common/install/ $INST_SCRIPTS/
ADD ./src/debian/install/ $INST_SCRIPTS/

### Install some common tools

RUN $INST_SCRIPTS/tools.sh
ENV LANG='en_US.UTF-8' LANGUAGE='en_US:en' LC_ALL='en_US.UTF-8'

### Install custom fonts

RUN $INST_SCRIPTS/install_custom_fonts.sh

### Install xfce UI

RUN $INST_SCRIPTS/xfce_ui.sh
ADD ./src/common/xfce/ $HOME/

### configure startup

RUN $INST_SCRIPTS/libnss_wrapper.sh
ADD ./src/common/scripts $STARTUPDIR
RUN $INST_SCRIPTS/set_user_permission.sh $STARTUPDIR $HOME

USER 0

ENTRYPOINT ["/dockerstartup/startup.sh"]
CMD ["--wait"]

cat ./src/common/scripts/startup.sh
#!/bin/bash
### every exit != 0 fails the script
set -e
mkdir -p "$HOME/.vnc"
PASSWD_PATH="$HOME/.vnc/passwd"

echo "$VNC_PW" | vncpasswd -f >> $PASSWD_PATH
chmod 600 $PASSWD_PATH

startx &
sleep 3

x11vnc -rfbport $VNC_PORT -rfbauth $HOME/.vnc/passwd  -display :0 -forever -bg -repeat -nowf -o $HOME/.vnc/x11vnc.log

echo "Executing command: '$@'"
exec "$@"
```

### Build Docker Image

```sh
$ docker build -t consol/ubuntu-xfce1 -f Dockerfile.ubuntu-xfce1 . --load
```

### Run Container

```sh
$ sudo nerdctl run -p "$VNC_PORT":"$VNC_PORT"  --runtime io.containerd.kata.v2 \
  --env "VNC_PORT=$VNC_PORT" --env "VNC_PW=$VNC_PW" --env "USER=$USER"  \
  --group-add 1001  --group-add 44  --group-add 108  --device /dev/dri/card0:/dev/dri/card0  \
  --device /dev/dri/renderD128:/dev/dri/renderD128  --device /dev/vga_arbiter:/dev/vga_arbiter \
  --rm -it docker.io/consol/ubuntu-xfce1:latest bash
```

### Verify GPU Configuration

```sh
$root@69d866f5627e:~# lspci -kk
00:00.0 Host bridge: Red Hat, Inc. QEMU PCIe Host bridge
	Subsystem: Red Hat, Inc. QEMU PCIe Host bridge
lspci: Unable to load libkmod resources: error -2
00:01.0 Communication controller: Red Hat, Inc. Virtio console
	Subsystem: Red Hat, Inc. Virtio console
	Kernel driver in use: virtio-pci
00:02.0 PCI bridge: Red Hat, Inc. QEMU PCI-PCI bridge
00:03.0 SCSI storage controller: Red Hat, Inc. Virtio block device
	Subsystem: Red Hat, Inc. Virtio block device
	Kernel driver in use: virtio-pci
00:04.0 Display controller: Red Hat, Inc. Virtio GPU (rev 01)
	Subsystem: Red Hat, Inc. Virtio GPU
	Kernel driver in use: virtio-pci
00:05.0 SCSI storage controller: Red Hat, Inc. Virtio SCSI
	Subsystem: Red Hat, Inc. Virtio SCSI
	Kernel driver in use: virtio-pci

$root@69d866f5627e:~# ps aux
root          27  0.3  2.2 604512 45288 ?        Sl   10:08   0:00 /usr/lib/xorg/Xorg -nolisten tcp :0 -auth /tmp/serverauth.G4zfkyYY9C $root@69d866f5627e:~# glxinfo -B
name of display: :0.0
display: :0  screen: 0
direct rendering: Yes
Extended renderer info (GLX_MESA_query_renderer):
    Vendor: Mesa (0x1af4)
    Device: virgl (NV166) (0x1010)
    Version: 23.0.4
    Accelerated: yes
    Video memory: 0MB
    Unified memory: no
    Preferred profile: core (0x1)
    Max core profile version: 4.2
    Max compat profile version: 4.2
    Max GLES1 profile version: 1.1
    Max GLES[23] profile version: 3.2
OpenGL vendor string: Mesa
OpenGL renderer string: virgl (NV166)
OpenGL core profile version string: 4.2 (Core Profile) Mesa 23.0.4-0ubuntu1~22.04.1
OpenGL core profile shading language version string: 4.20
OpenGL core profile context flags: (none)
OpenGL core profile profile mask: core profile

OpenGL version string: 4.2 (Compatibility Profile) Mesa 23.0.4-0ubuntu1~22.04.1
OpenGL shading language version string: 4.20
OpenGL context flags: (none)
OpenGL profile mask: compatibility profile

OpenGL ES profile version string: OpenGL ES 3.2 Mesa 23.0.4-0ubuntu1~22.04.1
OpenGL ES profile shading language version string: OpenGL ES GLSL ES 3.20
```

### Remote access via vncviewer 

```bash
$vncviewer $IP-address:$N
```

### Display Information and Graphics Benchmark

```bash
$root@69d866f5627e:~# DISPLAY=:0
$root@69d866f5627e:~# glmark2
=======================================================
    glmark2 2021.02
=======================================================
    OpenGL Information
    GL_VENDOR:     Mesa
    GL_RENDERER:   virgl (NV166)
    GL_VERSION:    4.2 (Compatibility Profile) Mesa 23.0.4-0ubuntu1~22.04.1
=======================================================
[build] use-vbo=false: FPS: 282 FrameTime: 3.546 ms
[build] use-vbo=true: FPS: 282 FrameTime: 3.546 ms
[texture] texture-filter=nearest: FPS: 276 FrameTime: 3.623 ms
[texture] texture-filter=linear: FPS: 315 FrameTime: 3.175 ms
[texture] texture-filter=mipmap: FPS: 286 FrameTime: 3.497 ms
[shading] shading=gouraud: FPS: 198 FrameTime: 5.051 ms
[shading] shading=blinn-phong-inf: FPS: 219 FrameTime: 4.566 ms
[shading] shading=phong: FPS: 238 FrameTime: 4.202 ms
[shading] shading=cel: FPS: 426 FrameTime: 2.347 ms
[bump] bump-render=high-poly: FPS: 199 FrameTime: 5.025 ms
[bump] bump-render=normals: FPS: 421 FrameTime: 2.375 ms
[bump] bump-render=height: FPS: 209 FrameTime: 4.785 ms
[effect2d] kernel=0,1,0;1,-4,1;0,1,0;: FPS: 188 FrameTime: 5.319 ms
[effect2d] kernel=1,1,1,1,1;1,1,1,1,1;1,1,1,1,1;: FPS: 194 FrameTime: 5.155 ms
[pulsar] light=false:quads=5:texture=false: FPS: 288 FrameTime: 3.472 ms
[desktop] blur-radius=5:effect=blur:passes=1:separable=true:windows=4: FPS: 217 FrameTime: 4.608 ms
[desktop] effect=shadow:windows=4: FPS: 223 FrameTime: 4.484 ms
[buffer] columns=200:interleave=false:update-dispersion=0.9:update-fraction=0.5:update-method=map: FPS: 90 FrameTime: 11.111 ms
[buffer] columns=200:interleave=false:update-dispersion=0.9:update-fraction=0.5:update-method=subdata: FPS: 163 FrameTime: 6.135 ms
[buffer] columns=200:interleave=true:update-dispersion=0.9:update-fraction=0.5:update-method=map: FPS: 90 FrameTime: 11.111 ms
[ideas] speed=duration: FPS: 215 FrameTime: 4.651 ms
[jellyfish] <default>: FPS: 243 FrameTime: 4.115 ms
[terrain] <default>: FPS: 94 FrameTime: 10.638 ms
[shadow] <default>: FPS: 199 FrameTime: 5.025 ms
[refract] <default>: FPS: 170 FrameTime: 5.882 ms
[conditionals] fragment-steps=0:vertex-steps=0: FPS: 799 FrameTime: 1.252 ms
[conditionals] fragment-steps=5:vertex-steps=0: FPS: 722 FrameTime: 1.385 ms
[conditionals] fragment-steps=0:vertex-steps=5: FPS: 568 FrameTime: 1.761 ms
[function] fragment-complexity=low:fragment-steps=5: FPS: 456 FrameTime: 2.193 ms
[function] fragment-complexity=medium:fragment-steps=5: FPS: 523 FrameTime: 1.912 ms
[loop] fragment-loop=false:fragment-steps=5:vertex-steps=5: FPS: 902 FrameTime: 1.109 ms
[loop] fragment-steps=5:fragment-uniform=false:vertex-steps=5: FPS: 671 FrameTime: 1.490 ms
[loop] fragment-steps=5:fragment-uniform=true:vertex-steps=5: FPS: 495 FrameTime: 2.020 ms
=======================================================
                                  glmark2 Score: 329 
=======================================================  
```
![Benchmark1](https://user-images.githubusercontent.com/117434719/265878698-9313b8bb-aad1-4b56-be41-964dc0fd3030.png "Benchmark1")
![Benchmark2](https://user-images.githubusercontent.com/117434719/265878715-00354df5-3af8-4b8a-a3b1-dbd2a4cf61f0.png "Benchmark2")
