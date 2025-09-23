## Introduction

This Dockerfile is used to create a rootfs image that is compatible with Nvidia HGX systems.

## Building and Running

You first need the kernel nvidia headers from the kernel-nvidia-gpu-tarball step in `tools/packaging/kata-deploy/local-build`:

```
cd tools/packaging/kata-deploy/local-build/ 
make kernel-nvidia-gpu-tarball
```

```
docker build --label hgx --tag hgx:latest .
docker run -it -v /dev:/dev -v $(pwd)/output:/output hgx
```

The output artifacts will live in the `output/` directory.