## Introduction

This Dockerfile is used to create a rootfs image that is compatible with Nvidia HGX systems.

## Building and Running

```
docker build --label hgx --tag hgx:latest .
docker run -it -v /dev:/dev -v $(pwd)/output:/output hgx
```

The output artifacts will live in the `output/` directory.